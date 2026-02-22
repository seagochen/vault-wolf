//! Scanner subscription data structures.
//!
//! Ported from: `ScannerSubscription.h`.

use serde::{Deserialize, Serialize};

// ============================================================================
// ScannerSubscription
// ============================================================================

/// Market scanner subscription parameters.
///
/// All C++ sentinel values (`DBL_MAX`, `INT_MAX`) are mapped to `Option<T>`.
///
/// C++ source: `struct ScannerSubscription` in `ScannerSubscription.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerSubscription {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_of_rows: Option<i32>,
    pub instrument: String,
    pub location_code: String,
    pub scan_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub above_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub below_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub above_volume: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_cap_above: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_cap_below: Option<f64>,
    pub moody_rating_above: String,
    pub moody_rating_below: String,
    pub sp_rating_above: String,
    pub sp_rating_below: String,
    pub maturity_date_above: String,
    pub maturity_date_below: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupon_rate_above: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coupon_rate_below: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_convertible: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_option_volume_above: Option<i32>,
    pub scanner_setting_pairs: String,
    pub stock_type_filter: String,
}
