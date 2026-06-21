// 覆盖 Quant Engine 第一层（70% 基本面）的 CAPE/ERP 分位合成、审计字段与边界行为。

mod common;

use common::{
    balanced_test_config, make_history, standard_history, test_config, test_percentile_config,
    BALANCED_CAPE_WEIGHT, CAPE_ONLY_WEIGHT, CHEAP_SCORE_UPPER_BOUND, DEFAULT_HALF_LIFE_MONTHS,
    DEFAULT_MIN_HISTORY_LEN, ERP_ONLY_WEIGHT, EXACT_FLOAT_TOLERANCE, EXPENSIVE_SCORE_LOWER_BOUND,
    MAX_PERCENTILE, MIN_PERCENTILE, NEUTRAL_PERCENTILE, NEUTRAL_TOLERANCE,
    VERY_LOW_PERCENTILE_UPPER_BOUND,
};
use quant_engine::{
    evaluate_fundamental, weighted_percentile_of, EwPercentileConfig, FundamentalConfig,
    FundamentalSnapshot, QuantError, Weight,
};

#[test]
fn fundamental_expensive_market() {
    // 极端贵：CAPE 高于全部历史（加权分位 → 1.0），ERP 低于全部历史
    // （加权分位 → 0.0，倒置后 → 1.0）。方向性对任意半衰期都稳健。
    let cape_h: Vec<f64> = (1..=100).map(|i| i as f64 * 10.0).collect();
    let erp_h: Vec<f64> = (1..=100).map(|i| i as f64 * 0.1).collect();
    let cfg = balanced_test_config();

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: 100_000.0,
        erp_history: erp_h,
        erp_current: 0.001,
    };

    let sig = evaluate_fundamental(&snapshot, &cfg).unwrap();
    assert!(
        sig.score.value() > EXPENSIVE_SCORE_LOWER_BOUND,
        "贵市场得分应 > {EXPENSIVE_SCORE_LOWER_BOUND:.2}，实际 {}",
        sig.score
    );
}

#[test]
fn fundamental_cheap_market() {
    // 极端便宜：CAPE 低于全部历史（加权分位 → 0.0），ERP 高于全部历史
    // （加权分位 → 1.0，倒置后 → 0.0）。方向性对任意半衰期都稳健。
    let cape_h: Vec<f64> = (1..=100).map(|i| i as f64 * 10.0).collect();
    let erp_h: Vec<f64> = (1..=100).map(|i| i as f64 * 0.1).collect();
    let cfg = balanced_test_config();

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: 1.0,
        erp_history: erp_h,
        erp_current: 100.0,
    };

    let sig = evaluate_fundamental(&snapshot, &cfg).unwrap();
    assert!(
        sig.score.value() < CHEAP_SCORE_UPPER_BOUND,
        "便宜市场得分应 < {CHEAP_SCORE_UPPER_BOUND:.2}，实际 {}",
        sig.score
    );
}

#[test]
fn fundamental_neutral_market() {
    let cape_h = standard_history();
    let erp_h = standard_history();
    let cfg = FundamentalConfig::default();

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: 71.0,
        erp_history: erp_h,
        erp_current: 71.0,
    };

    let sig = evaluate_fundamental(&snapshot, &cfg).unwrap();
    assert!(
        (sig.score.value() - NEUTRAL_PERCENTILE).abs() < NEUTRAL_TOLERANCE,
        "中性市场得分应 ≈ 0.50，实际 {}",
        sig.score
    );
}

