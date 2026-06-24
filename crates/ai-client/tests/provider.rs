//! AiProvider trait 与 AiConfig 集成测试。
//!
//! 验证：Mock 实现、trait 可替换性、密钥安全。

use std::sync::Mutex;

use ai_client::{AiClientError, AiConfig, AiProvider, Sentiment};

// ─── Mock Provider ───────────────────────────────────────────────────────────

/// 可预设返回值的 Mock AI 提供者，用于测试 trait 接口。
struct MockAiProvider {
    response: Mutex<Result<Sentiment, AiClientError>>,
}

impl MockAiProvider {
    fn with_sentiment(s: Sentiment) -> Self {
        Self {
            response: Mutex::new(Ok(s)),
        }
    }

    fn with_error(e: AiClientError) -> Self {
        Self {
            response: Mutex::new(Err(e)),
        }
    }
}

#[async_trait::async_trait]
impl AiProvider for MockAiProvider {
    async fn analyze(&self, _prompt: &str) -> Result<Sentiment, AiClientError> {
        // 简化处理：clone 错误需要构造新实例
        let guard = self.response.lock().expect("mock lock poisoned");
        match &*guard {
            Ok(s) => Ok(*s),
            Err(AiClientError::Timeout { seconds }) => {
                Err(AiClientError::Timeout { seconds: *seconds })
            }
            Err(AiClientError::HttpStatus { status }) => {
                Err(AiClientError::HttpStatus { status: *status })
            }
            Err(AiClientError::UnexpectedStructure) => Err(AiClientError::UnexpectedStructure),
            Err(AiClientError::ParseFailure) => Err(AiClientError::ParseFailure),
            Err(AiClientError::EmptyResponse) => Err(AiClientError::EmptyResponse),
            Err(_) => Err(AiClientError::EmptyResponse),
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn mock_provider_returns_positive_sentiment() {
    let provider = MockAiProvider::with_sentiment(Sentiment::new(0.6).unwrap());
    let result = provider.analyze("利好新闻").await.unwrap();
    assert_eq!(result.value(), 0.6);
}

#[tokio::test]
async fn mock_provider_returns_negative_sentiment() {
    let provider = MockAiProvider::with_sentiment(Sentiment::new(-0.4).unwrap());
    let result = provider.analyze("利空新闻").await.unwrap();
    assert_eq!(result.value(), -0.4);
}

#[tokio::test]
async fn mock_provider_returns_neutral() {
    let provider = MockAiProvider::with_sentiment(Sentiment::neutral());
    let result = provider.analyze("任何新闻").await.unwrap();
    assert_eq!(result, Sentiment::NEUTRAL);
}

#[tokio::test]
async fn mock_provider_returns_error_then_degradation() {
    let provider =
        MockAiProvider::with_error(AiClientError::Timeout { seconds: 30 });
    let result = provider.analyze("新闻").await;
    assert!(result.is_err());
    // 模拟退化
    let safe = result.unwrap_or_else(|_| Sentiment::neutral());
    assert_eq!(safe, Sentiment::NEUTRAL);
}

#[test]
fn config_debug_hides_api_key() {
    let config = AiConfig {
        api_key: "sk-very-secret-key-do-not-leak".to_owned(),
        ..Default::default()
    };
    let debug = format!("{config:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("sk-very-secret-key-do-not-leak"));
}

#[test]
fn config_display_hides_api_key() {
    let config = AiConfig {
        api_key: "sk-very-secret-key-do-not-leak".to_owned(),
        ..Default::default()
    };
    let display = format!("{config}");
    assert!(!display.contains("sk-very-secret-key-do-not-leak"));
    assert!(display.contains("qwen-plus"));
}

#[test]
fn config_default_uses_qwen_dashscope() {
    let config = AiConfig::default();
    assert!(config.base_url.contains("dashscope.aliyuncs.com"));
    assert_eq!(config.model, "qwen-plus");
    assert_eq!(config.timeout.as_secs(), 30);
    assert_eq!(config.max_tokens, 128);
    assert_eq!(config.temperature, 0.0);
}
