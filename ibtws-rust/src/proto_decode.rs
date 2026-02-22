//! Protobuf message decoding for IB TWS API protocol.
//!
//! When `server_version >= MIN_SERVER_VER_PROTOBUF` (201), the server may send
//! certain messages in protobuf format instead of the traditional null-terminated
//! ASCII encoding. Protobuf messages are identified by a message ID offset:
//! if `msg_id > PROTOBUF_MSG_ID` (200), subtract 200 to get the real message
//! type, and decode the remaining bytes as a protobuf message.
//!
//! Currently 6 incoming message types support protobuf encoding:
//! - ORDER_STATUS (3)
//! - ERR_MSG (4)
//! - OPEN_ORDER (5)
//! - EXECUTION_DATA (11)
//! - OPEN_ORDER_END (53)
//! - EXECUTION_DATA_END (55)
//!
//! Ported from: `EDecoder.cpp` (processXxxProtoBuf functions) and
//! `EDecoderUtils.cpp` (decodeContract, decodeOrder, decodeOrderState,
//! decodeExecution helper functions).

#![allow(clippy::field_reassign_with_default)]

use prost::Message;
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::errors::{IBApiError, Result};
use crate::models::common::{SoftDollarTier, TagValue};
use crate::models::contract::{ComboLeg, Contract, DeltaNeutralContract};
use crate::models::enums::*;
use crate::models::execution::Execution;
use crate::models::order::{
    Order, OrderAllocation, OrderComboLeg, OrderCondition, OrderState,
};
use crate::wrapper::IBEvent;

// Include the prost-generated protobuf types.
#[allow(clippy::derive_partial_eq_without_eq)]
pub mod pb {
    include!("generated/protobuf.rs");
}

// ============================================================================
// Public dispatch function
// ============================================================================

/// Decode a protobuf-encoded server message into an `IBEvent`.
///
/// `real_msg_id` is the message ID after subtracting `PROTOBUF_MSG_ID` (200).
/// `data` is the raw protobuf bytes (everything after the 4-byte msg_id).
///
/// Corresponds to the protobuf branch of `EDecoder::parseAndProcessMsg` in C++.
pub fn decode_protobuf_msg(real_msg_id: i32, data: &[u8]) -> Result<IBEvent> {
    use crate::protocol::incoming;

    match real_msg_id {
        incoming::ORDER_STATUS => decode_order_status_pb(data),
        incoming::ERR_MSG => decode_error_msg_pb(data),
        incoming::OPEN_ORDER => decode_open_order_pb(data),
        incoming::EXECUTION_DATA => decode_execution_details_pb(data),
        incoming::OPEN_ORDER_END => decode_open_order_end_pb(data),
        incoming::EXECUTION_DATA_END => decode_execution_details_end_pb(data),
        _ => Err(IBApiError::Decoding(format!(
            "unknown protobuf message id: {real_msg_id}"
        ))),
    }
}

// ============================================================================
// Individual protobuf message decoders
// ============================================================================

/// Decode protobuf ORDER_STATUS message.
/// C++: `EDecoder::processOrderStatusMsgProtoBuf`
fn decode_order_status_pb(data: &[u8]) -> Result<IBEvent> {
    let proto = pb::OrderStatus::decode(data)
        .map_err(|e| IBApiError::Decoding(format!("protobuf OrderStatus: {e}")))?;

    Ok(IBEvent::OrderStatus {
        order_id: proto.order_id.unwrap_or(0) as i64,
        status: proto.status.unwrap_or_default(),
        filled: parse_decimal_opt(&proto.filled),
        remaining: parse_decimal_opt(&proto.remaining),
        avg_fill_price: proto.avg_fill_price.unwrap_or(f64::MAX),
        perm_id: proto.perm_id.unwrap_or(0),
        parent_id: proto.parent_id.unwrap_or(0),
        last_fill_price: proto.last_fill_price.unwrap_or(f64::MAX),
        client_id: proto.client_id.unwrap_or(0),
        why_held: proto.why_held.unwrap_or_default(),
        mkt_cap_price: proto.mkt_cap_price.unwrap_or(f64::MAX),
    })
}

/// Decode protobuf ERR_MSG message.
/// C++: `EDecoder::processErrorMsgProtoBuf`
fn decode_error_msg_pb(data: &[u8]) -> Result<IBEvent> {
    let proto = pb::ErrorMessage::decode(data)
        .map_err(|e| IBApiError::Decoding(format!("protobuf ErrorMessage: {e}")))?;

    Ok(IBEvent::Error {
        req_id: proto.id.unwrap_or(0),
        error_time: proto.error_time.unwrap_or(0),
        code: proto.error_code.unwrap_or(0),
        message: proto.error_msg.unwrap_or_default(),
        advanced_order_reject_json: proto.advanced_order_reject_json.unwrap_or_default(),
    })
}

/// Decode protobuf OPEN_ORDER message.
/// C++: `EDecoder::processOpenOrderMsgProtoBuf`
fn decode_open_order_pb(data: &[u8]) -> Result<IBEvent> {
    let proto = pb::OpenOrder::decode(data)
        .map_err(|e| IBApiError::Decoding(format!("protobuf OpenOrder: {e}")))?;

    let order_id = proto.order_id.unwrap_or(0) as i64;

    let contract = match proto.contract {
        Some(ref c) => decode_contract_pb(c),
        None => Contract::default(),
    };

    let order = match (&proto.contract, &proto.order) {
        (Some(cp), Some(op)) => decode_order_pb(cp, op),
        (None, Some(op)) => {
            let empty = pb::Contract::default();
            decode_order_pb(&empty, op)
        }
        _ => Order::default(),
    };

    let order_state = match proto.order_state {
        Some(ref os) => decode_order_state_pb(os),
        None => OrderState::default(),
    };

    Ok(IBEvent::OpenOrder {
        order_id,
        contract: Box::new(contract),
        order: Box::new(order),
        order_state: Box::new(order_state),
    })
}

