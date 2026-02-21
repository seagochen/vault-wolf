//! Bar (OHLCV) and historical tick data structures.
//!
//! Ported from: `bar.h`, `HistoricalTick.h`, `HistoricalTickBidAsk.h`,
//! `HistoricalTickLast.h`, `HistoricalSession.h`.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::market_data::{TickAttribBidAsk, TickAttribLast};

// ============================================================================
// Bar
// ============================================================================

/// OHLCV bar for historical and real-time data.
///
/// C++ source: `struct Bar` in `bar.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bar {
    pub time: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wap: Option<Decimal>,
    pub count: i32,
}

// ============================================================================
// HistoricalTick
// ============================================================================

/// A single historical tick (trade or midpoint).
///
/// C++ source: `struct HistoricalTick` in `HistoricalTick.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalTick {
    pub time: i64,
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Decimal>,
}

// ============================================================================
// HistoricalTickBidAsk
// ============================================================================

/// A single historical bid/ask tick.
///
/// C++ source: `struct HistoricalTickBidAsk` in `HistoricalTickBidAsk.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalTickBidAsk {
    pub time: i64,
    pub tick_attrib_bid_ask: TickAttribBidAsk,
    pub price_bid: f64,
    pub price_ask: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bid: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_ask: Option<Decimal>,
}

// ============================================================================
// HistoricalTickLast
// ============================================================================

/// A single historical last-trade tick.
///
/// C++ source: `struct HistoricalTickLast` in `HistoricalTickLast.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalTickLast {
    pub time: i64,
    pub tick_attrib_last: TickAttribLast,
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Decimal>,
    pub exchange: String,
    pub special_conditions: String,
}

// ============================================================================
// HistoricalSession
// ============================================================================

/// Trading session schedule entry.
///
/// C++ source: `struct HistoricalSession` in `HistoricalSession.h`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalSession {
    pub start_date_time: String,
    pub end_date_time: String,
    pub ref_date: String,
}
