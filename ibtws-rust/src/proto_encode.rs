//! Protobuf encoding helpers for outgoing IB API messages.
//!
//! Converts Rust model types to protobuf message types for encoding.
//! Used by `IBClient` for server versions that require protobuf encoding:
//! - placeOrder (sv >= 203)
//! - cancelOrder (sv >= 203)
//! - reqExecutions (sv >= 201)
//! - reqGlobalCancel (sv >= 203)

use crate::models::contract::Contract;
use crate::models::execution::ExecutionFilter;
use crate::models::order::{Order, OrderCancel, OrderCondition};

// Include the prost-generated protobuf types.
#[allow(clippy::derive_partial_eq_without_eq)]
mod pb {
    include!("generated/protobuf.rs");
}

// ============================================================================
// Public builders
// ============================================================================

pub fn build_place_order_request(
    id: i64,
    contract: &Contract,
    order: &Order,
) -> pb::PlaceOrderRequest {
    pb::PlaceOrderRequest {
        order_id: Some(id as i32),
        contract: Some(contract_to_proto(contract)),
        order: Some(order_to_proto(order)),
    }
}

pub fn build_cancel_order_request(id: i64, cancel: &OrderCancel) -> pb::CancelOrderRequest {
    pb::CancelOrderRequest {
        order_id: Some(id as i32),
        order_cancel: Some(pb::OrderCancel {
            manual_order_cancel_time: opt_str(&cancel.manual_order_cancel_time),
            ext_operator: opt_str(&cancel.ext_operator),
            manual_order_indicator: cancel.manual_order_indicator,
        }),
    }
}

pub fn build_execution_request(req_id: i32, filter: &ExecutionFilter) -> pb::ExecutionRequest {
    pb::ExecutionRequest {
        req_id: Some(req_id),
        execution_filter: Some(pb::ExecutionFilter {
            client_id: Some(filter.client_id as i32),
            acct_code: opt_str(&filter.acct_code),
            time: opt_str(&filter.time),
            symbol: opt_str(&filter.symbol),
            sec_type: opt_str(&filter.sec_type),
            exchange: opt_str(&filter.exchange),
            side: opt_str(&filter.side),
            last_n_days: filter.last_n_days,
            specific_dates: filter.specific_dates.iter().map(|&d| d as i32).collect(),
        }),
    }
}

pub fn build_global_cancel_request(cancel: &OrderCancel) -> pb::GlobalCancelRequest {
    pb::GlobalCancelRequest {
        order_cancel: Some(pb::OrderCancel {
            manual_order_cancel_time: opt_str(&cancel.manual_order_cancel_time),
            ext_operator: opt_str(&cancel.ext_operator),
            manual_order_indicator: cancel.manual_order_indicator,
        }),
    }
}

// ============================================================================
// Private conversion helpers
// ============================================================================