#[test]
fn rate_repricing_low_erp_pushes_score_expensive_despite_neutral_cape() {
    // 金融场景：利率重估。CAPE 处于历史中性（≈0.50），但利率抬升压缩了风险补偿，
    // 使 ERP 跌至历史极低位。预期：尽管估值锚中性，整体仍应偏贵
    // 因为低 ERP 倒置后变高，把综合得分推向"贵"。这验证 ERP 倒置语义在
    // 两个维度"背离"时仍正确生效（既有方向性测试均为两维同向，未覆盖此处）。
    let cape_h = standard_history();
    let erp_h = standard_history();
    let cfg = balanced_test_config();

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: 88.0, // 加权 CAPE 分位 ≈ 0.50（中性）
        erp_history: erp_h,
        erp_current: 5.0, // 加权 ERP 分位处于历史极低位，风险补偿被压缩
    };

    let sig = evaluate_fundamental(&snapshot, &cfg).unwrap();

    // CAPE 维度确实中性（加权 ECDF 因截断尾项无法精确等于 0.5，用近似容差）。
    assert!(
        (sig.cape_percentile.value() - NEUTRAL_PERCENTILE).abs() < NEUTRAL_TOLERANCE,
        "CAPE 应处于历史中性，实际 {}",
        sig.cape_percentile
    );
    // ERP 原始分位极低（审计字段未倒置）。
    assert!(
        sig.erp_percentile.value() < VERY_LOW_PERCENTILE_UPPER_BOUND,
        "低 ERP 的原始分位应处于历史极低位，实际 {}",
        sig.erp_percentile
    );
    // 关键断言：低 ERP 单独把综合得分推向偏贵，超过中性的 CAPE。
    assert!(
        sig.score.value() > sig.cape_percentile.value(),
        "低 ERP 应把综合得分推得比中性 CAPE 更贵：score {} 应 > cape_p {}",
        sig.score,
        sig.cape_percentile
    );
    assert!(
        sig.score.value() > NEUTRAL_PERCENTILE,
        "利率重估（低 ERP）下整体应偏贵（> {NEUTRAL_PERCENTILE:.2}），实际 {}",
        sig.score
    );
}

#[test]
fn default_config_matches_readme_contract() {
    // 默认配置应为：CAPE/ERP 各半，指数加权分位半衰期 36 个月，
    // 且至少需要 5 年（60）月度有效历史数据。
    let cfg = FundamentalConfig::default();
    assert_eq!(cfg.cape_weight.value(), BALANCED_CAPE_WEIGHT);
    assert_eq!(cfg.percentile_config.min_len(), DEFAULT_MIN_HISTORY_LEN);

    let expected_alpha = 1.0 - 0.5_f64.powf(1.0 / DEFAULT_HALF_LIFE_MONTHS);
    assert!(
        (cfg.percentile_config.alpha() - expected_alpha).abs() < EXACT_FLOAT_TOLERANCE,
        "默认半衰期 36 个月应映射为 alpha {expected_alpha}，实际 {}",
        cfg.percentile_config.alpha()
    );
}

#[test]
fn weight_display_uses_percent_format_for_audit_logs() {
    let cfg = FundamentalConfig::default();
    assert_eq!(cfg.cape_weight.to_string(), "50.0%");
}

#[test]
fn weight_try_from_accepts_valid_raw_value() {
    let weight = Weight::try_from(BALANCED_CAPE_WEIGHT).unwrap();
    assert_eq!(weight.value(), BALANCED_CAPE_WEIGHT);
}

#[test]
fn weight_converts_into_raw_f64() {
    let weight = Weight::new(BALANCED_CAPE_WEIGHT).unwrap();
    let raw: f64 = weight.into();
    assert_eq!(raw, BALANCED_CAPE_WEIGHT);
}

#[test]
fn erp_percentile_audit_field_is_not_inverted() {
    // FundamentalSignal.erp_percentile 文档声明为"未倒置"（高 ERP = 高分位）。
    // 审计原则：存原始输入分位，倒置只发生在合成 score 内部。
    let cape_h = standard_history();
    let erp_h = standard_history();
    let cfg = balanced_test_config();

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: 10.0,
        erp_history: erp_h,
        erp_current: 9999.0, // 极高 ERP
    };

    let sig = evaluate_fundamental(&snapshot, &cfg).unwrap();
    assert!(
        sig.erp_percentile.value() > EXPENSIVE_SCORE_LOWER_BOUND,
        "高 ERP 的原始分位应处于高位（未倒置），实际 {}",
        sig.erp_percentile
    );
    // 但因为 ERP 越高市场越便宜，倒置后综合得分应偏低。
    assert!(
        sig.score.value() < NEUTRAL_PERCENTILE,
        "高 ERP 应使综合得分偏便宜（< {NEUTRAL_PERCENTILE:.2}），实际 {}",
        sig.score
    );
}

