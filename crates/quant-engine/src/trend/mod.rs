//! 第二层（20% 趋势）评估。
//!
//! 当前仅提供中性存根，后续接入 200 日均线、RSI、VIX 等节奏指标。

use core_domain::Percentile;

/// 第二层（20% 趋势）的评估结果。
///
/// 当前为存根：始终返回中性得分 `0.5`，待后续迭代接入 200 日均线/RSI/VIX。
#[derive(Debug, Clone)]
pub struct TrendSignal {
    /// 趋势综合得分（0.0 = 强势上涨/赶顶风险，1.0 = 强势下跌/接飞刀风险）。
    pub score: Percentile,
}

/// 第二层（20% 趋势）评估的占位实现，始终返回中性。
///
/// 注意：此函数是存根，返回值不应用于实盘。
pub fn evaluate_trend_stub() -> TrendSignal {
    TrendSignal {
        score: Percentile::new(0.5).unwrap(),
    }
}
