//! 历史分位计算工具。
//!
//! 本模块提供跨 Quant Engine 各层共享的「历史位置」计算能力，
//! 包括无权历史分位 [`percentile_of`] 与指数加权历史分位 [`weighted_percentile_of`]。

use std::num::NonZeroUsize;

use core_domain::Percentile;

use crate::QuantError;

/// 计算 `current` 在 `history` 中的历史分位（百分位排名）。
///
/// 返回历史序列中**小于等于** `current` 的数据点占比。
///
/// # 示例
///
/// ```
/// use quant_engine::percentile_of;
///
/// let history: Vec<f64> = (1..=100).map(|i| i as f64).collect();
/// let p = percentile_of("TEST", &history, 50.0, 10).unwrap();
/// assert!((p.value() - 0.50).abs() < 0.01);
/// ```
///
/// # 错误
///
/// - [`QuantError::InsufficientHistory`]：有效数据点数 < `min_len`
/// - [`QuantError::InvalidMinHistoryLen`]：`min_len` 为 0
/// - [`QuantError::InvalidCurrentValue`]：`current` 不是有限数
pub fn percentile_of(
    indicator: &'static str,
    history: &[f64],
    current: f64,
    min_len: usize,
) -> Result<Percentile, QuantError> {
    if min_len == 0 {
        return Err(QuantError::InvalidMinHistoryLen { value: min_len });
    }

    if !current.is_finite() {
        return Err(QuantError::InvalidCurrentValue {
            indicator,
            value: current,
        });
    }

    // 过滤掉历史中的 NaN（脏数据容错）
    let valid: Vec<f64> = history.iter().copied().filter(|v| !v.is_nan()).collect();

    if valid.len() < min_len {
        return Err(QuantError::InsufficientHistory {
            indicator,
            required: min_len,
            found: valid.len(),
        });
    }

    let count_le = valid.iter().filter(|&&v| v <= current).count();
    let p = count_le as f64 / valid.len() as f64;

    // p 的取值范围必然在 [0.0, 1.0]，unwrap 安全
    Ok(Percentile::new(p).expect("count_le / valid.len() 必然在 [0.0, 1.0]"))
}

/// 指数加权历史分位的配置。
///
/// 以**半衰期**为唯一旋钮控制历史样本的指数衰减，避免硬窗口的「幽灵跌落」，
/// 同时保持纯分位语义（无分布假设）。衰减系数 `alpha` 与半衰期 `H` 的关系为
/// `alpha = 1 - 0.5^(1/H)`，即滞后 `H` 个样本处权重恰好衰减至 `0.5`。
#[derive(Debug, Clone, PartialEq)]
pub struct EwPercentileConfig {
    /// 指数衰减系数 `alpha ∈ (0.0, 1.0]`；越大越偏重近端样本。
    alpha: f64,
    /// 计算分位所需的最少有效（非 NaN）样本数。
    min_len: NonZeroUsize,
}

impl EwPercentileConfig {
    /// 由半衰期构造（推荐入口）。
    ///
    /// `half_life` 的单位必须与数据频率一致（月频数据用月数，日频数据用交易日数）。
    ///
    /// # 错误
    ///
    /// - [`QuantError::InvalidHalfLife`]：`half_life` 非有限数或 ≤ 0
    /// - 其余错误透传自 [`EwPercentileConfig::from_alpha`]
    pub fn from_half_life(half_life: f64, min_len: usize) -> Result<Self, QuantError> {
        if !half_life.is_finite() || half_life <= 0.0 {
            return Err(QuantError::InvalidHalfLife { value: half_life });
        }

        let alpha = 1.0 - 0.5_f64.powf(1.0 / half_life);
        Self::from_alpha(alpha, min_len)
    }

    /// 直接由衰减系数 `alpha ∈ (0.0, 1.0]` 构造。
    ///
    /// # 错误
    ///
    /// - [`QuantError::InvalidDecay`]：`alpha` 非有限数或不在 `(0.0, 1.0]`
    /// - [`QuantError::InvalidMinHistoryLen`]：`min_len` 为 0
    pub fn from_alpha(alpha: f64, min_len: usize) -> Result<Self, QuantError> {
        if !alpha.is_finite() || alpha <= 0.0 || alpha > 1.0 {
            return Err(QuantError::InvalidDecay { alpha });
        }

        let min_len = NonZeroUsize::new(min_len)
            .ok_or(QuantError::InvalidMinHistoryLen { value: min_len })?;

        Ok(Self { alpha, min_len })
    }

    /// 返回衰减系数 `alpha`。
    #[must_use]
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// 返回最少有效样本数。
    #[must_use]
    pub fn min_len(&self) -> usize {
        self.min_len.get()
    }
}

/// 计算 `current` 在 `history` 中的**指数加权**历史分位。
///
/// `history` 必须按时间顺序排列：索引 `0` 为最旧、末尾为最新；越新的样本权重越高
/// （最新样本滞后 `0`、权重 `1`，每向旧一步权重乘以 `1 - alpha`）。返回加权后
/// 「小于等于 `current`」的样本占比，落在 `[0.0, 1.0]`。
///
/// NaN 样本会被跳过，但**不压缩滞后**，以保持权重对应真实时间距离。
///
/// # 错误
///
/// - [`QuantError::InvalidCurrentValue`]：`current` 不是有限数
/// - [`QuantError::InsufficientHistory`]：有效样本数 < `min_len`，或全部有效权重下溢为 0
pub fn weighted_percentile_of(
    indicator: &'static str,
    history: &[f64],
    current: f64,
    config: &EwPercentileConfig,
) -> Result<Percentile, QuantError> {
    if !current.is_finite() {
        return Err(QuantError::InvalidCurrentValue {
            indicator,
            value: current,
        });
    }

    let one_minus_alpha = 1.0 - config.alpha;
    let mut total_weight = 0.0_f64;
    let mut le_weight = 0.0_f64;
    let mut valid = 0usize;

    // 从最新（末尾）向最旧（开头）遍历；NaN 跳过但不压缩滞后。
    let mut weight = 1.0_f64;
    for &x in history.iter().rev() {
        if !x.is_nan() {
            total_weight += weight;
            if x <= current {
                le_weight += weight;
            }
            valid += 1;
        }
        weight *= one_minus_alpha;
    }

    if valid < config.min_len.get() {
        return Err(QuantError::InsufficientHistory {
            indicator,
            required: config.min_len.get(),
            found: valid,
        });
    }

    // 极端长历史下，远端有效样本的权重可能全部下溢为 0。
    if total_weight <= 0.0 {
        return Err(QuantError::InsufficientHistory {
            indicator,
            required: config.min_len.get(),
            found: 0,
        });
    }

    let p = le_weight / total_weight;

    // le_weight 是 total_weight 的子集累加，比值必然落在 [0.0, 1.0]
    Ok(Percentile::new(p).expect("加权 ECDF 比值必然落在 [0.0, 1.0]"))
}