#[test]
fn audit_fields_reflect_raw_percentiles() {
    // 审计字段须如实记录两个维度各自的原始分位，便于回放"为何那天的得分"。
    let cape_h = standard_history();
    let erp_h = standard_history();
    let cfg = balanced_test_config();

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: 0.0,
        erp_history: erp_h,
        erp_current: 9999.0,
    };

    let sig = evaluate_fundamental(&snapshot, &cfg).unwrap();
    assert_eq!(sig.cape_percentile.value(), MIN_PERCENTILE);
    assert_eq!(sig.erp_percentile.value(), MAX_PERCENTILE);
}

#[test]
fn cape_weight_full_uses_only_cape() {
    // cape_weight = 1.0：得分应完全等于 CAPE 分位，忽略 ERP。
    let cape_h = standard_history();
    let erp_h = standard_history();
    let cfg = test_config(CAPE_ONLY_WEIGHT);

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: 80.0,
        erp_history: erp_h,
        erp_current: 10.0, // 任意值都不应影响结果
    };

    let sig = evaluate_fundamental(&snapshot, &cfg).unwrap();
    assert_eq!(
        sig.score.value(),
        sig.cape_percentile.value(),
        "cape_weight=1.0 时得分应完全等于 CAPE 分位"
    );
}

#[test]
fn cape_weight_zero_uses_only_inverted_erp() {
    // cape_weight = 0.0：得分应完全等于倒置后的 ERP 分位。
    let cape_h = standard_history();
    let erp_h = standard_history();
    let cfg = test_config(ERP_ONLY_WEIGHT);

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: 80.0, // 任意值都不应影响结果
        erp_history: erp_h,
        erp_current: 70.0,
    };

    let sig = evaluate_fundamental(&snapshot, &cfg).unwrap();
    let inverted_erp = MAX_PERCENTILE - sig.erp_percentile.value();
    assert!(
        (sig.score.value() - inverted_erp).abs() < EXACT_FLOAT_TOLERANCE,
        "cape_weight=0.0 时得分应等于倒置 ERP 分位 {}，实际 {}",
        inverted_erp,
        sig.score
    );
}

#[test]
fn propagates_insufficient_history_for_circuit_breaker() {
    // 数据不足时应向上传播错误，供熔断/降级链触发默认 Skip。
    let cfg = FundamentalConfig::default(); // min_history_len = 60
    let snapshot = FundamentalSnapshot {
        cape_history: vec![1.0, 2.0, 3.0],
        cape_current: 2.0,
        erp_history: standard_history(),
        erp_current: 50.0,
    };

    let err = evaluate_fundamental(&snapshot, &cfg).unwrap_err();
    assert!(
        matches!(
            err,
            QuantError::InsufficientHistory {
                indicator: "CAPE",
                ..
            }
        ),
        "CAPE 历史不足应传播 InsufficientHistory，实际 {:?}",
        err
    );
}

#[test]
fn propagates_invalid_input_on_nan_current() {
    // 当前读数为 NaN 时应传播 InvalidCurrentValue，绝不静默产出得分。
    let cfg = balanced_test_config();
    let snapshot = FundamentalSnapshot {
        cape_history: standard_history(),
        cape_current: 50.0,
        erp_history: standard_history(),
        erp_current: f64::NAN,
    };

    let err = evaluate_fundamental(&snapshot, &cfg).unwrap_err();
    assert!(
        matches!(
            err,
            QuantError::InvalidCurrentValue {
                indicator: "ERP",
                ..
            }
        ),
        "ERP 当前值为 NaN 应传播 InvalidCurrentValue，实际 {:?}",
        err
    );
}

// ─── 边界：±Inf 当前读数 ───────────────────────────────────────────────────

#[test]
fn infinite_current_returns_invalid_input() {
    // 金融读数必须是有限数；±Inf 更像上游数据管道异常，而非合法极端估值。
    let cape_h = standard_history();
    let erp_h = standard_history();
    let cfg = balanced_test_config();

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: f64::INFINITY,
        erp_history: erp_h,
        erp_current: f64::NEG_INFINITY,
    };

    let err = evaluate_fundamental(&snapshot, &cfg).unwrap_err();
    assert!(
        matches!(
            err,
            QuantError::InvalidCurrentValue {
                indicator: "CAPE",
                ..
            }
        ),
        "非有限当前读数应传播 InvalidCurrentValue，实际 {:?}",
        err
    );
}

