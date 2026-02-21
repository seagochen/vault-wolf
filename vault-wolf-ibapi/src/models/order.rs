//! Order-related data structures and the `OrderCondition` enum.
//!
//! Ported from: `cppclient/client/Order.h`, `OrderState.h`, `OrderCancel.h`,
//! `OrderCondition.h`, and all condition subtypes (`PriceCondition.h`,
//! `TimeCondition.h`, `MarginCondition.h`, `VolumeCondition.h`,
//! `PercentChangeCondition.h`, `ExecutionCondition.h`).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::common::{SoftDollarTier, TagValue};
use super::enums::*;

// ============================================================================
// OrderCondition
// ============================================================================

/// Order condition -- replaces the C++ `OrderCondition` class hierarchy.
///
/// C++ uses inheritance: `OrderCondition` -> `OperatorCondition` ->
/// `ContractCondition` -> `PriceCondition`. Rust flattens this into a single
/// enum with named fields per variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OrderCondition {
    Price {
        is_conjunction_connection: bool,
        is_more: bool,
        con_id: i32,
        exchange: String,
        price: f64,
        trigger_method: TriggerMethod,
    },
    Time {
        is_conjunction_connection: bool,
        is_more: bool,
        time: String,
    },
    Margin {
        is_conjunction_connection: bool,
        is_more: bool,
        percent: i32,
    },
    Execution {
        is_conjunction_connection: bool,
        exchange: String,
        sec_type: String,
        symbol: String,
    },
    Volume {
        is_conjunction_connection: bool,
        is_more: bool,
        con_id: i32,
        exchange: String,
        volume: i32,
    },
    PercentChange {
        is_conjunction_connection: bool,
        is_more: bool,
        con_id: i32,
        exchange: String,
        change_percent: Option<f64>,
    },
}

// ============================================================================
// OrderComboLeg
// ============================================================================

/// Per-leg price for combo orders.
///
/// C++ source: `struct OrderComboLeg` in `Order.h`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderComboLeg {
    /// C++ default: `UNSET_DOUBLE`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
}

// ============================================================================
// Order
// ============================================================================