/// Decode protobuf EXECUTION_DATA message.
/// C++: `EDecoder::processExecutionDetailsMsgProtoBuf`
fn decode_execution_details_pb(data: &[u8]) -> Result<IBEvent> {
    let proto = pb::ExecutionDetails::decode(data)
        .map_err(|e| IBApiError::Decoding(format!("protobuf ExecutionDetails: {e}")))?;

    let req_id = proto.req_id.unwrap_or(0);

    let contract = match proto.contract {
        Some(ref c) => decode_contract_pb(c),
        None => Contract::default(),
    };

    let execution = match proto.execution {
        Some(ref e) => decode_execution_pb(e),
        None => Execution::default(),
    };

    Ok(IBEvent::ExecDetails {
        req_id,
        contract: Box::new(contract),
        execution: Box::new(execution),
    })
}

/// Decode protobuf OPEN_ORDER_END message.
/// C++: `EDecoder::processOpenOrderEndMsgProtoBuf`
fn decode_open_order_end_pb(data: &[u8]) -> Result<IBEvent> {
    // Parse to validate, but this message has no fields.
    let _proto = pb::OpenOrdersEnd::decode(data)
        .map_err(|e| IBApiError::Decoding(format!("protobuf OpenOrdersEnd: {e}")))?;

    Ok(IBEvent::OpenOrderEnd)
}

/// Decode protobuf EXECUTION_DATA_END message.
/// C++: `EDecoder::processExecutionDetailsEndMsgProtoBuf`
fn decode_execution_details_end_pb(data: &[u8]) -> Result<IBEvent> {
    let proto = pb::ExecutionDetailsEnd::decode(data)
        .map_err(|e| IBApiError::Decoding(format!("protobuf ExecutionDetailsEnd: {e}")))?;

    Ok(IBEvent::ExecDetailsEnd {
        req_id: proto.req_id.unwrap_or(0),
    })
}

// ============================================================================
// Proto → Rust model conversion helpers
// ============================================================================
// Ported from: `EDecoderUtils.cpp`

/// Convert protobuf Contract → Rust Contract.
/// C++: `EDecoderUtils::decodeContract`
fn decode_contract_pb(cp: &pb::Contract) -> Contract {
    let mut c = Contract::default();

    if let Some(v) = cp.con_id { c.con_id = v as i64; }
    if let Some(ref v) = cp.symbol { c.symbol.clone_from(v); }
    if let Some(ref v) = cp.sec_type { c.sec_type = SecType::from_str(v).ok(); }
    if let Some(ref v) = cp.last_trade_date_or_contract_month {
        c.last_trade_date_or_contract_month.clone_from(v);
    }
    if let Some(v) = cp.strike { c.strike = Some(v); }
    if let Some(ref v) = cp.right { c.right = Right::from_str(v).ok(); }
    if let Some(v) = cp.multiplier { c.multiplier = v.to_string(); }
    if let Some(ref v) = cp.exchange { c.exchange.clone_from(v); }
    if let Some(ref v) = cp.primary_exch { c.primary_exchange.clone_from(v); }
    if let Some(ref v) = cp.currency { c.currency.clone_from(v); }
    if let Some(ref v) = cp.local_symbol { c.local_symbol.clone_from(v); }
    if let Some(ref v) = cp.trading_class { c.trading_class.clone_from(v); }
    if let Some(ref v) = cp.combo_legs_descrip { c.combo_legs_descrip.clone_from(v); }

    // ComboLegs
    if !cp.combo_legs.is_empty() {
        let legs: Vec<ComboLeg> = cp.combo_legs.iter().map(|leg| {
            let mut cl = ComboLeg::default();
            if let Some(v) = leg.con_id { cl.con_id = v as i64; }
            if let Some(v) = leg.ratio { cl.ratio = v as i64; }
            if let Some(ref v) = leg.action { cl.action = Action::from_str(v).ok(); }
            if let Some(ref v) = leg.exchange { cl.exchange.clone_from(v); }
            if let Some(v) = leg.open_close {
                cl.open_close = LegOpenClose::try_from(v).unwrap_or(LegOpenClose::Same);
            }
            if let Some(v) = leg.short_sales_slot { cl.short_sale_slot = v; }
            if let Some(ref v) = leg.designated_location { cl.designated_location.clone_from(v); }
            if let Some(v) = leg.exempt_code { cl.exempt_code = v; }
            cl
        }).collect();
        c.combo_legs = Some(legs);
    }

    // DeltaNeutralContract
    if let Some(ref dnc) = cp.delta_neutral_contract {
        let mut d = DeltaNeutralContract::default();
        if let Some(v) = dnc.con_id { d.con_id = v as i64; }
        if let Some(v) = dnc.delta { d.delta = v; }
        if let Some(v) = dnc.price { d.price = v; }
        c.delta_neutral_contract = Some(d);
    }

    c
}