// ─── 边界：配置不变量 ───────────────────────────────────────────────────────

#[test]
fn rejects_cape_weight_above_one_at_construction() {
    let err = FundamentalConfig::new(2.0, test_percentile_config()).unwrap_err();
    assert!(
        matches!(err, QuantError::InvalidWeight { .. }),
        "cape_weight > 1.0 应在配置构造期被拒绝，实际 {:?}",
        err
    );
}

#[test]
fn rejects_cape_weight_below_zero_at_construction() {
    let err = FundamentalConfig::new(-1.0, test_percentile_config()).unwrap_err();
    assert!(
        matches!(err, QuantError::InvalidWeight { .. }),
        "cape_weight < 0.0 应在配置构造期被拒绝，实际 {:?}",
        err
    );
}

#[test]
fn rejects_zero_min_history_len_at_construction() {
    let err = EwPercentileConfig::from_half_life(DEFAULT_HALF_LIFE_MONTHS, 0).unwrap_err();
    assert!(
        matches!(err, QuantError::InvalidMinHistoryLen { value: 0 }),
        "min_history_len = 0 应在配置构造期被拒绝，实际 {:?}",
        err
    );
}

// ─── 边界：两个历史序列长度不一致（当前为未定义行为） ────────────────────────

#[test]
fn unequal_history_lengths_are_accepted() {
    // 边界（当前为未定义行为）：FundamentalSnapshot 不要求两个历史序列等长。
    // 各指标分位独立计算，只要各自有效点数 ≥ min_history_len 即成功。
    // 若后续实现层加入等长校验，应改为断言返回错误。
    let cape_h = standard_history(); // 100 点
    let erp_h = make_history(DEFAULT_MIN_HISTORY_LEN); // 60 点
    let cfg = balanced_test_config();

    let snapshot = FundamentalSnapshot {
        cape_history: cape_h,
        cape_current: 50.0,
        erp_history: erp_h,
        erp_current: 30.0,
    };

    let sig = evaluate_fundamental(&snapshot, &cfg).unwrap();
    let expected_cape =
        weighted_percentile_of("CAPE", &snapshot.cape_history, 50.0, &cfg.percentile_config)
            .unwrap();
    let expected_erp =
        weighted_percentile_of("ERP", &snapshot.erp_history, 30.0, &cfg.percentile_config).unwrap();

    assert_eq!(
        sig.cape_percentile.value(),
        expected_cape.value(),
        "CAPE 应按自身 100 点序列加权定位"
    );
    assert_eq!(
        sig.erp_percentile.value(),
        expected_erp.value(),
        "ERP 应按自身 60 点序列加权定位"
    );
}

#[test]
fn unequal_lengths_below_min_propagate_insufficient_for_short_series() {
    // 边界（当前为未定义行为）：不等长且较短序列 < min_history_len 时，
    // 错误应明确指向较短的那个指标（此处为 ERP），而非静默产出得分。
    let percentile_config =
        EwPercentileConfig::from_half_life(DEFAULT_HALF_LIFE_MONTHS, DEFAULT_MIN_HISTORY_LEN)
            .expect("默认半衰期与历史长度有效");
    let cfg = FundamentalConfig::new(BALANCED_CAPE_WEIGHT, percentile_config)
        .expect("测试配置权重与历史长度有效");

    let snapshot = FundamentalSnapshot {
        cape_history: standard_history(), // 充足
        cape_current: 50.0,
        erp_history: make_history(30), // 不足 60
        erp_current: 15.0,
    };

    let err = evaluate_fundamental(&snapshot, &cfg).unwrap_err();
    assert!(
        matches!(
            err,
            QuantError::InsufficientHistory {
                indicator: "ERP",
                found: 30,
                ..
            }
        ),
        "较短的 ERP 序列应触发 InsufficientHistory{{found:30}}，实际 {:?}",
        err
    );
}
