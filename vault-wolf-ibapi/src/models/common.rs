//! Shared/utility data structures used across the IB API.
//!
//! Ported from: `TagValue.h`, `SoftDollarTier.h`, `FamilyCode.h`, `NewsProvider.h`,
//! `HistogramEntry.h`, `PriceIncrement.h`, `IneligibilityReason.h`, `WshEventData.h`.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ============================================================================
// TagValue
// ============================================================================

/// Generic key-value pair used for algo parameters, misc options, etc.
///
/// C++ source: `struct TagValue` in `TagValue.h`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagValue {
    pub tag: String,
    pub value: String,
}

impl TagValue {
    pub fn new(tag: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            value: value.into(),
        }
    }
}

// ============================================================================
// SoftDollarTier
// ============================================================================

/// Soft dollar tier for institutional orders.
///
/// C++ source: `class SoftDollarTier` in `SoftDollarTier.h`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftDollarTier {
    pub name: String,
    pub val: String,
    pub display_name: String,
}

// ============================================================================
// FamilyCode
// ============================================================================

/// Family code linking accounts under the same family.
///
/// C++ source: `struct FamilyCode` in `FamilyCode.h`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FamilyCode {
    pub account_id: String,
    pub family_code_str: String,
}

// ============================================================================
// NewsProvider
// ============================================================================

/// News provider information.
///
/// C++ source: `struct NewsProvider` in `NewsProvider.h`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsProvider {
    pub provider_code: String,
    pub provider_name: String,
}

// ============================================================================
// HistogramEntry
// ============================================================================

/// A single entry in histogram data.
///
/// C++ source: `struct HistogramEntry` in `HistogramEntry.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistogramEntry {
    pub price: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Decimal>,
}

// ============================================================================
// PriceIncrement
// ============================================================================

/// Price increment rule defining tick sizes for price ranges.
///
/// C++ source: `struct PriceIncrement` in `PriceIncrement.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceIncrement {
    pub low_edge: f64,
    pub increment: f64,
}

// ============================================================================
// IneligibilityReason
// ============================================================================

/// Reason why a contract is ineligible for certain operations.
///
/// C++ source: `struct IneligibilityReason` in `IneligibilityReason.h`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IneligibilityReason {
    pub id: String,
    pub description: String,
}

// ============================================================================
// WshEventData
// ============================================================================

/// Wall Street Horizon event data request parameters.
///
/// C++ source: `struct WshEventData` in `WshEventData.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WshEventData {
    pub con_id: i32,
    pub filter: String,
    pub fill_watchlist: bool,
    pub fill_portfolio: bool,
    pub fill_competitors: bool,
    pub start_date: String,
    pub end_date: String,
    pub total_limit: i32,
}

// ============================================================================
// SmartComponent
// ============================================================================

/// Smart routing component information.
///
/// C++ uses: `std::map<int, std::tuple<std::string, char>>` in EWrapper callbacks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartComponent {
    pub bit_number: i32,
    pub exchange: String,
    pub exchange_letter: char,
}