/// Convert protobuf Execution → Rust Execution.
/// C++: `EDecoderUtils::decodeExecution`
fn decode_execution_pb(ep: &pb::Execution) -> Execution {
    let mut e = Execution::default();

    if let Some(v) = ep.order_id { e.order_id = v as i64; }
    if let Some(v) = ep.client_id { e.client_id = v as i64; }
    if let Some(ref v) = ep.exec_id { e.exec_id.clone_from(v); }
    if let Some(ref v) = ep.time { e.time.clone_from(v); }
    if let Some(ref v) = ep.acct_number { e.acct_number.clone_from(v); }
    if let Some(ref v) = ep.exchange { e.exchange.clone_from(v); }
    if let Some(ref v) = ep.side { e.side.clone_from(v); }
    if let Some(ref v) = ep.shares {
        e.shares = Decimal::from_str(v).ok();
    }
    if let Some(v) = ep.price { e.price = v; }
    if let Some(v) = ep.perm_id { e.perm_id = v; }
    if let Some(v) = ep.is_liquidation { e.liquidation = if v { 1 } else { 0 }; }
    if let Some(ref v) = ep.cum_qty {
        e.cum_qty = Decimal::from_str(v).ok();
    }
    if let Some(v) = ep.avg_price { e.avg_price = v; }
    if let Some(ref v) = ep.order_ref { e.order_ref.clone_from(v); }
    if let Some(ref v) = ep.ev_rule { e.ev_rule.clone_from(v); }
    if let Some(v) = ep.ev_multiplier { e.ev_multiplier = v; }
    if let Some(ref v) = ep.model_code { e.model_code.clone_from(v); }
    if let Some(v) = ep.last_liquidity { e.last_liquidity = v; }
    if let Some(v) = ep.is_price_revision_pending { e.pending_price_revision = v; }
    if let Some(ref v) = ep.submitter { e.submitter.clone_from(v); }
    if let Some(v) = ep.opt_exercise_or_lapse_type {
        e.opt_exercise_or_lapse_type =
            OptionExerciseType::try_from(v).unwrap_or_default();
    }

    e
}