/// Full IB order specification.
///
/// This is the largest struct in the API with ~200 fields, organized into
/// logical sections. All fields that use C++ sentinel values (`UNSET_DOUBLE`,
/// `UNSET_INTEGER`) are represented as `Option<T>`.
///
/// C++ source: `struct Order` in `Order.h`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    // ----- Order Identification -----
    pub order_id: i64,
    pub client_id: i64,
    pub perm_id: i64,

    // ----- Main Order Fields -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_quantity: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_type: Option<OrderType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lmt_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aux_price: Option<f64>,

    // ----- Extended Order Fields -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tif: Option<TimeInForce>,
    pub active_start_time: String,
    pub active_stop_time: String,
    pub oca_group: String,
    pub oca_type: i32,
    pub order_ref: String,
    pub transmit: bool,
    pub parent_id: i64,
    pub block_order: bool,
    pub sweep_to_fill: bool,
    pub display_size: i32,
    pub trigger_method: i32,
    pub outside_rth: bool,
    pub hidden: bool,
    pub good_after_time: String,
    pub good_till_date: String,
    pub rule_80a: String,
    pub all_or_none: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_qty: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_offset: Option<f64>,
    pub override_percentage_constraints: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trail_stop_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trailing_percent: Option<f64>,

    // ----- Financial Advisors -----
    pub fa_group: String,
    pub fa_method: String,
    pub fa_percentage: String,

    // ----- Institutional -----
    pub open_close: String,
    pub origin: Origin,
    pub short_sale_slot: i32,
    pub designated_location: String,
    /// C++ default: -1.
    pub exempt_code: i32,

    // ----- SMART Routing -----
    pub discretionary_amt: f64,
    pub opt_out_smart_routing: bool,

    // ----- BOX Exchange -----
    pub auction_strategy: AuctionStrategy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starting_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_ref_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f64>,

    // ----- Pegged to Stock / VOL -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_range_lower: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock_range_upper: Option<f64>,
    pub randomize_size: bool,
    pub randomize_price: bool,

    // ----- Volatility Orders -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volatility: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volatility_type: Option<i32>,
    pub delta_neutral_order_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_neutral_aux_price: Option<f64>,
    pub delta_neutral_con_id: i64,
    pub delta_neutral_settling_firm: String,
    pub delta_neutral_clearing_account: String,
    pub delta_neutral_clearing_intent: String,
    pub delta_neutral_open_close: String,
    pub delta_neutral_short_sale: bool,
    pub delta_neutral_short_sale_slot: i32,
    pub delta_neutral_designated_location: String,
    pub continuous_update: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_price_type: Option<i32>,

    // ----- Combo Orders -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basis_points: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basis_points_type: Option<i32>,

    // ----- Scale Orders -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_init_level_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_subs_level_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_price_increment: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_price_adjust_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_price_adjust_interval: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_profit_offset: Option<f64>,
    pub scale_auto_reset: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_init_position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_init_fill_qty: Option<i32>,
    pub scale_random_percent: bool,
    pub scale_table: String,

    // ----- Hedge Orders -----
    pub hedge_type: String,
    pub hedge_param: String,

    // ----- Clearing Info -----
    pub account: String,
    pub settling_firm: String,
    pub clearing_account: String,
    pub clearing_intent: String,

    // ----- Algo Orders -----
    pub algo_strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algo_params: Option<Vec<TagValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smart_combo_routing_params: Option<Vec<TagValue>>,
    pub algo_id: String,

    // ----- What-if / Not Held -----
    pub what_if: bool,
    pub not_held: bool,
    pub solicited: bool,

    // ----- Models -----
    pub model_code: String,

    // ----- Order Combo Legs -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_combo_legs: Option<Vec<OrderComboLeg>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_misc_options: Option<Vec<TagValue>>,

    // ----- Pegged to Benchmark -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_contract_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pegged_change_amount: Option<f64>,
    pub is_pegged_change_amount_decrease: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_change_amount: Option<f64>,
    pub reference_exchange_id: String,
    pub adjusted_order_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjusted_stop_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjusted_stop_limit_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjusted_trailing_amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjustable_trailing_unit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lmt_price_offset: Option<f64>,

    // ----- Conditions -----
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<OrderCondition>,
    pub conditions_cancel_order: bool,
    pub conditions_ignore_rth: bool,

    // ----- External Operator -----
    pub ext_operator: String,

    // ----- Soft Dollar Tier -----
    pub soft_dollar_tier: SoftDollarTier,

    // ----- Cash Quantity -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cash_qty: Option<f64>,

    // ----- MiFID II -----
    pub mifid2_decision_maker: String,
    pub mifid2_decision_algo: String,
    pub mifid2_execution_trader: String,
    pub mifid2_execution_algo: String,

    // ----- Miscellaneous -----
    pub dont_use_auto_price_for_hedge: bool,
    pub is_oms_container: bool,
    pub discretionary_up_to_limit_price: bool,
    pub auto_cancel_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filled_quantity: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_futures_con_id: Option<i32>,
    pub auto_cancel_parent: bool,
    pub shareholder: String,
    pub imbalance_only: bool,
    pub route_marketable_to_bbo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_perm_id: Option<i64>,
    pub use_price_mgmt_algo: UsePriceMgmtAlgo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_to_ats: Option<i32>,
    pub advanced_error_override: String,
    pub manual_order_time: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_trade_qty: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_compete_size: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compete_against_best_offset: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mid_offset_at_whole: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mid_offset_at_half: Option<f64>,
    pub customer_account: String,
    pub professional_customer: bool,
    pub bond_accrued_interest: String,
    pub include_overnight: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_order_indicator: Option<i32>,
    pub submitter: String,
}

