// quant-engine 集成测试共享夹具：统一领域阈值、历史序列与测试配置。

#![allow(dead_code)]

use quant_engine::{EwPercentileConfig, FundamentalConfig};

/// 测试中常用的标准历史长度（便于构造精确百分位）。
pub const STANDARD_HISTORY_LEN: usize = 100;
/// 默认配置契约：5 年月度数据。
pub const DEFAULT_MIN_HISTORY_LEN: usize = 60;
/// 默认配置契约：指数加权分位半衰期为 36 个月。
pub const DEFAULT_HALF_LIFE_MONTHS: f64 = 36.0;
/// 集成测试使用的低门槛历史长度，避免夹具过重。
pub const TEST_MIN_HISTORY_LEN: usize = 10;
/// 集成测试使用的较短半衰期，便于构造可读的加权分位场景。
pub const TEST_HALF_LIFE_MONTHS: f64 = 12.0;

/// 第一层默认均衡权重。
pub const BALANCED_CAPE_WEIGHT: f64 = 0.50;
/// 仅使用 CAPE 维度。
pub const CAPE_ONLY_WEIGHT: f64 = 1.0;
/// 仅使用倒置后的 ERP 维度。
pub const ERP_ONLY_WEIGHT: f64 = 0.0;

/// 位置语言中的中性分位。
pub const NEUTRAL_PERCENTILE: f64 = 0.50;
/// 中性场景允许的近似误差。
pub const NEUTRAL_TOLERANCE: f64 = 0.05;
/// 精确分位断言允许的浮点误差。
pub const EXACT_FLOAT_TOLERANCE: f64 = 1e-9;

/// 低位 / 便宜区域的测试上界。
pub const CHEAP_SCORE_UPPER_BOUND: f64 = 0.20;
/// 高位 / 贵区域的测试下界。
pub const EXPENSIVE_SCORE_LOWER_BOUND: f64 = 0.80;
/// 极低原始分位的测试上界。
pub const VERY_LOW_PERCENTILE_UPPER_BOUND: f64 = 0.10;

/// 分位值边界。
pub const MIN_PERCENTILE: f64 = 0.0;
pub const MAX_PERCENTILE: f64 = 1.0;

pub fn make_history(n: usize) -> Vec<f64> {
    (1..=n).map(|i| i as f64).collect()
}

pub fn standard_history() -> Vec<f64> {
    make_history(STANDARD_HISTORY_LEN)
}

pub fn test_percentile_config() -> EwPercentileConfig {
    EwPercentileConfig::from_half_life(TEST_HALF_LIFE_MONTHS, TEST_MIN_HISTORY_LEN)
        .expect("测试半衰期与历史长度有效")
}

pub fn test_config(cape_weight: f64) -> FundamentalConfig {
    FundamentalConfig::new(cape_weight, test_percentile_config()).expect("测试配置有效")
}

pub fn balanced_test_config() -> FundamentalConfig {
    test_config(BALANCED_CAPE_WEIGHT)
}
