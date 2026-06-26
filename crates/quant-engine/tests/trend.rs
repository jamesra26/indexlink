// 覆盖 Quant Engine 第二层（20% 趋势）的全量测试边界。
//
// 子模块分组：
// - direction：过渡存根契约与整体方向性
// - indicators：单指标隔离与审计字段
// - regime：节奏体制（TrendRegime）
// - errors：错误传播与边界输入
// - config：配置不变量与默认配置契约

mod common;

mod prelude {
    pub use crate::common::{
        falling_knife_trend_snapshot, make_history, neutral_trend_snapshot,
        overheated_trend_snapshot, standard_history, trend_balanced_test_config,
        trend_config_with_weights, trend_test_percentile_config, CHEAP_SCORE_UPPER_BOUND,
        EXACT_FLOAT_TOLERANCE, EXPENSIVE_SCORE_LOWER_BOUND, MAX_PERCENTILE, MIN_PERCENTILE,
        NEUTRAL_PERCENTILE, NEUTRAL_TOLERANCE, TREND_EQUAL_MA_WEIGHT, TREND_EQUAL_RSI_WEIGHT,
        TREND_EQUAL_VIX_WEIGHT, TREND_FALLING_KNIFE_ABOVE, TREND_MA_ONLY_MA, TREND_MA_ONLY_RSI,
        TREND_MA_ONLY_VIX, TREND_OVERHEATED_ABOVE, TREND_RSI_ONLY_MA, TREND_RSI_ONLY_RSI,
        TREND_RSI_ONLY_VIX, TREND_TEST_HALF_LIFE, TREND_VIX_ONLY_MA, TREND_VIX_ONLY_RSI,
        TREND_VIX_ONLY_VIX,
    };

    #[allow(deprecated)]
    pub use quant_engine::evaluate_trend_stub;
    pub use quant_engine::{
        evaluate_trend, EwPercentileConfig, QuantError, TrendConfig, TrendRegime, TrendSnapshot,
        TrendWeights,
    };
}

#[path = "trend/config.rs"]
mod config;
#[path = "trend/direction.rs"]
mod direction;
#[path = "trend/errors.rs"]
mod errors;
#[path = "trend/indicators.rs"]
mod indicators;
#[path = "trend/regime.rs"]
mod regime;
