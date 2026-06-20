use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

/// API 错误。
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// 依赖服务当前不可用。
    #[error("service unavailable")]
    ServiceUnavailable,
}

/// 统一错误响应外层结构。
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    /// 错误详情。
    pub error: ErrorBody,
}

/// 对客户端安全的错误详情。
#[derive(Debug, Serialize)]
pub struct ErrorBody {
    /// 稳定的机器可读错误码。
    pub code: &'static str,
    /// 不包含内部实现细节的错误消息。
    pub message: &'static str,
    /// 可选请求标识，供后续链路追踪扩展。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match self {
            Self::ServiceUnavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                ErrorEnvelope {
                    error: ErrorBody {
                        code: "service_unavailable",
                        message: "database is unavailable",
                        request_id: None,
                    },
                },
            ),
        };

        (status, Json(body)).into_response()
    }
}
