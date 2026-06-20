//! 历史分位计算工具。
//!
//! 本模块提供跨 Quant Engine 各层共享的「历史位置」计算能力。

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
