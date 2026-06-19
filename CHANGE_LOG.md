# Change Log

## Unreleased

### 2026-06-19（文档）

- 执行模型：Composer。
- 变更类型：文档。
- 涉及文件：
  - `readme.md`
  - `CHANGE_LOG.md`
- 变更内容：
  - 将 `readme.md` 顶部徽章区从 AuroraView 模板链接替换为 IndexLink（`jamesra26/indexlink`）项目链接。
  - 移除不适用的 PyPI、Python、Codecov、CI workflow、pre-commit、ruff、mypy 等徽章。
  - 补充 Rust workspace、crate 结构、CHANGELOG、AGENTS 及 GitHub 社区类徽章；页脚链接改为 Issue Tracker、LICENSE、CHANGELOG。
- 验证：
  - 文档改动，未运行测试。

### 2026-06-19 23:12 UTC+10

- 执行模型：GPT-5.5。
- 变更类型：领域 API 风格一致性。
- 涉及文件：
  - `crates/core-domain/src/lib.rs`
  - `CHANGE_LOG.md`
- 变更内容：
  - 为 `Multiplier` 增加 `Display` 实现，按百分比格式输出倍率，保持与 `Percentile` 的格式化能力对称。
  - 为 `Multiplier` 的 `Display` 行为增加单元测试。
  - 为 `Action` 增加 `Hash` 派生，便于后续作为 `HashMap` 键或用于去重统计。
  - 保持 workspace edition 为 `2021`，未进行 edition 升级。
- 验证：
  - `cargo test -p core-domain` 通过：13 个单元测试全部通过。
  - `cargo llvm-cov -p core-domain --summary-only` 通过：Region / Function / Line 覆盖率均为 100.00%。
  - `crates/core-domain/src/lib.rs` 无 IDE linter 错误。

### 2026-06-19（文档）

- 执行模型：Composer。
- 变更类型：文档。
- 涉及文件：
  - `AGENTS.md`
  - `CHANGE_LOG.md`
- 变更内容：
  - 在 `AGENTS.md` 顶部补充基于 `readme.md` 提炼的项目一句话描述。
- 验证：
  - 文档改动，未运行测试。

### 2026-06-19 23:02 UTC+10

- 执行模型：GPT-5.5。
- 变更类型：Agent 协作规范。
- 涉及文件：
  - `AGENT.md`
  - `CHANGE_LOG.md`
- 变更内容：
  - 在 `AGENT.md` 写明其他 agent 应遵循的项目规范，包括中文回复、变更日志记录、Rust crate 分层边界、`core-domain` lint 约束、newtype 不变量、测试覆盖率和审计/serde 原则。
- 验证：
  - 文档改动，未运行测试。

### 2026-06-19 23:00 UTC+10

- 执行模型：GPT-5.5。
- 变更类型：测试覆盖率补强。
- 涉及文件：
  - `crates/core-domain/src/lib.rs`
  - `CHANGE_LOG.md`
- 变更内容：
  - 为 `PercentileError` 的 `Display` 输出增加单元测试，覆盖 `Nan` 与 `OutOfRange` 两种错误格式化。
  - 为 `Percentile` 的 `Display` 输出增加单元测试，覆盖百分比格式化行为。
- 验证：
  - `cargo test -p core-domain` 通过：12 个单元测试全部通过。
  - `cargo llvm-cov -p core-domain --summary-only` 通过：Region / Function / Line 覆盖率均为 100.00%。
  - `crates/core-domain/src/lib.rs` 无 IDE linter 错误。

### 2026-06-19 22:52 UTC+10

- 执行模型：GPT-5.5。
- 变更类型：代码质量 / 文档约束。
- 涉及文件：
  - `crates/core-domain/src/lib.rs`
  - `CHANGE_LOG.md`
- 变更内容：
  - 在 `core-domain` 增加 crate 级 `#![forbid(unsafe_code)]`，明确基础领域 crate 禁止 unsafe 代码。
  - 在 `core-domain` 增加 crate 级 `#![warn(missing_docs)]`，要求后续公开领域 API 补齐文档。
  - 补齐 `Multiplier::MIN`、`Multiplier::MAX`、`Multiplier::value` 的公开文档。
  - 补齐 `PercentileError::OutOfRange.value` 字段文档，消除新增 `missing_docs` 警告。
- 验证：
  - `cargo check -p core-domain` 通过。
  - `crates/core-domain/src/lib.rs` 无 IDE linter 错误。

### 2026-06-19 22:49 UTC+10

- 执行模型：GPT-5.5。
- 变更类型：路线图 / 审计能力说明。
- 涉及文件：
  - `readme.md`
- 变更内容：
  - 在第 4 阶段路线图中明确后续为纯数据结构补充 feature-gated `serde` 支持。
  - 说明 `serde` 仅提供数据编码/解码能力，不引入 IO。
  - 说明 `Percentile`、`Multiplier` 等带不变量的 newtype 反序列化必须复用构造校验，避免绕过安全边界。
- 验证：
  - 文档改动，未运行测试。
