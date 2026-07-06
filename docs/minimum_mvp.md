# IndexLink 最小 MVP 清单

本文档描述“演示可用”的最小 MVP，而不是完整生产版本。当前目标是让后端可以稳定演示：投资计划创建、执行预览、双桶投入拆分，以及后续接入阿里云 DashScope/Qwen 后的市场情绪分析闭环。

## MVP 边界

最小 MVP 聚焦单用户、单仓库后端演示，不包含真实券商下单、复杂多用户权限、自动调度器和完整前端。金额统一使用 `Decimal`，HTTP JSON 边界以字符串表达金额，避免浮点精度问题。

## 已实现内容

- 后端基础设施
  - Axum API server。
  - PostgreSQL storage adapter。
  - 健康检查与 readiness 检查。
  - 统一 JSON error envelope。

- 投资计划
  - 创建 investment plan。
  - 列出 investment plans。
  - 按 ID 查询 investment plan。
  - 更新 investment plan。
  - 领域层规范化 `name`、`symbol`、`currency`、金额与执行日。
  - 入站 DTO 与领域类型隔离，避免 serde 直接构造带不变量的领域模型。

- 执行预览
  - 判断计划在指定月内日期是否 `due`、`waiting` 或 `inactive`。
  - due 时返回受 `max_single_execution` 限制后的计划投入金额。
  - API 暴露执行预览入口：`POST /investment-plans/:id/execution-preview`。

- 双桶投入拆分
  - 支持 `core` 与 `opportunity` 两个投入桶。
  - 支持校验双桶比例必须在 `0..=1`。
  - 支持校验两个比例合计必须为 `1`。
  - due 且请求提供双桶配置时，返回本次计划投入金额的 core/opportunity 拆分。
  - 非 due 状态下不返回投入金额，也不返回双桶拆分。

- AI 感知层库能力
  - DashScope/OpenAI-compatible Qwen client。
  - CNBC RSS 新闻源。
  - 新闻格式化为 sentiment prompt。
  - 拉取新闻并调用 AI provider 返回市场情绪的库层 pipeline。

## 最小演示流程

1. 启动 PostgreSQL 与 API server。
2. 调用健康检查，确认服务可用。
3. 创建一个 investment plan，例如 VOO 每月 15 日定投。
4. 查询该计划，展示服务端规范化后的 symbol、currency 和字符串金额。
5. 调用执行预览接口，传入非执行日，展示 `waiting`，且不返回投入金额。
6. 调用执行预览接口，传入执行日和双桶比例，例如 `0.80 / 0.20`。
7. 展示返回结果：
   - `status = due`
   - `planned_contribution`
   - `bucket_split.core_contribution`
   - `bucket_split.opportunity_contribution`
8. 接入阿里云 API Key 后，追加演示市场新闻情绪分析。
9. 后续 decision preview 接口完成后，再展示“投资计划 + 双桶 + AI 情绪”的组合建议。

## 示例请求

```http
POST /investment-plans/:id/execution-preview
Content-Type: application/json

{
  "day_of_month": 15,
  "bucket_allocation": {
    "core_ratio": "0.80",
    "opportunity_ratio": "0.20"
  }
}
```

示例响应：

```json
{
  "plan_id": "00000000-0000-0000-0000-000000000001",
  "symbol": "VOO",
  "currency": "USD",
  "schedule_kind": "monthly",
  "schedule_day": 15,
  "day_of_month": 15,
  "status": "due",
  "planned_contribution": "1000",
  "bucket_split": {
    "planned_contribution": "1000",
    "core_contribution": "800.00",
    "opportunity_contribution": "200.00"
  }
}
```

## 阿里云接入后需要补齐

- Server config
  - 从环境变量读取 `DASHSCOPE_API_KEY`。
  - 支持配置 Qwen model、base URL、timeout、temperature 和 max tokens。
  - 不在日志或错误响应中暴露密钥。

- API state
  - 在 server 启动时构造 Qwen client。
  - 将 AI provider 与新闻源注入 API state。
  - readiness 能区分“数据库不可用”和“AI 配置缺失”。

- Market sentiment API
  - 新增市场情绪分析 endpoint。
  - 测试使用 fake provider。
  - 演示环境使用真实 DashScope/Qwen。

- Decision preview API
  - 组合 investment plan、执行预览、双桶拆分、AI sentiment 和量化信号。
  - 返回建议投入金额、core/opportunity 分配和简短解释。

## 暂不属于最小 MVP

- 真实券商下单。
- 自动定时任务。
- 订单状态机与成交回报。
- 多用户账户体系。
- 完整前端。
- 真实 trend 量化模型替代所有 fallback。

## 待完成任务

1. 接入阿里云配置到 server/API state。
2. 新增 market sentiment API。
3. 新增 AI readiness 与安全错误映射。
4. 新增 decision preview 领域类型。
5. 新增 decision preview API。
6. 补充 demo seed 或 smoke 文档。
7. 使用真实 `DASHSCOPE_API_KEY` 做一次手动 smoke test。