/// Convert protobuf Order → Rust Order.
/// C++: `EDecoderUtils::decodeOrder`
fn decode_order_pb(cp: &pb::Contract, op: &pb::Order) -> Order {
    let mut o = Order::default();

    // --- Identification ---
    if let Some(v) = op.client_id { o.client_id = v as i64; }
    if let Some(v) = op.order_id { o.order_id = v as i64; }
    if let Some(v) = op.perm_id { o.perm_id = v; }
    if let Some(v) = op.parent_id { o.parent_id = v as i64; }

    // --- Main fields ---
    if let Some(ref v) = op.action { o.action = Action::from_str(v).ok(); }
    if let Some(ref v) = op.total_quantity {
        o.total_quantity = Decimal::from_str(v).ok();
    }
    if let Some(ref v) = op.order_type { o.order_type = OrderType::from_str(v).ok(); }
    if let Some(v) = op.lmt_price { o.lmt_price = Some(v); }
    if let Some(v) = op.aux_price { o.aux_price = Some(v); }
    if let Some(ref v) = op.tif { o.tif = TimeInForce::from_str(v).ok(); }

    // --- Extended ---
    if let Some(ref v) = op.oca_group { o.oca_group.clone_from(v); }
    if let Some(ref v) = op.account { o.account.clone_from(v); }
    if let Some(ref v) = op.open_close { o.open_close.clone_from(v); }
    if let Some(v) = op.origin {
        o.origin = Origin::try_from(v).unwrap_or(Origin::Customer);
    }
    if let Some(ref v) = op.order_ref { o.order_ref.clone_from(v); }
    if let Some(v) = op.outside_rth { o.outside_rth = v; }
    if let Some(v) = op.hidden { o.hidden = v; }
    if let Some(v) = op.discretionary_amt { o.discretionary_amt = v; }
    if let Some(ref v) = op.good_after_time { o.good_after_time.clone_from(v); }
    if let Some(ref v) = op.fa_group { o.fa_group.clone_from(v); }
    if let Some(ref v) = op.fa_method { o.fa_method.clone_from(v); }
    if let Some(ref v) = op.fa_percentage { o.fa_percentage.clone_from(v); }
    if let Some(ref v) = op.model_code { o.model_code.clone_from(v); }
    if let Some(ref v) = op.good_till_date { o.good_till_date.clone_from(v); }
    if let Some(ref v) = op.rule80_a { o.rule_80a.clone_from(v); }
    if let Some(v) = op.percent_offset { o.percent_offset = Some(v); }
    if let Some(ref v) = op.settling_firm { o.settling_firm.clone_from(v); }
    if let Some(v) = op.short_sale_slot { o.short_sale_slot = v; }
    if let Some(ref v) = op.designated_location { o.designated_location.clone_from(v); }
    if let Some(v) = op.exempt_code { o.exempt_code = v; }
    if let Some(v) = op.starting_price { o.starting_price = Some(v); }
    if let Some(v) = op.stock_ref_price { o.stock_ref_price = Some(v); }
    if let Some(v) = op.delta { o.delta = Some(v); }
    if let Some(v) = op.stock_range_lower { o.stock_range_lower = Some(v); }
    if let Some(v) = op.stock_range_upper { o.stock_range_upper = Some(v); }
    if let Some(v) = op.display_size { o.display_size = v; }
    if let Some(v) = op.block_order { o.block_order = v; }
    if let Some(v) = op.sweep_to_fill { o.sweep_to_fill = v; }
    if let Some(v) = op.all_or_none { o.all_or_none = v; }
    if let Some(v) = op.min_qty { o.min_qty = Some(v); }
    if let Some(v) = op.oca_type { o.oca_type = v; }
    if let Some(v) = op.trigger_method { o.trigger_method = v; }
    if let Some(v) = op.volatility { o.volatility = Some(v); }
    if let Some(v) = op.volatility_type { o.volatility_type = Some(v); }
    if let Some(ref v) = op.delta_neutral_order_type { o.delta_neutral_order_type.clone_from(v); }
    if let Some(v) = op.delta_neutral_aux_price { o.delta_neutral_aux_price = Some(v); }
    if let Some(v) = op.delta_neutral_con_id { o.delta_neutral_con_id = v as i64; }
    if let Some(ref v) = op.delta_neutral_settling_firm { o.delta_neutral_settling_firm.clone_from(v); }
    if let Some(ref v) = op.delta_neutral_clearing_account { o.delta_neutral_clearing_account.clone_from(v); }
    if let Some(ref v) = op.delta_neutral_clearing_intent { o.delta_neutral_clearing_intent.clone_from(v); }
    if let Some(ref v) = op.delta_neutral_open_close { o.delta_neutral_open_close.clone_from(v); }
    if let Some(v) = op.delta_neutral_short_sale { o.delta_neutral_short_sale = v; }
    if let Some(v) = op.delta_neutral_short_sale_slot { o.delta_neutral_short_sale_slot = v; }
    if let Some(ref v) = op.delta_neutral_designated_location { o.delta_neutral_designated_location.clone_from(v); }
    if let Some(v) = op.continuous_update { o.continuous_update = v; }
    if let Some(v) = op.reference_price_type { o.reference_price_type = Some(v); }
    if let Some(v) = op.trail_stop_price { o.trail_stop_price = Some(v); }
    if let Some(v) = op.trailing_percent { o.trailing_percent = Some(v); }

    // --- OrderComboLegs (derived from contract combo legs) ---
    if !cp.combo_legs.is_empty() {
        let order_combo_legs: Vec<OrderComboLeg> = cp.combo_legs.iter().map(|leg| {
            OrderComboLeg { price: leg.per_leg_price }
        }).collect();
        if order_combo_legs.iter().any(|l| l.price.is_some()) {
            o.order_combo_legs = Some(order_combo_legs);
        }
    }

    // --- SmartComboRoutingParams ---
    let scr_params = decode_tag_value_map(&op.smart_combo_routing_params);
    if !scr_params.is_empty() {
        o.smart_combo_routing_params = Some(scr_params);
    }

    // --- Scale ---
    if let Some(v) = op.scale_init_level_size { o.scale_init_level_size = Some(v); }
    if let Some(v) = op.scale_subs_level_size { o.scale_subs_level_size = Some(v); }
    if let Some(v) = op.scale_price_increment { o.scale_price_increment = Some(v); }
    if let Some(v) = op.scale_price_adjust_value { o.scale_price_adjust_value = Some(v); }
    if let Some(v) = op.scale_price_adjust_interval { o.scale_price_adjust_interval = Some(v); }
    if let Some(v) = op.scale_profit_offset { o.scale_profit_offset = Some(v); }
    if let Some(v) = op.scale_auto_reset { o.scale_auto_reset = v; }
    if let Some(v) = op.scale_init_position { o.scale_init_position = Some(v); }
    if let Some(v) = op.scale_init_fill_qty { o.scale_init_fill_qty = Some(v); }
    if let Some(v) = op.scale_random_percent { o.scale_random_percent = v; }

    // --- Hedge ---
    if let Some(ref v) = op.hedge_type { o.hedge_type.clone_from(v); }
    if op.hedge_type.is_some() && !o.hedge_type.is_empty() {
        if let Some(ref v) = op.hedge_param { o.hedge_param.clone_from(v); }
    }

    // --- Clearing / routing ---
    if let Some(v) = op.opt_out_smart_routing { o.opt_out_smart_routing = v; }
    if let Some(ref v) = op.clearing_account { o.clearing_account.clone_from(v); }
    if let Some(ref v) = op.clearing_intent { o.clearing_intent.clone_from(v); }
    if let Some(v) = op.not_held { o.not_held = v; }

    // --- Algo ---
    if let Some(ref v) = op.algo_strategy {
        o.algo_strategy.clone_from(v);
        let params = decode_tag_value_map(&op.algo_params);
        if !params.is_empty() {
            o.algo_params = Some(params);
        }
    }
    if let Some(ref v) = op.algo_id { o.algo_id.clone_from(v); }

    // --- Misc ---
    if let Some(v) = op.solicited { o.solicited = v; }
    if let Some(v) = op.what_if { o.what_if = v; }
    if let Some(v) = op.randomize_size { o.randomize_size = v; }
    if let Some(v) = op.randomize_price { o.randomize_price = v; }
    if let Some(v) = op.transmit { o.transmit = v; }
    if let Some(v) = op.override_percentage_constraints { o.override_percentage_constraints = v; }

    // --- Pegged to Benchmark ---
    if let Some(v) = op.reference_contract_id { o.reference_contract_id = Some(v); }
    if let Some(v) = op.is_pegged_change_amount_decrease { o.is_pegged_change_amount_decrease = v; }
    if let Some(v) = op.pegged_change_amount { o.pegged_change_amount = Some(v); }
    if let Some(v) = op.reference_change_amount { o.reference_change_amount = Some(v); }
    if let Some(ref v) = op.reference_exchange_id { o.reference_exchange_id.clone_from(v); }

    // --- Conditions ---
    let conditions = decode_conditions_pb(&op.conditions);
    if !conditions.is_empty() {
        o.conditions = conditions;
    }
    if let Some(v) = op.conditions_ignore_rth { o.conditions_ignore_rth = v; }
    if let Some(v) = op.conditions_cancel_order { o.conditions_cancel_order = v; }

    // --- Adjusted ---
    if let Some(ref v) = op.adjusted_order_type { o.adjusted_order_type.clone_from(v); }
    if let Some(v) = op.trigger_price { o.trigger_price = Some(v); }
    if let Some(v) = op.lmt_price_offset { o.lmt_price_offset = Some(v); }
    if let Some(v) = op.adjusted_stop_price { o.adjusted_stop_price = Some(v); }
    if let Some(v) = op.adjusted_stop_limit_price { o.adjusted_stop_limit_price = Some(v); }
    if let Some(v) = op.adjusted_trailing_amount { o.adjusted_trailing_amount = Some(v); }
    if let Some(v) = op.adjustable_trailing_unit { o.adjustable_trailing_unit = Some(v); }

    // --- SoftDollarTier ---
    if let Some(ref sdt) = op.soft_dollar_tier {
        o.soft_dollar_tier = SoftDollarTier {
            name: sdt.name.clone().unwrap_or_default(),
            val: sdt.value.clone().unwrap_or_default(),
            display_name: sdt.display_name.clone().unwrap_or_default(),
        };
    }

    // --- More misc ---
    if let Some(v) = op.cash_qty { o.cash_qty = Some(v); }
    if let Some(v) = op.dont_use_auto_price_for_hedge { o.dont_use_auto_price_for_hedge = v; }
    if let Some(v) = op.is_oms_container { o.is_oms_container = v; }
    if let Some(v) = op.discretionary_up_to_limit_price { o.discretionary_up_to_limit_price = v; }
    if let Some(v) = op.use_price_mgmt_algo {
        o.use_price_mgmt_algo = if v != 0 { UsePriceMgmtAlgo::Use } else { UsePriceMgmtAlgo::DontUse };
    }
    if let Some(v) = op.duration { o.duration = Some(v); }
    if let Some(v) = op.post_to_ats { o.post_to_ats = Some(v); }
    if let Some(v) = op.auto_cancel_parent { o.auto_cancel_parent = v; }
    if let Some(v) = op.min_trade_qty { o.min_trade_qty = Some(v); }
    if let Some(v) = op.min_compete_size { o.min_compete_size = Some(v); }
    if let Some(v) = op.compete_against_best_offset { o.compete_against_best_offset = Some(v); }
    if let Some(v) = op.mid_offset_at_whole { o.mid_offset_at_whole = Some(v); }
    if let Some(v) = op.mid_offset_at_half { o.mid_offset_at_half = Some(v); }
    if let Some(ref v) = op.customer_account { o.customer_account.clone_from(v); }
    if let Some(v) = op.professional_customer { o.professional_customer = v; }
    if let Some(ref v) = op.bond_accrued_interest { o.bond_accrued_interest.clone_from(v); }
    if let Some(v) = op.include_overnight { o.include_overnight = v; }
    if let Some(ref v) = op.ext_operator { o.ext_operator.clone_from(v); }
    if let Some(v) = op.manual_order_indicator { o.manual_order_indicator = Some(v); }
    if let Some(ref v) = op.submitter { o.submitter.clone_from(v); }
    if let Some(v) = op.imbalance_only { o.imbalance_only = v; }

    // --- Active times ---
    if let Some(ref v) = op.active_start_time { o.active_start_time.clone_from(v); }
    if let Some(ref v) = op.active_stop_time { o.active_stop_time.clone_from(v); }
    if let Some(ref v) = op.auto_cancel_date { o.auto_cancel_date.clone_from(v); }

    o
}

