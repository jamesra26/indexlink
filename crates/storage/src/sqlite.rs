//! SQLite 本地存储基础设施与 migration runner。

use std::{
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};

use crate::StorageError;

/// SQLite 本地存储连接。
///
/// 此类型在 SQLite adapter 接入前提供连接、迁移与健康检查基础设施；它不会改变
/// 现有 PostgreSQL production wiring。
#[derive(Clone, Debug)]
pub struct SqliteStorage {
    pool: SqlitePool,
}

impl SqliteStorage {
    /// 使用调用方提供的 SQLite 连接池构建存储句柄。
    #[must_use]
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// 使用指定连接池参数连接本地 SQLite 数据库。
    ///
    /// 连接会自动创建尚不存在的数据库文件、启用外键约束，并设置 WAL journal 模式。
    ///
    /// # 错误
    ///
    /// 配置无效、URL 无效、连接超时或 SQLite 无法打开数据库时返回 [`StorageError`]。
    pub async fn connect_with_options(
        database_url: &str,
        max_connections: u32,
        connect_timeout: Duration,
    ) -> Result<Self, StorageError> {
        if max_connections == 0 {
            return Err(StorageError::InvalidConfiguration(
                "database max connections must be greater than zero",
            ));
        }
        if connect_timeout.is_zero() {
            return Err(StorageError::InvalidConfiguration(
                "database connect timeout must be greater than zero",
            ));
        }

        let options = SqliteConnectOptions::from_str(database_url)
            .map_err(StorageError::InvalidDatabaseUrl)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(connect_timeout);
        let connect = SqlitePoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(connect_timeout)
            .connect_with(options);

        let pool = tokio::time::timeout(connect_timeout, connect)
            .await
            .map_err(|_| StorageError::ConnectionTimeout {
                seconds: connect_timeout.as_secs(),
            })?
            .map_err(StorageError::Connection)?;

        Ok(Self { pool })
    }

    /// 执行 SQLite 专用 migration 目录中的全部 migration。
    ///
    /// 调用方应在开始提供 HTTP 服务前调用此方法；失败时返回
    /// [`StorageError::Migration`]，以阻止服务运行在不完整 schema 上。
    pub async fn migrate(&self) -> Result<(), StorageError> {
        sqlx::migrate::Migrator::new(sqlite_migration_directory())
            .await
            .map_err(StorageError::Migration)?
            .run(&self.pool)
            .await
            .map_err(StorageError::Migration)
    }

    /// 检查 SQLite 是否可响应查询。
    ///
    /// # 错误
    ///
    /// 数据库不可用时返回 [`StorageError::Ping`]。
    pub async fn ping(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(StorageError::Ping)?;
        Ok(())
    }

    /// 返回底层 SQLite 连接池。
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

fn sqlite_migration_directory() -> PathBuf {
    let workspace_relative = PathBuf::from("migrations/sqlite");
    if workspace_relative.is_dir() {
        workspace_relative
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../migrations/sqlite")
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use super::*;

    /// 验证 migration 可在隔离的本地 SQLite 数据库中创建 MVP schema。
    #[tokio::test]
    async fn migration_creates_mvp_schema_in_memory() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .in_memory(true)
                    .foreign_keys(true),
            )
            .await
            .expect("in-memory SQLite pool must connect");
        let storage = SqliteStorage::from_pool(pool);

        storage
            .migrate()
            .await
            .expect("SQLite migration must apply");
        let tables = sqlx::query_scalar::<_, String>(
            "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name",
        )
        .fetch_all(storage.pool())
        .await
        .expect("SQLite schema tables must be queryable");

        assert_eq!(
            tables,
            vec![
                "_sqlx_migrations".to_owned(),
                "decision_records".to_owned(),
                "investment_plans".to_owned(),
            ]
        );
    }

    /// 验证 SQLite 连接配置在接触数据库前拒绝零连接数。
    #[tokio::test]
    async fn connect_rejects_zero_max_connections() {
        let error =
            SqliteStorage::connect_with_options("sqlite::memory:", 0, Duration::from_secs(1))
                .await
                .expect_err("zero max connections must fail");

        assert!(matches!(error, StorageError::InvalidConfiguration(_)));
    }
}
