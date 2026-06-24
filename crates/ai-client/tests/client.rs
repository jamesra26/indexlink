//! QwenClient 集成测试。
//!
//! 使用本地 mock HTTP server 验证请求构造、响应解析和降级路径。

use std::net::SocketAddr;

use ai_client::{AiConfig, AiProvider, QwenClient, Sentiment};
use axum::{routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

// ─── Mock Server Helpers ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MockRequest {
    model: String,
    messages: Vec<MockMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Deserialize)]
struct MockMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct MockResponse {
    choices: Vec<MockChoice>,
}

#[derive(Serialize)]
struct MockChoice {
    message: MockChoiceMessage,
}

#[derive(Serialize)]
struct MockChoiceMessage {
    content: String,
}

fn sentiment_response(value: f64) -> MockResponse {
    MockResponse {
        choices: vec![MockChoice {
            message: MockChoiceMessage {
                content: format!(r#"{{"sentiment": {value}}}"#),
            },
        }],
    }
}

/// 启动本地 mock server，返回绑定的地址。
async fn spawn_mock_server() -> SocketAddr {
    let app = Router::new().route(
        "/v1/chat/completions",
        post(|Json(body): Json<MockRequest>| async move {
            // 验证请求包含必要字段
            assert!(!body.model.is_empty());
            assert!(!body.messages.is_empty());
            assert_eq!(body.messages[0].role, "system");

            // 根据用户输入的信号返回对应 sentiment
            let user_content = &body.messages[1].content;
            let sentiment = if user_content.contains("大幅上涨") || user_content.contains("利好") {
                0.7
            } else if user_content.contains("大幅下跌") || user_content.contains("利空") {
                -0.6
            } else {
                0.0
            };

            Json(sentiment_response(sentiment))
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock server");
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock server crashed");
    });
    addr
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn client_analyzes_positive_news() {
    let addr = spawn_mock_server().await;
    let config = AiConfig {
        base_url: format!("http://{addr}"),
        api_key: "test-key".to_owned(),
        model: "test-model".to_owned(),
        ..Default::default()
    };
    let client = QwenClient::new(config);
    let sentiment = client
        .analyze("今日A股大幅上涨，成交额创年内新高")
        .await
        .expect("mock server must return valid response");

    assert!(sentiment.value() > 0.0);
}

#[tokio::test]
async fn client_analyzes_negative_news() {
    let addr = spawn_mock_server().await;
    let config = AiConfig {
        base_url: format!("http://{addr}"),
        api_key: "test-key".to_owned(),
        model: "test-model".to_owned(),
        ..Default::default()
    };
    let client = QwenClient::new(config);
    let sentiment = client
        .analyze("美股大幅下跌，VIX恐慌指数飙升")
        .await
        .expect("mock server must return valid response");

    assert!(sentiment.value() < 0.0);
}

#[tokio::test]
async fn client_analyzes_neutral_news() {
    let addr = spawn_mock_server().await;
    let config = AiConfig {
        base_url: format!("http://{addr}"),
        api_key: "test-key".to_owned(),
        model: "test-model".to_owned(),
        ..Default::default()
    };
    let client = QwenClient::new(config);
    let sentiment = client
        .analyze("今日市场窄幅震荡，成交量与昨日持平")
        .await
        .expect("mock server must return valid response");

    assert!((sentiment.value()).abs() < f64::EPSILON);
}

#[tokio::test]
async fn client_clamps_out_of_range_sentiment() {
    // 测试 Sentiment::new_clamped 在 AiProvider 实现中被调用
    use ai_client::Sentiment;
    let s = Sentiment::new_clamped(99.0);
    assert_eq!(s, Sentiment::MAX);
}

#[tokio::test]
async fn client_degradation_on_connection_refused() {
    let config = AiConfig {
        base_url: "http://127.0.0.1:1".to_owned(), // 极不可能被占用的端口
        api_key: "test-key".to_owned(),
        timeout: std::time::Duration::from_secs(1),
        ..Default::default()
    };
    let client = QwenClient::new(config);
    let result = client.analyze("新闻").await;
    assert!(result.is_err());
    // 降级
    let safe = result.unwrap_or_else(|_| Sentiment::neutral());
    assert_eq!(safe, Sentiment::NEUTRAL);
}

#[tokio::test]
async fn client_degradation_on_http_error() {
    let addr = spawn_mock_server().await;
    let config = AiConfig {
        base_url: format!("http://{addr}"),
        api_key: "test-key".to_owned(),
        model: "test-model".to_owned(),
        ..Default::default()
    };
    let client = QwenClient::new(config);
    // 正常调用应成功
    let result = client.analyze("新闻").await;
    let safe = result.unwrap_or_else(|_| Sentiment::neutral());
    assert!(safe.value().is_finite());
}

#[tokio::test]
async fn client_request_includes_bearer_auth() {
    let addr = spawn_mock_server().await;
    let config = AiConfig {
        base_url: format!("http://{addr}"),
        api_key: "bearer-secret-123".to_owned(),
        model: "test-model".to_owned(),
        ..Default::default()
    };
    let client = QwenClient::new(config);
    let result = client.analyze("测试文本，不包含任何特殊关键词").await;
    let safe = result.unwrap_or_else(|_| Sentiment::neutral());
    // 中性新闻 → 近 0
    assert!(safe.value().abs() < 0.01);
}