/// Convert protobuf OrderState → Rust OrderState.
/// C++: `EDecoderUtils::decodeOrderState`
fn decode_order_state_pb(os: &pb::OrderState) -> OrderState {
    let mut s = OrderState::default();

    if let Some(ref v) = os.status { s.status.clone_from(v); }
    if let Some(v) = os.init_margin_before { s.init_margin_before = v.to_string(); }
    if let Some(v) = os.maint_margin_before { s.maint_margin_before = v.to_string(); }
    if let Some(v) = os.equity_with_loan_before { s.equity_with_loan_before = v.to_string(); }
    if let Some(v) = os.init_margin_change { s.init_margin_change = v.to_string(); }
    if let Some(v) = os.maint_margin_change { s.maint_margin_change = v.to_string(); }
    if let Some(v) = os.equity_with_loan_change { s.equity_with_loan_change = v.to_string(); }
    if let Some(v) = os.init_margin_after { s.init_margin_after = v.to_string(); }
    if let Some(v) = os.maint_margin_after { s.maint_margin_after = v.to_string(); }
    if let Some(v) = os.equity_with_loan_after { s.equity_with_loan_after = v.to_string(); }
    if let Some(v) = os.commission_and_fees { s.commission_and_fees = Some(v); }
    if let Some(v) = os.min_commission_and_fees { s.min_commission_and_fees = Some(v); }
    if let Some(v) = os.max_commission_and_fees { s.max_commission_and_fees = Some(v); }
    if let Some(ref v) = os.commission_and_fees_currency { s.commission_and_fees_currency.clone_from(v); }
    if let Some(ref v) = os.margin_currency { s.margin_currency.clone_from(v); }
    if let Some(ref v) = os.warning_text { s.warning_text.clone_from(v); }

    // Outside RTH margins
    if let Some(v) = os.init_margin_before_outside_rth { s.init_margin_before_outside_rth = Some(v); }
    if let Some(v) = os.maint_margin_before_outside_rth { s.maint_margin_before_outside_rth = Some(v); }
    if let Some(v) = os.equity_with_loan_before_outside_rth { s.equity_with_loan_before_outside_rth = Some(v); }
    if let Some(v) = os.init_margin_change_outside_rth { s.init_margin_change_outside_rth = Some(v); }
    if let Some(v) = os.maint_margin_change_outside_rth { s.maint_margin_change_outside_rth = Some(v); }
    if let Some(v) = os.equity_with_loan_change_outside_rth { s.equity_with_loan_change_outside_rth = Some(v); }
    if let Some(v) = os.init_margin_after_outside_rth { s.init_margin_after_outside_rth = Some(v); }
    if let Some(v) = os.maint_margin_after_outside_rth { s.maint_margin_after_outside_rth = Some(v); }
    if let Some(v) = os.equity_with_loan_after_outside_rth { s.equity_with_loan_after_outside_rth = Some(v); }

    if let Some(ref v) = os.suggested_size {
        s.suggested_size = Decimal::from_str(v).ok();
    }
    if let Some(ref v) = os.reject_reason { s.reject_reason.clone_from(v); }

    // OrderAllocations
    if !os.order_allocations.is_empty() {
        let allocs: Vec<OrderAllocation> = os.order_allocations.iter().map(|a| {
            OrderAllocation {
                account: a.account.clone().unwrap_or_default(),
                position: a.position.as_ref().and_then(|v| Decimal::from_str(v).ok()),
                position_desired: a.position_desired.as_ref().and_then(|v| Decimal::from_str(v).ok()),
                position_after: a.position_after.as_ref().and_then(|v| Decimal::from_str(v).ok()),
                desired_alloc_qty: a.desired_alloc_qty.as_ref().and_then(|v| Decimal::from_str(v).ok()),
                allowed_alloc_qty: a.allowed_alloc_qty.as_ref().and_then(|v| Decimal::from_str(v).ok()),
                is_monetary: a.is_monetary.unwrap_or(false),
            }
        }).collect();
        s.order_allocations = Some(allocs);
    }

    if let Some(ref v) = os.completed_time { s.completed_time.clone_from(v); }
    if let Some(ref v) = os.completed_status { s.completed_status.clone_from(v); }

    s
}

