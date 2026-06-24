//! Sentiment newtype 集成测试。
//!
//! 验证构造器、clamp 边界行为、格式化和安全降级路径。

use ai_client::Sentiment;

#[test]
fn sentiment_neutral_is_zero() {
    assert_eq!(Sentiment::neutral().value(), 0.0);
    assert_eq!(Sentiment::neutral(), Sentiment::NEUTRAL);
}

#[test]
fn sentiment_new_validates_strictly() {
    assert!(Sentiment::new(-1.0).is_some());
    assert!(Sentiment::new(0.0).is_some());
    assert!(Sentiment::new(1.0).is_some());
    assert!(Sentiment::new(f64::NAN).is_none());
    assert!(Sentiment::new(-100.0).is_none());
    assert!(Sentiment::new(100.0).is_none());
}

#[test]
fn sentiment_new_clamped_is_permissive() {
    // 合法值不变
    assert_eq!(Sentiment::new_clamped(0.5).value(), 0.5);
    assert_eq!(Sentiment::new_clamped(-0.3).value(), -0.3);

    // 越界 clamp
    assert_eq!(Sentiment::new_clamped(5.0), Sentiment::MAX);
    assert_eq!(Sentiment::new_clamped(-5.0), Sentiment::MIN);
    assert_eq!(Sentiment::new_clamped(f64::INFINITY), Sentiment::MAX);
    assert_eq!(Sentiment::new_clamped(f64::NEG_INFINITY), Sentiment::MIN);

    // NaN 降级
    assert_eq!(Sentiment::new_clamped(f64::NAN), Sentiment::NEUTRAL);
}

#[test]
fn sentiment_display_format() {
    assert_eq!(Sentiment::NEUTRAL.to_string(), "0.0");
    assert_eq!(Sentiment::MAX.to_string(), "+1.0");
    assert_eq!(Sentiment::MIN.to_string(), "-1.0");
    assert_eq!(
        Sentiment::new(0.75).unwrap().to_string(),
        "+0.8" // 0.75 rounds to 0.8 with one decimal
    );
}

#[test]
fn sentiment_partial_ord() {
    let neg = Sentiment::new_clamped(-0.8);
    let zero = Sentiment::neutral();
    let pos = Sentiment::new_clamped(0.3);

    assert!(neg < zero);
    assert!(zero < pos);
    assert!(neg < pos);
    assert!(pos > neg);
}

#[test]
fn sentiment_into_f64_roundtrip() {
    let original = Sentiment::new(0.42).unwrap();
    let raw: f64 = original.into();
    assert_eq!(raw, 0.42);
    let roundtripped = Sentiment::new_clamped(raw);
    assert_eq!(roundtripped, original);
}

#[test]
fn sentiment_neutral_is_default_degradation_target() {
    // 验证退化路径的正确目标值
    assert_eq!(Sentiment::neutral().value(), 0.0);

    // 模拟退化场景：解析失败 → neutral
    let result: Result<Sentiment, &str> = Err("模拟 LLM 不可用");
    let safe = result.unwrap_or_else(|_| Sentiment::neutral());
    assert_eq!(safe, Sentiment::NEUTRAL);
}