fn opt_str(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn contract_to_proto(c: &Contract) -> pb::Contract {
    pb::Contract {
        con_id: Some(c.con_id as i32),
        symbol: opt_str(&c.symbol),
        sec_type: c.sec_type.as_ref().map(|s| s.to_string()),
        last_trade_date_or_contract_month: opt_str(&c.last_trade_date_or_contract_month),
        strike: c.strike,
        right: c.right.as_ref().map(|r| r.to_string()),
        multiplier: c.multiplier.parse::<f64>().ok(),
        exchange: opt_str(&c.exchange),
        primary_exch: opt_str(&c.primary_exchange),
        currency: opt_str(&c.currency),
        local_symbol: opt_str(&c.local_symbol),
        trading_class: opt_str(&c.trading_class),
        sec_id_type: c.sec_id_type.as_ref().map(|s| s.to_string()),
        sec_id: opt_str(&c.sec_id),
        description: opt_str(&c.description),
        issuer_id: opt_str(&c.issuer_id),
        include_expired: Some(c.include_expired),
        combo_legs_descrip: opt_str(&c.combo_legs_descrip),
        delta_neutral_contract: c.delta_neutral_contract.as_ref().map(|dnc| {
            pb::DeltaNeutralContract {
                con_id: Some(dnc.con_id as i32),
                delta: Some(dnc.delta),
                price: Some(dnc.price),
            }
        }),
        combo_legs: c
            .combo_legs
            .as_ref()
            .map(|legs| {
                legs.iter()
                    .map(|leg| pb::ComboLeg {
                        con_id: Some(leg.con_id as i32),
                        ratio: Some(leg.ratio as i32),
                        action: leg.action.as_ref().map(|a| a.to_string()),
                        exchange: opt_str(&leg.exchange),
                        open_close: Some(leg.open_close as i32),
                        short_sales_slot: Some(leg.short_sale_slot),
                        designated_location: opt_str(&leg.designated_location),
                        exempt_code: Some(leg.exempt_code),
                        per_leg_price: None,
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn order_to_proto(o: &Order) -> pb::Order {
    pb::Order {
        client_id: Some(o.client_id as i32),
        order_id: Some(o.order_id as i32),
        perm_id: Some(o.perm_id),
        parent_id: Some(o.parent_id as i32),
        action: o.action.as_ref().map(|a| a.to_string()),
        total_quantity: o.total_quantity.map(|d| d.to_string()),
        display_size: Some(o.display_size),
        order_type: o.order_type.as_ref().map(|t| t.to_string()),
        lmt_price: o.lmt_price,
        aux_price: o.aux_price,
        tif: o.tif.as_ref().map(|t| t.to_string()),
        account: opt_str(&o.account),
        settling_firm: opt_str(&o.settling_firm),
        clearing_account: opt_str(&o.clearing_account),
        clearing_intent: opt_str(&o.clearing_intent),
        all_or_none: Some(o.all_or_none),
        block_order: Some(o.block_order),
        hidden: Some(o.hidden),
        outside_rth: Some(o.outside_rth),
        sweep_to_fill: Some(o.sweep_to_fill),
        percent_offset: o.percent_offset,
        trailing_percent: o.trailing_percent,
        trail_stop_price: o.trail_stop_price,
        min_qty: o.min_qty,
        good_after_time: opt_str(&o.good_after_time),
        good_till_date: opt_str(&o.good_till_date),
        oca_group: opt_str(&o.oca_group),
        order_ref: opt_str(&o.order_ref),
        rule80_a: opt_str(&o.rule_80a),
        oca_type: Some(o.oca_type),
        trigger_method: Some(o.trigger_method),
        active_start_time: opt_str(&o.active_start_time),
        active_stop_time: opt_str(&o.active_stop_time),
        fa_group: opt_str(&o.fa_group),
        fa_method: opt_str(&o.fa_method),
        fa_percentage: opt_str(&o.fa_percentage),
        volatility: o.volatility,
        volatility_type: o.volatility_type,
        continuous_update: Some(o.continuous_update),
        reference_price_type: o.reference_price_type,
        delta_neutral_order_type: opt_str(&o.delta_neutral_order_type),
        delta_neutral_aux_price: o.delta_neutral_aux_price,
        delta_neutral_con_id: if o.delta_neutral_con_id != 0 {
            Some(o.delta_neutral_con_id as i32)
        } else {
            None
        },
        delta_neutral_open_close: opt_str(&o.delta_neutral_open_close),
        delta_neutral_short_sale: Some(o.delta_neutral_short_sale),
        delta_neutral_short_sale_slot: Some(o.delta_neutral_short_sale_slot),
        delta_neutral_designated_location: opt_str(&o.delta_neutral_designated_location),
        scale_init_level_size: o.scale_init_level_size,
        scale_subs_level_size: o.scale_subs_level_size,
        scale_price_increment: o.scale_price_increment,
        scale_price_adjust_value: o.scale_price_adjust_value,
        scale_price_adjust_interval: o.scale_price_adjust_interval,
        scale_profit_offset: o.scale_profit_offset,
        scale_auto_reset: Some(o.scale_auto_reset),
        scale_init_position: o.scale_init_position,
        scale_init_fill_qty: o.scale_init_fill_qty,
        scale_random_percent: Some(o.scale_random_percent),
        scale_table: opt_str(&o.scale_table),
        hedge_type: opt_str(&o.hedge_type),
        hedge_param: opt_str(&o.hedge_param),
        algo_strategy: opt_str(&o.algo_strategy),
        algo_params: o
            .algo_params
            .as_ref()
            .map(|params| {
                params
                    .iter()
                    .map(|tv| (tv.tag.clone(), tv.value.clone()))
                    .collect()
            })
            .unwrap_or_default(),
        algo_id: opt_str(&o.algo_id),
        smart_combo_routing_params: o
            .smart_combo_routing_params
            .as_ref()
            .map(|params| {
                params
                    .iter()
                    .map(|tv| (tv.tag.clone(), tv.value.clone()))
                    .collect()
            })
            .unwrap_or_default(),
        what_if: Some(o.what_if),
        transmit: Some(o.transmit),
        override_percentage_constraints: Some(o.override_percentage_constraints),
        open_close: opt_str(&o.open_close),
        origin: Some(o.origin as i32),
        short_sale_slot: Some(o.short_sale_slot),
        designated_location: opt_str(&o.designated_location),
        exempt_code: Some(o.exempt_code),
        delta_neutral_settling_firm: opt_str(&o.delta_neutral_settling_firm),
        delta_neutral_clearing_account: opt_str(&o.delta_neutral_clearing_account),
        delta_neutral_clearing_intent: opt_str(&o.delta_neutral_clearing_intent),
        discretionary_amt: Some(o.discretionary_amt),
        opt_out_smart_routing: Some(o.opt_out_smart_routing),
        starting_price: o.starting_price,
        stock_ref_price: o.stock_ref_price,
        delta: o.delta,
        stock_range_lower: o.stock_range_lower,
        stock_range_upper: o.stock_range_upper,
        not_held: Some(o.not_held),
        order_misc_options: o
            .order_misc_options
            .as_ref()
            .map(|opts| {
                opts.iter()
                    .map(|tv| (tv.tag.clone(), tv.value.clone()))
                    .collect()
            })
            .unwrap_or_default(),
        solicited: Some(o.solicited),
        randomize_size: Some(o.randomize_size),
        randomize_price: Some(o.randomize_price),
        reference_contract_id: o.reference_contract_id,
        pegged_change_amount: o.pegged_change_amount,
        is_pegged_change_amount_decrease: Some(o.is_pegged_change_amount_decrease),
        reference_change_amount: o.reference_change_amount,
        reference_exchange_id: opt_str(&o.reference_exchange_id),
        adjusted_order_type: opt_str(&o.adjusted_order_type),
        trigger_price: o.trigger_price,
        adjusted_stop_price: o.adjusted_stop_price,
        adjusted_stop_limit_price: o.adjusted_stop_limit_price,
        adjusted_trailing_amount: o.adjusted_trailing_amount,
        adjustable_trailing_unit: o.adjustable_trailing_unit,
        lmt_price_offset: o.lmt_price_offset,
        conditions: o.conditions.iter().map(condition_to_proto).collect(),
        conditions_cancel_order: Some(o.conditions_cancel_order),
        conditions_ignore_rth: Some(o.conditions_ignore_rth),
        model_code: opt_str(&o.model_code),
        ext_operator: opt_str(&o.ext_operator),
        soft_dollar_tier: Some(pb::SoftDollarTier {
            name: opt_str(&o.soft_dollar_tier.name),
            value: opt_str(&o.soft_dollar_tier.val),
            display_name: opt_str(&o.soft_dollar_tier.display_name),
        }),
        cash_qty: o.cash_qty,
        mifid2_decision_maker: opt_str(&o.mifid2_decision_maker),
        mifid2_decision_algo: opt_str(&o.mifid2_decision_algo),
        mifid2_execution_trader: opt_str(&o.mifid2_execution_trader),
        mifid2_execution_algo: opt_str(&o.mifid2_execution_algo),
        dont_use_auto_price_for_hedge: Some(o.dont_use_auto_price_for_hedge),
        is_oms_container: Some(o.is_oms_container),
        discretionary_up_to_limit_price: Some(o.discretionary_up_to_limit_price),
        auto_cancel_date: opt_str(&o.auto_cancel_date),
        filled_quantity: o.filled_quantity.map(|d| d.to_string()),
        ref_futures_con_id: o.ref_futures_con_id,
        auto_cancel_parent: Some(o.auto_cancel_parent),
        shareholder: opt_str(&o.shareholder),
        imbalance_only: Some(o.imbalance_only),
        route_marketable_to_bbo: Some(o.route_marketable_to_bbo),
        parent_perm_id: o.parent_perm_id,
        use_price_mgmt_algo: Some(o.use_price_mgmt_algo as i32),
        duration: o.duration,
        post_to_ats: o.post_to_ats,
        advanced_error_override: opt_str(&o.advanced_error_override),
        manual_order_time: opt_str(&o.manual_order_time),
        min_trade_qty: o.min_trade_qty,
        min_compete_size: o.min_compete_size,
        compete_against_best_offset: o.compete_against_best_offset,
        mid_offset_at_whole: o.mid_offset_at_whole,
        mid_offset_at_half: o.mid_offset_at_half,
        customer_account: opt_str(&o.customer_account),
        professional_customer: Some(o.professional_customer),
        bond_accrued_interest: opt_str(&o.bond_accrued_interest),
        include_overnight: Some(o.include_overnight),
        manual_order_indicator: o.manual_order_indicator,
        submitter: opt_str(&o.submitter),
    }
}

fn condition_to_proto(cond: &OrderCondition) -> pb::OrderCondition {
    match cond {
        OrderCondition::Price {
            is_conjunction_connection,
            is_more,
            con_id,
            exchange,
            price,
            trigger_method,
        } => pb::OrderCondition {
            r#type: Some(1),
            is_conjunction_connection: Some(*is_conjunction_connection),
            is_more: Some(*is_more),
            con_id: Some(*con_id),
            exchange: opt_str(exchange),
            price: Some(*price),
            trigger_method: Some(*trigger_method as i32),
            symbol: None,
            sec_type: None,
            percent: None,
            change_percent: None,
            time: None,
            volume: None,
        },
        OrderCondition::Time {
            is_conjunction_connection,
            is_more,
            time,
        } => pb::OrderCondition {
            r#type: Some(3),
            is_conjunction_connection: Some(*is_conjunction_connection),
            is_more: Some(*is_more),
            time: opt_str(time),
            con_id: None,
            exchange: None,
            symbol: None,
            sec_type: None,
            percent: None,
            change_percent: None,
            price: None,
            trigger_method: None,
            volume: None,
        },
        OrderCondition::Margin {
            is_conjunction_connection,
            is_more,
            percent,
        } => pb::OrderCondition {
            r#type: Some(4),
            is_conjunction_connection: Some(*is_conjunction_connection),
            is_more: Some(*is_more),
            percent: Some(*percent),
            con_id: None,
            exchange: None,
            symbol: None,
            sec_type: None,
            change_percent: None,
            price: None,
            trigger_method: None,
            time: None,
            volume: None,
        },
        OrderCondition::Execution {
            is_conjunction_connection,
            exchange,
            sec_type,
            symbol,
        } => pb::OrderCondition {
            r#type: Some(5),
            is_conjunction_connection: Some(*is_conjunction_connection),
            exchange: opt_str(exchange),
            sec_type: opt_str(sec_type),
            symbol: opt_str(symbol),
            is_more: None,
            con_id: None,
            percent: None,
            change_percent: None,
            price: None,
            trigger_method: None,
            time: None,
            volume: None,
        },
        OrderCondition::Volume {
            is_conjunction_connection,
            is_more,
            con_id,
            exchange,
            volume,
        } => pb::OrderCondition {
            r#type: Some(6),
            is_conjunction_connection: Some(*is_conjunction_connection),
            is_more: Some(*is_more),
            con_id: Some(*con_id),
            exchange: opt_str(exchange),
            volume: Some(*volume),
            symbol: None,
            sec_type: None,
            percent: None,
            change_percent: None,
            price: None,
            trigger_method: None,
            time: None,
        },
        OrderCondition::PercentChange {
            is_conjunction_connection,
            is_more,
            con_id,
            exchange,
            change_percent,
        } => pb::OrderCondition {
            r#type: Some(7),
            is_conjunction_connection: Some(*is_conjunction_connection),
            is_more: Some(*is_more),
            con_id: Some(*con_id),
            exchange: opt_str(exchange),
            change_percent: *change_percent,
            symbol: None,
            sec_type: None,
            percent: None,
            price: None,
            trigger_method: None,
            time: None,
            volume: None,
        },
    }
}