// ============================================================================
// Utility helpers
// ============================================================================

/// Parse an optional string field as Decimal, defaulting to UNSET_DECIMAL on missing/invalid.
fn parse_decimal_opt(s: &Option<String>) -> Decimal {
    match s {
        Some(v) => Decimal::from_str(v).unwrap_or(Decimal::MAX),
        None => Decimal::MAX,
    }
}

/// Convert a protobuf `map<string, string>` → `Vec<TagValue>`.
fn decode_tag_value_map(m: &std::collections::HashMap<String, String>) -> Vec<TagValue> {
    m.iter()
        .map(|(k, v)| TagValue {
            tag: k.clone(),
            value: v.clone(),
        })
        .collect()
}

/// Convert protobuf OrderConditions → Rust OrderConditions.
/// C++: `EDecoderUtils::decodeConditions`
fn decode_conditions_pb(conditions: &[pb::OrderCondition]) -> Vec<OrderCondition> {
    conditions
        .iter()
        .filter_map(|c| {
            let cond_type = OrderConditionType::try_from(c.r#type.unwrap_or(0)).ok()?;
            let conj = c.is_conjunction_connection.unwrap_or(true);
            let is_more = c.is_more.unwrap_or(false);

            Some(match cond_type {
                OrderConditionType::Price => OrderCondition::Price {
                    is_conjunction_connection: conj,
                    is_more,
                    con_id: c.con_id.unwrap_or(0),
                    exchange: c.exchange.clone().unwrap_or_default(),
                    price: c.price.unwrap_or(0.0),
                    trigger_method: TriggerMethod::try_from(
                        c.trigger_method.unwrap_or(0),
                    )
                    .unwrap_or(TriggerMethod::Default),
                },
                OrderConditionType::Time => OrderCondition::Time {
                    is_conjunction_connection: conj,
                    is_more,
                    time: c.time.clone().unwrap_or_default(),
                },
                OrderConditionType::Margin => OrderCondition::Margin {
                    is_conjunction_connection: conj,
                    is_more,
                    percent: c.percent.unwrap_or(0),
                },
                OrderConditionType::Execution => OrderCondition::Execution {
                    is_conjunction_connection: conj,
                    exchange: c.exchange.clone().unwrap_or_default(),
                    sec_type: c.sec_type.clone().unwrap_or_default(),
                    symbol: c.symbol.clone().unwrap_or_default(),
                },
                OrderConditionType::Volume => OrderCondition::Volume {
                    is_conjunction_connection: conj,
                    is_more,
                    con_id: c.con_id.unwrap_or(0),
                    exchange: c.exchange.clone().unwrap_or_default(),
                    volume: c.volume.unwrap_or(0),
                },
                OrderConditionType::PercentChange => OrderCondition::PercentChange {
                    is_conjunction_connection: conj,
                    is_more,
                    con_id: c.con_id.unwrap_or(0),
                    exchange: c.exchange.clone().unwrap_or_default(),
                    change_percent: c.change_percent,
                },
            })
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    /// Helper: encode a protobuf message to bytes.
    fn encode_pb<M: Message>(msg: &M) -> Vec<u8> {
        msg.encode_to_vec()
    }

    #[test]
    fn decode_order_status_protobuf() {
        let proto = pb::OrderStatus {
            order_id: Some(42),
            status: Some("Filled".into()),
            filled: Some("100".into()),
            remaining: Some("0".into()),
            avg_fill_price: Some(150.5),
            perm_id: Some(9999),
            parent_id: Some(0),
            last_fill_price: Some(150.5),
            client_id: Some(1),
            why_held: Some("".into()),
            mkt_cap_price: Some(0.0),
        };
        let data = encode_pb(&proto);

        match decode_protobuf_msg(3, &data).unwrap() {
            IBEvent::OrderStatus {
                order_id,
                status,
                filled,
                remaining,
                avg_fill_price,
                perm_id,
                client_id,
                ..
            } => {
                assert_eq!(order_id, 42);
                assert_eq!(status, "Filled");
                assert_eq!(filled, Decimal::from(100));
                assert_eq!(remaining, Decimal::from(0));
                assert!((avg_fill_price - 150.5).abs() < f64::EPSILON);
                assert_eq!(perm_id, 9999);
                assert_eq!(client_id, 1);
            }
            other => panic!("expected OrderStatus, got {other:?}"),
        }
    }

    #[test]
    fn decode_error_msg_protobuf() {
        let proto = pb::ErrorMessage {
            id: Some(1),
            error_time: Some(1700000000),
            error_code: Some(200),
            error_msg: Some("No security definition".into()),
            advanced_order_reject_json: Some("{}".into()),
        };
        let data = encode_pb(&proto);

        match decode_protobuf_msg(4, &data).unwrap() {
            IBEvent::Error {
                req_id,
                error_time,
                code,
                message,
                advanced_order_reject_json,
            } => {
                assert_eq!(req_id, 1);
                assert_eq!(error_time, 1700000000);
                assert_eq!(code, 200);
                assert_eq!(message, "No security definition");
                assert_eq!(advanced_order_reject_json, "{}");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn decode_open_order_end_protobuf() {
        let proto = pb::OpenOrdersEnd {};
        let data = encode_pb(&proto);

        match decode_protobuf_msg(53, &data).unwrap() {
            IBEvent::OpenOrderEnd => {}
            other => panic!("expected OpenOrderEnd, got {other:?}"),
        }
    }

    #[test]
    fn decode_execution_details_end_protobuf() {
        let proto = pb::ExecutionDetailsEnd {
            req_id: Some(7),
        };
        let data = encode_pb(&proto);

        match decode_protobuf_msg(55, &data).unwrap() {
            IBEvent::ExecDetailsEnd { req_id } => {
                assert_eq!(req_id, 7);
            }
            other => panic!("expected ExecDetailsEnd, got {other:?}"),
        }
    }

    #[test]
    fn decode_open_order_protobuf_basic() {
        let proto = pb::OpenOrder {
            order_id: Some(101),
            contract: Some(pb::Contract {
                con_id: Some(265598),
                symbol: Some("AAPL".into()),
                sec_type: Some("STK".into()),
                exchange: Some("SMART".into()),
                currency: Some("USD".into()),
                ..Default::default()
            }),
            order: Some(pb::Order {
                client_id: Some(1),
                order_id: Some(101),
                perm_id: Some(55555),
                action: Some("BUY".into()),
                total_quantity: Some("100".into()),
                order_type: Some("LMT".into()),
                lmt_price: Some(150.0),
                tif: Some("GTC".into()),
                ..Default::default()
            }),
            order_state: Some(pb::OrderState {
                status: Some("PreSubmitted".into()),
                init_margin_before: Some(5000.0),
                commission_and_fees: Some(1.0),
                ..Default::default()
            }),
        };
        let data = encode_pb(&proto);

        match decode_protobuf_msg(5, &data).unwrap() {
            IBEvent::OpenOrder {
                order_id,
                contract,
                order,
                order_state,
            } => {
                assert_eq!(order_id, 101);
                assert_eq!(contract.symbol, "AAPL");
                assert_eq!(contract.con_id, 265598);
                assert_eq!(contract.currency, "USD");
                assert_eq!(order.perm_id, 55555);
                assert_eq!(order.total_quantity, Some(Decimal::from(100)));
                assert_eq!(order.lmt_price, Some(150.0));
                assert_eq!(order_state.status, "PreSubmitted");
                assert_eq!(order_state.init_margin_before, "5000");
                assert_eq!(order_state.commission_and_fees, Some(1.0));
            }
            other => panic!("expected OpenOrder, got {other:?}"),
        }
    }

    #[test]
    fn decode_execution_details_protobuf() {
        let proto = pb::ExecutionDetails {
            req_id: Some(3),
            contract: Some(pb::Contract {
                con_id: Some(265598),
                symbol: Some("AAPL".into()),
                sec_type: Some("STK".into()),
                ..Default::default()
            }),
            execution: Some(pb::Execution {
                order_id: Some(42),
                exec_id: Some("0001f4e8.66d9a5c4.01.01".into()),
                time: Some("20260222 10:30:00".into()),
                side: Some("BOT".into()),
                shares: Some("50".into()),
                price: Some(151.25),
                perm_id: Some(88888),
                client_id: Some(1),
                cum_qty: Some("50".into()),
                avg_price: Some(151.25),
                ..Default::default()
            }),
        };
        let data = encode_pb(&proto);

        match decode_protobuf_msg(11, &data).unwrap() {
            IBEvent::ExecDetails {
                req_id,
                contract,
                execution,
            } => {
                assert_eq!(req_id, 3);
                assert_eq!(contract.symbol, "AAPL");
                assert_eq!(execution.order_id, 42);
                assert_eq!(execution.exec_id, "0001f4e8.66d9a5c4.01.01");
                assert_eq!(execution.shares, Some(Decimal::from(50)));
                assert!((execution.price - 151.25).abs() < f64::EPSILON);
                assert_eq!(execution.perm_id, 88888);
            }
            other => panic!("expected ExecDetails, got {other:?}"),
        }
    }

    #[test]
    fn decode_open_order_with_conditions_protobuf() {
        let proto = pb::OpenOrder {
            order_id: Some(200),
            contract: Some(pb::Contract {
                symbol: Some("MSFT".into()),
                ..Default::default()
            }),
            order: Some(pb::Order {
                action: Some("SELL".into()),
                total_quantity: Some("50".into()),
                order_type: Some("MKT".into()),
                conditions: vec![
                    pb::OrderCondition {
                        r#type: Some(1), // Price
                        is_conjunction_connection: Some(true),
                        is_more: Some(true),
                        con_id: Some(265598),
                        exchange: Some("SMART".into()),
                        price: Some(400.0),
                        trigger_method: Some(0),
                        ..Default::default()
                    },
                    pb::OrderCondition {
                        r#type: Some(3), // Time
                        is_conjunction_connection: Some(false),
                        is_more: Some(true),
                        time: Some("20260301 09:30:00".into()),
                        ..Default::default()
                    },
                ],
                conditions_cancel_order: Some(true),
                conditions_ignore_rth: Some(false),
                ..Default::default()
            }),
            order_state: Some(pb::OrderState::default()),
        };
        let data = encode_pb(&proto);

        match decode_protobuf_msg(5, &data).unwrap() {
            IBEvent::OpenOrder { order, .. } => {
                assert_eq!(order.conditions.len(), 2);
                match &order.conditions[0] {
                    OrderCondition::Price { price, con_id, .. } => {
                        assert!((price - 400.0).abs() < f64::EPSILON);
                        assert_eq!(*con_id, 265598);
                    }
                    other => panic!("expected Price condition, got {other:?}"),
                }
                match &order.conditions[1] {
                    OrderCondition::Time { time, .. } => {
                        assert_eq!(time, "20260301 09:30:00");
                    }
                    other => panic!("expected Time condition, got {other:?}"),
                }
                assert!(order.conditions_cancel_order);
                assert!(!order.conditions_ignore_rth);
            }
            other => panic!("expected OpenOrder, got {other:?}"),
        }
    }

    #[test]
    fn decode_unknown_protobuf_msg_id() {
        let result = decode_protobuf_msg(999, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn decode_empty_protobuf_data() {
        // Empty data should still decode (all fields will be defaults)
        match decode_protobuf_msg(3, &[]).unwrap() {
            IBEvent::OrderStatus { order_id, status, .. } => {
                assert_eq!(order_id, 0);
                assert_eq!(status, "");
            }
            other => panic!("expected OrderStatus, got {other:?}"),
        }
    }

    #[test]
    fn decode_order_with_soft_dollar_tier_pb() {
        let proto = pb::OpenOrder {
            order_id: Some(300),
            contract: Some(pb::Contract::default()),
            order: Some(pb::Order {
                action: Some("BUY".into()),
                soft_dollar_tier: Some(pb::SoftDollarTier {
                    name: Some("Tier1".into()),
                    value: Some("1".into()),
                    display_name: Some("Tier One".into()),
                }),
                ..Default::default()
            }),
            order_state: Some(pb::OrderState::default()),
        };
        let data = encode_pb(&proto);

        match decode_protobuf_msg(5, &data).unwrap() {
            IBEvent::OpenOrder { order, .. } => {
                assert_eq!(order.soft_dollar_tier.name, "Tier1");
                assert_eq!(order.soft_dollar_tier.val, "1");
                assert_eq!(order.soft_dollar_tier.display_name, "Tier One");
            }
            other => panic!("expected OpenOrder, got {other:?}"),
        }
    }

    #[test]
    fn decode_order_state_with_allocations_pb() {
        let proto = pb::OpenOrder {
            order_id: Some(400),
            contract: Some(pb::Contract::default()),
            order: Some(pb::Order::default()),
            order_state: Some(pb::OrderState {
                status: Some("Submitted".into()),
                commission_and_fees: Some(2.5),
                order_allocations: vec![
                    pb::OrderAllocation {
                        account: Some("DU12345".into()),
                        position: Some("100".into()),
                        is_monetary: Some(false),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
        };
        let data = encode_pb(&proto);

        match decode_protobuf_msg(5, &data).unwrap() {
            IBEvent::OpenOrder { order_state, .. } => {
                assert_eq!(order_state.status, "Submitted");
                assert_eq!(order_state.commission_and_fees, Some(2.5));
                let allocs = order_state.order_allocations.as_ref().unwrap();
                assert_eq!(allocs.len(), 1);
                assert_eq!(allocs[0].account, "DU12345");
                assert_eq!(allocs[0].position, Some(Decimal::from(100)));
            }
            other => panic!("expected OpenOrder, got {other:?}"),
        }
    }

    #[test]
    fn decode_contract_with_combo_legs_pb() {
        let proto = pb::ExecutionDetails {
            req_id: Some(5),
            contract: Some(pb::Contract {
                symbol: Some("SPX".into()),
                sec_type: Some("BAG".into()),
                combo_legs: vec![
                    pb::ComboLeg {
                        con_id: Some(100),
                        ratio: Some(1),
                        action: Some("BUY".into()),
                        exchange: Some("CBOE".into()),
                        open_close: Some(1),
                        ..Default::default()
                    },
                    pb::ComboLeg {
                        con_id: Some(200),
                        ratio: Some(1),
                        action: Some("SELL".into()),
                        exchange: Some("CBOE".into()),
                        open_close: Some(2),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            execution: Some(pb::Execution::default()),
        };
        let data = encode_pb(&proto);

        match decode_protobuf_msg(11, &data).unwrap() {
            IBEvent::ExecDetails { contract, .. } => {
                assert_eq!(contract.symbol, "SPX");
                let legs = contract.combo_legs.as_ref().unwrap();
                assert_eq!(legs.len(), 2);
                assert_eq!(legs[0].con_id, 100);
                assert_eq!(legs[0].open_close, LegOpenClose::Open);
                assert_eq!(legs[1].con_id, 200);
                assert_eq!(legs[1].open_close, LegOpenClose::Close);
            }
            other => panic!("expected ExecDetails, got {other:?}"),
        }
    }
}
