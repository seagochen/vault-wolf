//! VaultWolf data models.
//!
//! These types mirror the C++ structs from `include/common/data_types.h`,
//! with `serde` derive for automatic JSON serialization.

use serde::{Deserialize, Serialize};

// ============================================================================
// Market Data
// ============================================================================

/// Real-time tick data for stocks/options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TickData {
    pub symbol: String,
    pub sec_type: String,
    pub req_id: i64,

    // Prices
    pub bid: f64,
    pub ask: f64,
    pub last: f64,
    pub close: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,

    // Sizes
    pub bid_size: i64,
    pub ask_size: i64,
    pub last_size: i64,
    pub volume: i64,

    // Option greeks (only populated for options)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implied_vol: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamma: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vega: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theta: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opt_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pv_dividend: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub und_price: Option<f64>,

    pub timestamp: String,
}

/// Historical bar data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalBar {
    pub date: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
    pub bar_count: i32,
    pub wap: f64,
}

/// Historical data response (collection of bars).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalData {
    pub symbol: String,
    pub sec_type: String,
    pub req_id: i64,
    pub start_date: String,
    pub end_date: String,
    pub bars: Vec<HistoricalBar>,
}

// ============================================================================
// Account Data
// ============================================================================

/// Account summary information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountSummary {
    pub account: String,
    pub values: std::collections::HashMap<String, String>,
}

/// Position information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub account: String,
    pub symbol: String,
    pub sec_type: String,
    pub currency: String,
    pub exchange: String,
    pub position: f64,
    pub avg_cost: f64,
    pub market_price: f64,
    pub market_value: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
}

// ============================================================================
// Order Data
// ============================================================================

/// Order information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInfo {
    pub order_id: i64,
    pub account: String,
    pub symbol: String,
    pub sec_type: String,
    pub exchange: String,
    pub currency: String,

    // Order details
    pub action: String,
    pub order_type: String,
    pub total_quantity: f64,
    pub lmt_price: f64,
    pub aux_price: f64,

    // Status
    pub status: String,
    pub filled: f64,
    pub remaining: f64,
    pub avg_fill_price: f64,
    pub perm_id: i64,
    pub parent_id: i64,
    pub last_fill_price: f64,

    // Option-specific
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<String>,

    pub submit_time: String,
    pub last_update_time: String,
}

// ============================================================================
// Contract Specification (for API requests)
// ============================================================================

/// Simple contract specification used for API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractSpec {
    pub symbol: String,
    #[serde(default = "default_sec_type")]
    pub sec_type: String,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default = "default_exchange")]
    pub exchange: String,

    // For options
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<String>,

    // For futures
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_trade_date_or_contract_month: Option<String>,
}

fn default_sec_type() -> String {
    "STK".to_string()
}
fn default_currency() -> String {
    "USD".to_string()
}
fn default_exchange() -> String {
    "SMART".to_string()
}

impl Default for ContractSpec {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            sec_type: default_sec_type(),
            currency: default_currency(),
            exchange: default_exchange(),
            right: None,
            strike: None,
            expiry: None,
            last_trade_date_or_contract_month: None,
        }
    }
}

// ============================================================================
// API Response Wrappers
// ============================================================================

/// Generic JSON API response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<i32>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(message: impl Into<String>, data: T) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
            error_code: None,
        }
    }
}

impl ApiResponse<()> {
    pub fn success_msg(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
            error_code: None,
        }
    }

    pub fn error(message: impl Into<String>, code: i32) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
            error_code: Some(code),
        }
    }
}
