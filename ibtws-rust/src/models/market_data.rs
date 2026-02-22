//! Market data tick attributes and depth descriptions.
//!
//! Ported from: `TickAttrib.h`, `TickAttribBidAsk.h`, `TickAttribLast.h`,
//! `DepthMktDataDescription.h`.

use serde::{Deserialize, Serialize};

// ============================================================================
// TickAttrib
// ============================================================================

/// Tick attributes for price ticks.
///
/// C++ source: `struct TickAttrib` in `TickAttrib.h`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TickAttrib {
    pub can_auto_execute: bool,
    pub past_limit: bool,
    pub pre_open: bool,
}

// ============================================================================
// TickAttribBidAsk
// ============================================================================

/// Tick attributes for bid/ask ticks.
///
/// C++ source: `struct TickAttribBidAsk` in `TickAttribBidAsk.h`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TickAttribBidAsk {
    pub bid_past_low: bool,
    pub ask_past_high: bool,
}

// ============================================================================
// TickAttribLast
// ============================================================================

/// Tick attributes for last-trade ticks.
///
/// C++ source: `struct TickAttribLast` in `TickAttribLast.h`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TickAttribLast {
    pub past_limit: bool,
    pub unreported: bool,
}

// ============================================================================
// DepthMktDataDescription
// ============================================================================

/// Description of available market depth data for an exchange.
///
/// C++ source: `struct DepthMktDataDescription` in `DepthMktDataDescription.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepthMktDataDescription {
    pub exchange: String,
    pub sec_type: String,
    pub listing_exch: String,
    pub service_data_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agg_group: Option<i32>,
}
