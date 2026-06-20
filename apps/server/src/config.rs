use std::{
    env,
    net::{IpAddr, SocketAddr},
    num::ParseIntError,
    time::Duration,
};

use axum::http::HeaderValue;

const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "8080";
const DEFAULT_MAX_CONNECTIONS: &str = "10";
const DEFAULT_CONNECT_TIMEOUT_SECONDS: &str = "5";

#[derive(Debug)]
pub(crate) struct Config {
    pub(crate) address: SocketAddr,
    pub(crate) database_url: String,
    pub(crate) database_max_connections: u32,
    pub(crate) database_connect_timeout: Duration,
    pub(crate) cors_allowed_origins: Vec<HeaderValue>,
}

impl Config {
    pub(crate) fn from_env() -> Result<Self, ConfigError> {
        let host = env::var("APP_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_owned());
        let port = parse_u16("APP_PORT", &env_or_default("APP_PORT", DEFAULT_PORT))?;
        let ip = host
            .parse::<IpAddr>()
            .map_err(|_| ConfigError::InvalidHost)?;

        let database_url = env::var("DATABASE_URL").map_err(|_| ConfigError::MissingDatabaseUrl)?;
        if database_url.trim().is_empty() {
            return Err(ConfigError::MissingDatabaseUrl);
        }

        let database_max_connections = parse_u32(
            "DATABASE_MAX_CONNECTIONS",
            &env_or_default("DATABASE_MAX_CONNECTIONS", DEFAULT_MAX_CONNECTIONS),
        )?;
        if database_max_connections == 0 {
            return Err(ConfigError::NonPositive("DATABASE_MAX_CONNECTIONS"));
        }

        let timeout_seconds = parse_u64(
            "DATABASE_CONNECT_TIMEOUT_SECONDS",
            &env_or_default(
                "DATABASE_CONNECT_TIMEOUT_SECONDS",
                DEFAULT_CONNECT_TIMEOUT_SECONDS,
            ),
        )?;
        if timeout_seconds == 0 {
            return Err(ConfigError::NonPositive("DATABASE_CONNECT_TIMEOUT_SECONDS"));
        }

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(|origin| {
                origin
                    .parse::<HeaderValue>()
                    .map_err(|_| ConfigError::InvalidCorsOrigin)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            address: SocketAddr::new(ip, port),
            database_url,
            database_max_connections,
            database_connect_timeout: Duration::from_secs(timeout_seconds),
            cors_allowed_origins,
        })
    }
}

fn env_or_default(name: &'static str, default: &'static str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn parse_u16(name: &'static str, value: &str) -> Result<u16, ConfigError> {
    value
        .parse()
        .map_err(|source| ConfigError::InvalidInteger { name, source })
}

fn parse_u32(name: &'static str, value: &str) -> Result<u32, ConfigError> {
    value
        .parse()
        .map_err(|source| ConfigError::InvalidInteger { name, source })
}

fn parse_u64(name: &'static str, value: &str) -> Result<u64, ConfigError> {
    value
        .parse()
        .map_err(|source| ConfigError::InvalidInteger { name, source })
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("DATABASE_URL must be set")]
    MissingDatabaseUrl,
    #[error("APP_HOST must be a valid IP address")]
    InvalidHost,
    #[error("{name} must be a valid integer")]
    InvalidInteger {
        name: &'static str,
        #[source]
        source: ParseIntError,
    },
    #[error("{0} must be greater than zero")]
    NonPositive(&'static str),
    #[error("CORS_ALLOWED_ORIGINS contains an invalid origin")]
    InvalidCorsOrigin,
}