impl Default for Order {
    fn default() -> Self {
        Self {
            // ----- Identification -----
            order_id: 0,
            client_id: 0,
            perm_id: 0,
            // ----- Main -----
            action: None,
            total_quantity: None,
            order_type: None,
            lmt_price: None,
            aux_price: None,
            // ----- Extended -----
            tif: None,
            active_start_time: String::new(),
            active_stop_time: String::new(),
            oca_group: String::new(),
            oca_type: 0,
            order_ref: String::new(),
            transmit: true, // C++ default: true
            parent_id: 0,
            block_order: false,
            sweep_to_fill: false,
            display_size: 0,
            trigger_method: 0,
            outside_rth: false,
            hidden: false,
            good_after_time: String::new(),
            good_till_date: String::new(),
            rule_80a: String::new(),
            all_or_none: false,
            min_qty: None,
            percent_offset: None,
            override_percentage_constraints: false,
            trail_stop_price: None,
            trailing_percent: None,
            // ----- FA -----
            fa_group: String::new(),
            fa_method: String::new(),
            fa_percentage: String::new(),
            // ----- Institutional -----
            open_close: String::new(),
            origin: Origin::Customer, // C++ default: CUSTOMER
            short_sale_slot: 0,
            designated_location: String::new(),
            exempt_code: -1, // C++ default: -1
            // ----- SMART -----
            discretionary_amt: 0.0,
            opt_out_smart_routing: false,
            // ----- BOX -----
            auction_strategy: AuctionStrategy::Unset,
            starting_price: None,
            stock_ref_price: None,
            delta: None,
            // ----- Pegged/VOL -----
            stock_range_lower: None,
            stock_range_upper: None,
            randomize_size: false,
            randomize_price: false,
            // ----- Volatility -----
            volatility: None,
            volatility_type: None,
            delta_neutral_order_type: String::new(),
            delta_neutral_aux_price: None,
            delta_neutral_con_id: 0,
            delta_neutral_settling_firm: String::new(),
            delta_neutral_clearing_account: String::new(),
            delta_neutral_clearing_intent: String::new(),
            delta_neutral_open_close: String::new(),
            delta_neutral_short_sale: false,
            delta_neutral_short_sale_slot: 0,
            delta_neutral_designated_location: String::new(),
            continuous_update: false,
            reference_price_type: None,
            // ----- Combo -----
            basis_points: None,
            basis_points_type: None,
            // ----- Scale -----
            scale_init_level_size: None,
            scale_subs_level_size: None,
            scale_price_increment: None,
            scale_price_adjust_value: None,
            scale_price_adjust_interval: None,
            scale_profit_offset: None,
            scale_auto_reset: false,
            scale_init_position: None,
            scale_init_fill_qty: None,
            scale_random_percent: false,
            scale_table: String::new(),
            // ----- Hedge -----
            hedge_type: String::new(),
            hedge_param: String::new(),
            // ----- Clearing -----
            account: String::new(),
            settling_firm: String::new(),
            clearing_account: String::new(),
            clearing_intent: String::new(),
            // ----- Algo -----
            algo_strategy: String::new(),
            algo_params: None,
            smart_combo_routing_params: None,
            algo_id: String::new(),
            // ----- What-if -----
            what_if: false,
            not_held: false,
            solicited: false,
            // ----- Models -----
            model_code: String::new(),
            // ----- Order Combo Legs -----
            order_combo_legs: None,
            order_misc_options: None,
            // ----- Pegged to Benchmark -----
            reference_contract_id: None,
            pegged_change_amount: None,
            is_pegged_change_amount_decrease: false,
            reference_change_amount: None,
            reference_exchange_id: String::new(),
            adjusted_order_type: String::new(),
            trigger_price: None,
            adjusted_stop_price: None,
            adjusted_stop_limit_price: None,
            adjusted_trailing_amount: None,
            adjustable_trailing_unit: None,
            lmt_price_offset: None,
            // ----- Conditions -----
            conditions: Vec::new(),
            conditions_cancel_order: false,
            conditions_ignore_rth: false,
            // ----- Ext Operator -----
            ext_operator: String::new(),
            // ----- Soft Dollar -----
            soft_dollar_tier: SoftDollarTier::default(),
            // ----- Cash -----
            cash_qty: None,
            // ----- MiFID II -----
            mifid2_decision_maker: String::new(),
            mifid2_decision_algo: String::new(),
            mifid2_execution_trader: String::new(),
            mifid2_execution_algo: String::new(),
            // ----- Misc -----
            dont_use_auto_price_for_hedge: false,
            is_oms_container: false,
            discretionary_up_to_limit_price: false,
            auto_cancel_date: String::new(),
            filled_quantity: None,
            ref_futures_con_id: None,
            auto_cancel_parent: false,
            shareholder: String::new(),
            imbalance_only: false,
            route_marketable_to_bbo: false,
            parent_perm_id: None,
            use_price_mgmt_algo: UsePriceMgmtAlgo::Default,
            duration: None,
            post_to_ats: None,
            advanced_error_override: String::new(),
            manual_order_time: String::new(),
            min_trade_qty: None,
            min_compete_size: None,
            compete_against_best_offset: None,
            mid_offset_at_whole: None,
            mid_offset_at_half: None,
            customer_account: String::new(),
            professional_customer: false,
            bond_accrued_interest: String::new(),
            include_overnight: false,
            manual_order_indicator: None,
            submitter: String::new(),
        }
    }
}

