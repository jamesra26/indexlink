use crate::prelude::*;

#[test]
fn regime_is_overheated_when_ma_above_threshold() {
    // MA 原始分位 > overheated_above 时，regime 应为 Overheated。
    let history = standard_history();
    let snapshot = TrendSnapshot {
        ma_distance_history: history.clone(),
        ma_distance_current: 9999.0, // 原始分位 → 1.0 > 0.90
        rsi_history: history.clone(),
        rsi_current: 50.0, // 中性
        vix_history: history.clone(),
        vix_current: 0.0001, // 极低，不触发接飞刀
    };
    let config = trend_balanced_test_config();

    let signal = evaluate_trend(&snapshot, &config).unwrap();
    assert_eq!(
        signal.regime,
        TrendRegime::Overheated,
        "MA 分位极高时 regime 应为 Overheated"
    );
}

#[test]
fn regime_is_overheated_when_rsi_above_threshold() {
    // RSI 原始分位 > overheated_above 时，regime 应为 Overheated。
    let history = standard_history();
    let snapshot = TrendSnapshot {
        ma_distance_history: history.clone(),
        ma_distance_current: 0.0001, // 极低，不触发
        rsi_history: history.clone(),
        rsi_current: 9999.0, // 原始分位 → 1.0 > 0.90
        vix_history: history.clone(),
        vix_current: 0.0001,
    };
    let config = trend_balanced_test_config();

    let signal = evaluate_trend(&snapshot, &config).unwrap();
    assert_eq!(
        signal.regime,
        TrendRegime::Overheated,
        "RSI 分位极高时 regime 应为 Overheated"
    );
}

#[test]
fn regime_is_falling_knife_when_vix_above_threshold() {
    // VIX 原始分位 > falling_knife_above 时，regime 应为 FallingKnife。
    let snapshot = falling_knife_trend_snapshot();
    let config = trend_balanced_test_config();

    let signal = evaluate_trend(&snapshot, &config).unwrap();
    assert_eq!(
        signal.regime,
        TrendRegime::FallingKnife,
        "VIX 分位极高时 regime 应为 FallingKnife"
    );
}

#[test]
fn regime_falling_knife_takes_precedence_over_overheated() {
    // 当 MA/RSI 触发 Overheated 且 VIX 同时触发 FallingKnife 时，
    // 应优先判定为 FallingKnife（急跌优先原则），避免遗漏保守信号。
    let history = standard_history();
    let snapshot = TrendSnapshot {
        ma_distance_history: history.clone(),
        ma_distance_current: 9999.0, // 触发 Overheated
        rsi_history: history.clone(),
        rsi_current: 9999.0,
        vix_history: history.clone(),
        vix_current: 9999.0, // 同时触发 FallingKnife
    };
    let config = trend_balanced_test_config();

    let signal = evaluate_trend(&snapshot, &config).unwrap();
    assert_eq!(
        signal.regime,
        TrendRegime::FallingKnife,
        "Overheated 与 FallingKnife 同时满足时应优先 FallingKnife"
    );
}

#[test]
fn regime_is_neutral_when_all_indicators_are_moderate() {
    // 三指标均处于历史中位，均不触发体制阈值，regime 应为 Neutral。
    let snapshot = neutral_trend_snapshot();
    let config = trend_balanced_test_config();

    let signal = evaluate_trend(&snapshot, &config).unwrap();
    assert_eq!(
        signal.regime,
        TrendRegime::Neutral,
        "中性场景下 regime 应为 Neutral"
    );
}
