//! Contract-related data structures.
//!
//! Ported from: `cppclient/client/Contract.h`.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::common::{IneligibilityReason, TagValue};
use super::enums::*;

// ============================================================================
// ComboLeg
// ============================================================================

/// A single leg of a combo/spread order.
///
/// C++ source: `struct ComboLeg` in `Contract.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComboLeg {
    pub con_id: i64,
    pub ratio: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    pub exchange: String,
    pub open_close: LegOpenClose,
    /// 1 = clearing broker, 2 = third party.
    pub short_sale_slot: i32,
    pub designated_location: String,
    /// C++ default: -1.
    pub exempt_code: i32,
}

impl Default for ComboLeg {
    fn default() -> Self {
        Self {
            con_id: 0,
            ratio: 0,
            action: None,
            exchange: String::new(),
            open_close: LegOpenClose::Same,
            short_sale_slot: 0,
            designated_location: String::new(),
            exempt_code: -1,
        }
    }
}

// ============================================================================
// DeltaNeutralContract
// ============================================================================

/// Delta neutral contract info attached to a combo.
///
/// C++ source: `struct DeltaNeutralContract` in `Contract.h`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeltaNeutralContract {
    pub con_id: i64,
    pub delta: f64,
    pub price: f64,
}

// ============================================================================
// Contract
// ============================================================================

/// Defines a financial instrument (stock, option, future, forex, etc.).
///
/// C++ source: `struct Contract` in `Contract.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub con_id: i64,
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_type: Option<SecType>,
    pub last_trade_date_or_contract_month: String,
    pub last_trade_date: String,
    /// C++ default: `UNSET_DOUBLE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<Right>,
    pub multiplier: String,
    pub exchange: String,
    pub primary_exchange: String,
    pub currency: String,
    pub local_symbol: String,
    pub trading_class: String,
    pub include_expired: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_id_type: Option<SecIdType>,
    pub sec_id: String,
    pub description: String,
    pub issuer_id: String,
    pub combo_legs_descrip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combo_legs: Option<Vec<ComboLeg>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_neutral_contract: Option<DeltaNeutralContract>,
}

// ============================================================================
// ContractDetails
// ============================================================================

/// Extended contract information returned by `reqContractDetails`.
///
/// C++ source: `struct ContractDetails` in `Contract.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractDetails {
    pub contract: Contract,
    pub market_name: String,
    pub min_tick: f64,
    pub order_types: String,
    pub valid_exchanges: String,
    pub price_magnifier: i64,
    pub under_con_id: i32,
    pub long_name: String,
    pub contract_month: String,
    pub industry: String,
    pub category: String,
    pub subcategory: String,
    pub time_zone_id: String,
    pub trading_hours: String,
    pub liquid_hours: String,
    pub ev_rule: String,
    pub ev_multiplier: f64,
    /// C++ default: `UNSET_INTEGER`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agg_group: Option<i32>,
    pub under_symbol: String,
    pub under_sec_type: String,
    pub market_rule_ids: String,
    pub real_expiration_date: String,
    pub last_trade_time: String,
    pub stock_type: String,
    /// C++ default: `UNSET_DECIMAL`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_size: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_increment: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_size_increment: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sec_id_list: Option<Vec<TagValue>>,

    // ----- Bond-specific fields -----
    pub cusip: String,
    pub ratings: String,
    pub desc_append: String,
    pub bond_type: String,
    pub coupon_type: String,
    pub callable: bool,
    pub putable: bool,
    pub coupon: f64,
    pub convertible: bool,
    pub maturity: String,
    pub issue_date: String,
    pub next_option_date: String,
    pub next_option_type: String,
    pub next_option_partial: bool,
    pub notes: String,

    // ----- Fund-specific fields -----
    pub fund_name: String,
    pub fund_family: String,
    pub fund_type: String,
    pub fund_front_load: String,
    pub fund_back_load: String,
    pub fund_back_load_time_interval: String,
    pub fund_management_fee: String,
    pub fund_closed: bool,
    pub fund_closed_for_new_investors: bool,
    pub fund_closed_for_new_money: bool,
    pub fund_notify_amount: String,
    pub fund_minimum_initial_purchase: String,
    pub fund_subsequent_minimum_purchase: String,
    pub fund_blue_sky_states: String,
    pub fund_blue_sky_territories: String,
    pub fund_distribution_policy_indicator: FundDistributionPolicyIndicator,
    pub fund_asset_type: FundAssetType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub ineligibility_reason_list: Option<Vec<IneligibilityReason>>,
}

// ============================================================================
// ContractDescription
// ============================================================================

/// Contract with its available derivative security types.
///
/// Returned by `reqMatchingSymbols`.
///
/// C++ source: `struct ContractDescription` in `Contract.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractDescription {
    pub contract: Contract,
    pub derivative_sec_types: Vec<String>,
}
