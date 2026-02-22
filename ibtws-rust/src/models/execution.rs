//! Execution and commission data structures.
//!
//! Ported from: `ibtws-cpp/client/Execution.h`, `CommissionAndFeesReport.h`.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::enums::OptionExerciseType;

// ============================================================================
// Execution
// ============================================================================

/// Details of a trade execution (fill).
///
/// C++ source: `struct Execution` in `Execution.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub exec_id: String,
    pub time: String,
    pub acct_number: String,
    pub exchange: String,
    /// "BUY", "SELL", or "SSHORT".
    pub side: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares: Option<Decimal>,
    pub price: f64,
    pub perm_id: i64,
    pub client_id: i64,
    pub order_id: i64,
    /// 0 = normal, 1 = liquidation.
    pub liquidation: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cum_qty: Option<Decimal>,
    pub avg_price: f64,
    pub order_ref: String,
    pub ev_rule: String,
    pub ev_multiplier: f64,
    pub model_code: String,
    pub last_liquidity: i32,
    pub pending_price_revision: bool,
    pub submitter: String,
    pub opt_exercise_or_lapse_type: OptionExerciseType,
}

// ============================================================================
// ExecutionFilter
// ============================================================================

/// Filter criteria for `reqExecutions`.
///
/// C++ source: `struct ExecutionFilter` in `Execution.h`.
/// Note: C++ uses `m_` prefix for members; Rust drops it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionFilter {
    pub client_id: i64,
    pub acct_code: String,
    pub time: String,
    pub symbol: String,
    pub sec_type: String,
    pub exchange: String,
    pub side: String,
    /// C++ default: `UNSET_INTEGER`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_n_days: Option<i32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub specific_dates: Vec<i64>,
}

// ============================================================================
// CommissionAndFeesReport
// ============================================================================

/// Commission and fees for an execution.
///
/// C++ source: `struct CommissionAndFeesReport` in `CommissionAndFeesReport.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommissionAndFeesReport {
    pub exec_id: String,
    pub commission_and_fees: f64,
    pub currency: String,
    pub realized_pnl: f64,
    pub r#yield: f64,
    /// YYYYMMDD format.
    pub yield_redemption_date: i32,
}