// ============================================================================
// OrderAllocation
// ============================================================================

/// Per-account allocation info within an order (FA orders).
///
/// C++ source: `struct OrderAllocation` in `OrderState.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderAllocation {
    pub account: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_desired: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_after: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desired_alloc_qty: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_alloc_qty: Option<Decimal>,
    pub is_monetary: bool,
}

// ============================================================================
// OrderState
// ============================================================================

/// Order state including margin impact and commission info.
///
/// C++ source: `struct OrderState` in `OrderState.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderState {
    pub status: String,

    // ----- Margin before -----
    pub init_margin_before: String,
    pub maint_margin_before: String,
    pub equity_with_loan_before: String,

    // ----- Margin change -----
    pub init_margin_change: String,
    pub maint_margin_change: String,
    pub equity_with_loan_change: String,

    // ----- Margin after -----
    pub init_margin_after: String,
    pub maint_margin_after: String,
    pub equity_with_loan_after: String,

    // ----- Commission -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commission_and_fees: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_commission_and_fees: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_commission_and_fees: Option<f64>,
    pub commission_and_fees_currency: String,
    pub margin_currency: String,

    // ----- Outside RTH margin -----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init_margin_before_outside_rth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maint_margin_before_outside_rth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equity_with_loan_before_outside_rth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init_margin_change_outside_rth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maint_margin_change_outside_rth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equity_with_loan_change_outside_rth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init_margin_after_outside_rth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maint_margin_after_outside_rth: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equity_with_loan_after_outside_rth: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_size: Option<Decimal>,
    pub reject_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_allocations: Option<Vec<OrderAllocation>>,
    pub warning_text: String,
    pub completed_time: String,
    pub completed_status: String,
}

// ============================================================================
// OrderCancel
// ============================================================================

/// Parameters for cancelling an order.
///
/// C++ source: `struct OrderCancel` in `OrderCancel.h`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderCancel {
    pub manual_order_cancel_time: String,
    pub ext_operator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manual_order_indicator: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_default_matches_cpp() {
        let order = Order::default();
        assert!(order.transmit, "C++ default: transmit = true");
        assert_eq!(order.origin, Origin::Customer, "C++ default: origin = CUSTOMER");
        assert_eq!(order.exempt_code, -1, "C++ default: exemptCode = -1");
        assert_eq!(order.auction_strategy, AuctionStrategy::Unset);
        assert_eq!(order.use_price_mgmt_algo, UsePriceMgmtAlgo::Default);
        assert!(order.conditions.is_empty());
        assert!(order.action.is_none());
        assert!(order.total_quantity.is_none());
        assert!(order.lmt_price.is_none());
        assert_eq!(order.order_id, 0);
    }

    #[test]
    fn order_condition_serde_round_trip() {
        let cond = OrderCondition::Price {
            is_conjunction_connection: true,
            is_more: true,
            con_id: 12345,
            exchange: "SMART".to_string(),
            price: 150.50,
            trigger_method: TriggerMethod::Last,
        };
        let json = serde_json::to_string(&cond).unwrap();
        let deserialized: OrderCondition = serde_json::from_str(&json).unwrap();
        assert_eq!(cond, deserialized);
    }

    #[test]
    fn order_state_default() {
        let state = OrderState::default();
        assert!(state.status.is_empty());
        assert!(state.commission_and_fees.is_none());
        assert!(state.order_allocations.is_none());
    }
}
