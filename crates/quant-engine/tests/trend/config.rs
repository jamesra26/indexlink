use crate::prelude::*;

// ═══════════════════════════════════════════════════════════════════════════
// 配置不变量（构造期校验）
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn rejects_weight_sum_not_equal_to_one() {
    // 三权重之和 ≠ 1.0 应在 TrendWeights::new 构造时被拒绝。
    let err = TrendWeights::new(0.5, 0.3, 0.3).unwrap_err(); // 和 = 1.1
    assert!(
        matches!(err, QuantError::InvalidWeight { .. }),
        "权重和 ≠ 1.0 应返回 InvalidWeight，实际 {:?}",
        err
    );
}

#[test]
fn rejects_individual_weight_above_one() {
    // 单权重 > 1.0 应在 Weight::new 内部被拦截。
    let err = TrendWeights::new(1.5, 0.0, 0.0).unwrap_err();
    assert!(
        matches!(err, QuantError::InvalidWeight { .. }),
        "单权重 > 1.0 应返回 InvalidWeight，实际 {:?}",
        err
    );
}

#[test]
fn rejects_individual_weight_below_zero() {
    let err = TrendWeights::new(-0.1, 0.6, 0.5).unwrap_err();
    assert!(
        matches!(err, QuantError::InvalidWeight { .. }),
        "单权重 < 0.0 应返回 InvalidWeight，实际 {:?}",
        err
    );
}

#[test]
fn rejects_zero_min_history_len() {
    // min_len = 0 应在 EwPercentileConfig 构造期被拒绝。
    let err = EwPercentileConfig::from_half_life(TREND_TEST_HALF_LIFE, 0).unwrap_err();
    assert!(
        matches!(err, QuantError::InvalidMinHistoryLen { value: 0 }),
        "min_len = 0 应返回 InvalidMinHistoryLen，实际 {:?}",
        err
    );
}

#[test]
fn rejects_invalid_overheated_threshold() {
    // overheated_above 超出 [0.0, 1.0] 应在 TrendConfig::new 被拒绝。
    let weights = TrendWeights::new(
        TREND_EQUAL_MA_WEIGHT,
        TREND_EQUAL_RSI_WEIGHT,
        TREND_EQUAL_VIX_WEIGHT,
    )
    .unwrap();
    let err = TrendConfig::new(
        weights,
        trend_test_percentile_config(),
        1.5, // 超界
        TREND_FALLING_KNIFE_ABOVE,
    )
    .unwrap_err();
    assert!(
        matches!(err, QuantError::InvalidWeight { .. }),
        "overheated_above 超界应返回错误，实际 {:?}",
        err
    );
}

#[test]
fn rejects_nan_weight() {
    let err = TrendWeights::new(f64::NAN, 0.5, 0.5).unwrap_err();
    assert!(
        matches!(err, QuantError::InvalidWeight { .. }),
        "NaN 权重应返回 InvalidWeight，实际 {:?}",
        err
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 默认配置契约（与设计文档约定对齐）
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn default_weights_match_design_constants() {
    // 默认子权重：MA=0.4, RSI=0.3, VIX=0.3，文档约定。
    let config = TrendConfig::default();
    assert!(
        (config.weights.ma_weight.value() - 0.4).abs() < EXACT_FLOAT_TOLERANCE,
        "默认 MA 权重应为 0.4，实际 {}",
        config.weights.ma_weight
    );
    assert!(
        (config.weights.rsi_weight.value() - 0.3).abs() < EXACT_FLOAT_TOLERANCE,
        "默认 RSI 权重应为 0.3，实际 {}",
        config.weights.rsi_weight
    );
    assert!(
        (config.weights.vix_weight.value() - 0.3).abs() < EXACT_FLOAT_TOLERANCE,
        "默认 VIX 权重应为 0.3，实际 {}",
        config.weights.vix_weight
    );
}

#[test]
fn default_thresholds_match_design_constants() {
    // 默认过热/接飞刀阈值均为 0.90。
    let config = TrendConfig::default();
    assert!(
        (config.overheated_above.value() - TREND_OVERHEATED_ABOVE).abs() < EXACT_FLOAT_TOLERANCE,
        "默认 overheated_above 应为 {TREND_OVERHEATED_ABOVE}"
    );
    assert!(
        (config.falling_knife_above.value() - TREND_FALLING_KNIFE_ABOVE).abs()
            < EXACT_FLOAT_TOLERANCE,
        "默认 falling_knife_above 应为 {TREND_FALLING_KNIFE_ABOVE}"
    );
}

#[test]
fn default_config_min_len_matches_design() {
    // 默认最少历史长度 252（约 1 年交易日）。
    let config = TrendConfig::default();
    assert_eq!(
        config.percentile_config.min_len(),
        252,
        "默认 min_len 应为 252"
    );
}

#[test]
fn default_half_life_matches_design() {
    // 默认半衰期 36，alpha 应满足 α = 1 − 0.5^(1/36)。
    let config = TrendConfig::default();
    let expected_alpha = 1.0 - 0.5_f64.powf(1.0 / 36.0);
    assert!(
        (config.percentile_config.alpha() - expected_alpha).abs() < EXACT_FLOAT_TOLERANCE,
        "默认半衰期 36 应映射为 alpha {expected_alpha:.6}，实际 {}",
        config.percentile_config.alpha()
    );
}
