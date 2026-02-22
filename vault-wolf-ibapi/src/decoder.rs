//! IB TWS API message decoder.
//!
//! Decodes incoming messages from the IB wire format: null-terminated ASCII
//! fields parsed from a byte buffer using a cursor/position-tracking pattern.
//!
//! Ported from: `EDecoder::DecodeField*`, `DecodeFieldMax`, `DecodeRawInt`,
//! `FindFieldEnd`, `CheckOffset`.

// Decoder functions sequentially assign fields from wire data,
// which is inherently incompatible with struct-literal initialization.
#![allow(clippy::field_reassign_with_default)]

use rust_decimal::Decimal;
use std::fmt;
use std::str::FromStr;

use crate::errors::{IBApiError, Result};
use crate::models::bar::{Bar, HistoricalSession, HistoricalTick, HistoricalTickBidAsk, HistoricalTickLast};
use crate::models::common::{
    FamilyCode, HistogramEntry, IneligibilityReason, NewsProvider, PriceIncrement, SmartComponent,
    SoftDollarTier, TagValue,
};
use crate::models::contract::{
    ComboLeg, Contract, ContractDescription, ContractDetails, DeltaNeutralContract,
};
use crate::models::enums::*;
use crate::models::execution::{CommissionAndFeesReport, Execution};
use crate::models::market_data::{DepthMktDataDescription, TickAttrib, TickAttribBidAsk, TickAttribLast};
use crate::models::order::{Order, OrderAllocation, OrderComboLeg, OrderCondition, OrderState};
use crate::protocol::{incoming, server_version, TickType, RAW_INT_LEN};
use crate::wrapper::{IBEvent, ScannerDataItem};

// ============================================================================
// MessageDecoder
// ============================================================================

/// Decodes IB API wire-format message fields from a byte buffer.
///
/// Wraps a byte slice and tracks the current read position. Each `decode_*`
/// method reads the next field (bytes up to null terminator), parses it into
/// the requested type, and advances the position.
///
/// Mirrors C++ `EDecoder::DecodeField`, `DecodeFieldMax`, `DecodeRawInt`.
pub struct MessageDecoder<'a> {
    data: &'a [u8],
    pos: usize,
    server_version: i32,
}

impl<'a> MessageDecoder<'a> {
    /// Create a decoder over a message body (without the 4-byte length header).
    pub fn new(data: &'a [u8], server_version: i32) -> Self {
        Self {
            data,
            pos: 0,
            server_version,
        }
    }

    pub fn server_version(&self) -> i32 {
        self.server_version
    }

    /// Check if there are more bytes to decode.
    ///
    /// Mirrors C++ `EDecoder::CheckOffset`.
    pub fn has_remaining(&self) -> bool {
        self.pos < self.data.len()
    }

    /// Return remaining undecoded bytes.
    pub fn remaining(&self) -> &[u8] {
        &self.data[self.pos..]
    }

    /// Current position in the buffer.
    pub fn position(&self) -> usize {
        self.pos
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Find the next null terminator starting from current position.
    /// Returns the index of the null byte within `self.data`.
    ///
    /// Mirrors C++ `EDecoder::FindFieldEnd`.
    fn find_field_end(&self) -> Result<usize> {
        self.data[self.pos..]
            .iter()
            .position(|&b| b == 0)
            .map(|offset| self.pos + offset)
            .ok_or_else(|| {
                IBApiError::Decoding("field not null-terminated".into())
            })
    }

    /// Read the raw bytes of the next field (up to but not including null),
    /// advance position past the null, return the field as a `&str`.
    fn read_field_str(&mut self) -> Result<&'a str> {
        if !self.has_remaining() {
            return Err(IBApiError::Decoding(
                "unexpected end of message".into(),
            ));
        }
        let end = self.find_field_end()?;
        let field = std::str::from_utf8(&self.data[self.pos..end]).map_err(
            |e| IBApiError::Decoding(format!("invalid UTF-8: {e}")),
        )?;
        self.pos = end + 1; // advance past the null byte
        Ok(field)
    }

    // ========================================================================
    // Type-specific decoders
    // ========================================================================

    /// Decode a String field.
    ///
    /// Mirrors C++ `DecodeField(std::string&, ...)`.
    pub fn decode_string(&mut self) -> Result<String> {
        self.read_field_str().map(|s| s.to_string())
    }

    /// Decode an i32 field.
    ///
    /// Empty string → 0 (matching C++ `atoi("")` behavior).
    /// Mirrors C++ `DecodeField(int&, ...)`.
    pub fn decode_i32(&mut self) -> Result<i32> {
        let s = self.read_field_str()?;
        if s.is_empty() {
            return Ok(0);
        }
        s.parse::<i32>()
            .map_err(|e| IBApiError::Decoding(format!("invalid i32 '{s}': {e}")))
    }

    /// Decode an i64 field.
    ///
    /// Empty string → 0 (matching C++ `atoll("")` behavior).
    /// Mirrors C++ `DecodeField(long long&, ...)`.
    pub fn decode_i64(&mut self) -> Result<i64> {
        let s = self.read_field_str()?;
        if s.is_empty() {
            return Ok(0);
        }
        s.parse::<i64>()
            .map_err(|e| IBApiError::Decoding(format!("invalid i64 '{s}': {e}")))
    }

    /// Decode a f64 field.
    ///
    /// Handles `"Infinity"` string. Empty string → 0.0 (matching C++ `atof("")`).
    /// Mirrors C++ `DecodeField(double&, ...)`.
    pub fn decode_f64(&mut self) -> Result<f64> {
        let s = self.read_field_str()?;
        if s.is_empty() {
            return Ok(0.0);
        }
        if s == "Infinity" {
            return Ok(f64::INFINITY);
        }
        s.parse::<f64>()
            .map_err(|e| IBApiError::Decoding(format!("invalid f64 '{s}': {e}")))
    }

    /// Decode a bool field.
    ///
    /// C++ decodes int, then `> 0 → true`.
    /// Mirrors C++ `DecodeField(bool&, ...)`.
    pub fn decode_bool(&mut self) -> Result<bool> {
        self.decode_i32().map(|v| v > 0)
    }

    /// Decode a Decimal field.
    ///
    /// Mirrors C++ `DecodeField(Decimal&, ...)`.
    pub fn decode_decimal(&mut self) -> Result<Decimal> {
        let s = self.read_field_str()?;
        if s.is_empty() {
            return Ok(Decimal::ZERO);
        }
        Decimal::from_str(s)
            .map_err(|e| IBApiError::Decoding(format!("invalid Decimal '{s}': {e}")))
    }

    /// Decode a time field (i64 from string).
    ///
    /// Mirrors C++ `DecodeFieldTime`.
    pub fn decode_time(&mut self) -> Result<i64> {
        self.decode_i64()
    }

    // ========================================================================
    // "Max" decoders — empty string → None (C++ UNSET sentinel)
    // ========================================================================

    /// Decode Option<i32>: empty string → None, else Some(parsed).
    ///
    /// Mirrors C++ `DecodeFieldMax(int&, ...)` where empty → UNSET_INTEGER.
    pub fn decode_i32_max(&mut self) -> Result<Option<i32>> {
        let s = self.read_field_str()?;
        if s.is_empty() {
            return Ok(None);
        }
        s.parse::<i32>()
            .map(Some)
            .map_err(|e| IBApiError::Decoding(format!("invalid i32 '{s}': {e}")))
    }

    /// Decode Option<i64>: empty string → None, else Some(parsed).
    pub fn decode_i64_max(&mut self) -> Result<Option<i64>> {
        let s = self.read_field_str()?;
        if s.is_empty() {
            return Ok(None);
        }
        s.parse::<i64>()
            .map(Some)
            .map_err(|e| IBApiError::Decoding(format!("invalid i64 '{s}': {e}")))
    }

    /// Decode Option<f64>: empty string → None, else Some(parsed).
    ///
    /// Mirrors C++ `DecodeFieldMax(double&, ...)` where empty → UNSET_DOUBLE.
    pub fn decode_f64_max(&mut self) -> Result<Option<f64>> {
        let s = self.read_field_str()?;
        if s.is_empty() {
            return Ok(None);
        }
        if s == "Infinity" {
            return Ok(Some(f64::INFINITY));
        }
        s.parse::<f64>()
            .map(Some)
            .map_err(|e| IBApiError::Decoding(format!("invalid f64 '{s}': {e}")))
    }

    /// Decode Option<Decimal>: empty string → None, else Some(parsed).
    pub fn decode_decimal_max(&mut self) -> Result<Option<Decimal>> {
        let s = self.read_field_str()?;
        if s.is_empty() {
            return Ok(None);
        }
        Decimal::from_str(s)
            .map(Some)
            .map_err(|e| IBApiError::Decoding(format!("invalid Decimal '{s}': {e}")))
    }

    // ========================================================================
    // Raw integer decoder (4-byte big-endian, no null terminator)
    // ========================================================================

    /// Decode a 4-byte big-endian integer (no null terminator).
    ///
    /// Used for message IDs when `server_version >= MIN_SERVER_VER_PROTOBUF`.
    /// Mirrors C++ `EDecoder::DecodeRawInt`.
    pub fn decode_raw_int(&mut self) -> Result<i32> {
        if self.data.len() - self.pos < RAW_INT_LEN {
            return Err(IBApiError::Decoding(
                "not enough bytes for raw int".into(),
            ));
        }
        let bytes: [u8; 4] = self.data[self.pos..self.pos + RAW_INT_LEN]
            .try_into()
            .expect("slice length verified above");
        self.pos += RAW_INT_LEN;
        Ok(i32::from_be_bytes(bytes))
    }

    // ========================================================================
    // Context-dependent message ID decoding
    // ========================================================================

    /// Decode a message ID: raw int for protobuf-capable servers, else text.
    ///
    /// Mirrors the logic in C++ `EDecoder::parseAndProcessMsg`.
    pub fn decode_msg_id(&mut self) -> Result<i32> {
        if self.server_version >= server_version::PROTOBUF {
            self.decode_raw_int()
        } else {
            self.decode_i32()
        }
    }

    // ========================================================================
    // Enum decoders
    // ========================================================================

    /// Decode a string field and parse it via `FromStr` into a typed enum.
    ///
    /// Works with SecType, OrderType, Action, etc.
    pub fn decode_enum<T: FromStr>(&mut self) -> Result<T>
    where
        T::Err: fmt::Display,
    {
        let s = self.read_field_str()?;
        s.parse::<T>().map_err(|e| {
            IBApiError::Decoding(format!("invalid enum value '{s}': {e}"))
        })
    }

    /// Decode an optional enum: empty string → None.
    pub fn decode_enum_opt<T: FromStr>(&mut self) -> Result<Option<T>>
    where
        T::Err: fmt::Display,
    {
        let s = self.read_field_str()?;
        if s.is_empty() {
            return Ok(None);
        }
        s.parse::<T>().map(Some).map_err(|e| {
            IBApiError::Decoding(format!("invalid enum value '{s}': {e}"))
        })
    }

    // ========================================================================
    // Skip helpers
    // ========================================================================

    /// Skip the next field without decoding it.
    pub fn skip_field(&mut self) -> Result<()> {
        let _ = self.read_field_str()?;
        Ok(())
    }

    /// Skip N fields.
    pub fn skip_fields(&mut self, n: usize) -> Result<()> {
        for _ in 0..n {
            self.skip_field()?;
        }
        Ok(())
    }
}

// ============================================================================
// Server Message Dispatch
// ============================================================================

/// Decode a complete server message into an `IBEvent`.
///
/// This is the main entry point for message dispatch, corresponding to
/// C++ `EDecoder::parseAndProcessMsg`. It reads the message ID, then
/// dispatches to the appropriate per-message decoder.
///
/// Messages that are not yet implemented return `IBEvent::Unknown`.
pub fn decode_server_msg(data: &[u8], server_version: i32) -> IBEvent {
    match decode_server_msg_inner(data, server_version) {
        Ok(event) => event,
        Err(e) => {
            tracing::error!("failed to decode server message: {e}");
            IBEvent::Unknown {
                msg_id: -1,
                data: data.to_vec(),
            }
        }
    }
}

/// Inner implementation that returns Result for cleaner error handling.
fn decode_server_msg_inner(data: &[u8], server_version: i32) -> Result<IBEvent> {
    let mut dec = MessageDecoder::new(data, server_version);
    let msg_id = dec.decode_msg_id()?;

    // Protobuf detection: if msg_id > PROTOBUF_MSG_ID (200), the remaining
    // bytes are a protobuf-encoded message. Subtract 200 to get the real
    // message type and delegate to the protobuf decoder.
    if msg_id > crate::protocol::outgoing::PROTOBUF_MSG_ID {
        let real_msg_id = msg_id - crate::protocol::outgoing::PROTOBUF_MSG_ID;
        let remaining = dec.remaining();
        return crate::proto_decode::decode_protobuf_msg(real_msg_id, remaining);
    }

    match msg_id {
        // Connection & Error (Phase 3)
        incoming::ERR_MSG => decode_err_msg(&mut dec),
        incoming::NEXT_VALID_ID => decode_next_valid_id(&mut dec),
        incoming::MANAGED_ACCTS => decode_managed_accts(&mut dec),
        incoming::CURRENT_TIME => decode_current_time(&mut dec),
        incoming::CURRENT_TIME_IN_MILLIS => decode_current_time_in_millis(&mut dec),
        // Market Data — Ticks
        incoming::TICK_PRICE => decode_tick_price(&mut dec),
        incoming::TICK_SIZE => decode_tick_size(&mut dec),
        incoming::TICK_OPTION_COMPUTATION => decode_tick_option_computation(&mut dec),
        incoming::TICK_GENERIC => decode_tick_generic(&mut dec),
        incoming::TICK_STRING => decode_tick_string(&mut dec),
        incoming::TICK_EFP => decode_tick_efp(&mut dec),
        incoming::TICK_SNAPSHOT_END => decode_tick_snapshot_end(&mut dec),
        incoming::TICK_REQ_PARAMS => decode_tick_req_params(&mut dec),
        incoming::TICK_NEWS => decode_tick_news(&mut dec),
        incoming::MARKET_DATA_TYPE => decode_market_data_type(&mut dec),
        incoming::TICK_BY_TICK => decode_tick_by_tick(&mut dec),
        // Orders
        incoming::ORDER_STATUS => decode_order_status(&mut dec),
        incoming::OPEN_ORDER => decode_open_order(&mut dec),
        incoming::OPEN_ORDER_END => Ok(IBEvent::OpenOrderEnd),
        incoming::ORDER_BOUND => decode_order_bound(&mut dec),
        incoming::COMPLETED_ORDER => decode_completed_order(&mut dec),
        incoming::COMPLETED_ORDERS_END => Ok(IBEvent::CompletedOrdersEnd),
        // Execution
        incoming::EXECUTION_DATA => decode_execution_data(&mut dec),
        incoming::EXECUTION_DATA_END => decode_execution_data_end(&mut dec),
        incoming::COMMISSION_AND_FEES_REPORT => decode_commission_report(&mut dec),
        // Account
        incoming::ACCT_VALUE => decode_acct_value(&mut dec),
        incoming::PORTFOLIO_VALUE => decode_portfolio_value(&mut dec),
        incoming::ACCT_UPDATE_TIME => decode_acct_update_time(&mut dec),
        incoming::ACCT_DOWNLOAD_END => decode_acct_download_end(&mut dec),
        incoming::ACCOUNT_SUMMARY => decode_account_summary(&mut dec),
        incoming::ACCOUNT_SUMMARY_END => decode_account_summary_end(&mut dec),
        incoming::POSITION_DATA => decode_position_data(&mut dec),
        incoming::POSITION_END => Ok(IBEvent::PositionEnd),
        incoming::POSITION_MULTI => decode_position_multi(&mut dec),
        incoming::POSITION_MULTI_END => decode_position_multi_end(&mut dec),
        incoming::ACCOUNT_UPDATE_MULTI => decode_account_update_multi(&mut dec),
        incoming::ACCOUNT_UPDATE_MULTI_END => decode_account_update_multi_end(&mut dec),
        // Contract
        incoming::CONTRACT_DATA => decode_contract_data(&mut dec),
        incoming::BOND_CONTRACT_DATA => decode_bond_contract_data(&mut dec),
        incoming::CONTRACT_DATA_END => decode_contract_data_end(&mut dec),
        incoming::SYMBOL_SAMPLES => decode_symbol_samples(&mut dec),
        incoming::DELTA_NEUTRAL_VALIDATION => decode_delta_neutral_validation(&mut dec),
        incoming::SECURITY_DEFINITION_OPTION_PARAMETER => decode_sec_def_opt_params(&mut dec),
        incoming::SECURITY_DEFINITION_OPTION_PARAMETER_END => decode_sec_def_opt_params_end(&mut dec),
        // Market Depth
        incoming::MARKET_DEPTH => decode_market_depth(&mut dec),
        incoming::MARKET_DEPTH_L2 => decode_market_depth_l2(&mut dec),
        incoming::MKT_DEPTH_EXCHANGES => decode_mkt_depth_exchanges(&mut dec),
        // Historical Data
        incoming::HISTORICAL_DATA => decode_historical_data(&mut dec),
        incoming::HISTORICAL_DATA_UPDATE => decode_historical_data_update(&mut dec),
        incoming::HISTORICAL_DATA_END => decode_historical_data_end_msg(&mut dec),
        incoming::HEAD_TIMESTAMP => decode_head_timestamp(&mut dec),
        incoming::HISTORICAL_TICKS => decode_historical_ticks(&mut dec),
        incoming::HISTORICAL_TICKS_BID_ASK => decode_historical_ticks_bid_ask(&mut dec),
        incoming::HISTORICAL_TICKS_LAST => decode_historical_ticks_last(&mut dec),
        incoming::HISTORICAL_SCHEDULE => decode_historical_schedule(&mut dec),
        // Real-time Bars
        incoming::REAL_TIME_BARS => decode_real_time_bars(&mut dec),
        // Scanner
        incoming::SCANNER_DATA => decode_scanner_data(&mut dec),
        incoming::SCANNER_PARAMETERS => decode_scanner_parameters(&mut dec),
        // P&L
        incoming::PNL => decode_pnl(&mut dec),
        incoming::PNL_SINGLE => decode_pnl_single(&mut dec),
        // News
        incoming::NEWS_BULLETINS => decode_news_bulletins(&mut dec),
        incoming::NEWS_ARTICLE => decode_news_article(&mut dec),
        incoming::NEWS_PROVIDERS => decode_news_providers(&mut dec),
        incoming::HISTORICAL_NEWS => decode_historical_news(&mut dec),
        incoming::HISTORICAL_NEWS_END => decode_historical_news_end(&mut dec),
        // Fundamentals
        incoming::FUNDAMENTAL_DATA => decode_fundamental_data(&mut dec),
        // Market Rules & Infrastructure
        incoming::MARKET_RULE => decode_market_rule(&mut dec),
        incoming::SMART_COMPONENTS => decode_smart_components(&mut dec),
        incoming::FAMILY_CODES => decode_family_codes(&mut dec),
        incoming::SOFT_DOLLAR_TIERS => decode_soft_dollar_tiers(&mut dec),
        incoming::HISTOGRAM_DATA => decode_histogram_data(&mut dec),
        incoming::REROUTE_MKT_DATA_REQ => decode_reroute_mkt_data_req(&mut dec),
        incoming::REROUTE_MKT_DEPTH_REQ => decode_reroute_mkt_depth_req(&mut dec),
        // FA
        incoming::RECEIVE_FA => decode_receive_fa(&mut dec),
        incoming::REPLACE_FA_END => decode_replace_fa_end(&mut dec),
        // Display Groups
        incoming::DISPLAY_GROUP_LIST => decode_display_group_list(&mut dec),
        incoming::DISPLAY_GROUP_UPDATED => decode_display_group_updated(&mut dec),
        // Verification
        incoming::VERIFY_MESSAGE_API => decode_verify_message_api(&mut dec),
        incoming::VERIFY_COMPLETED => decode_verify_completed(&mut dec),
        incoming::VERIFY_AND_AUTH_MESSAGE_API => decode_verify_and_auth_message_api(&mut dec),
        incoming::VERIFY_AND_AUTH_COMPLETED => decode_verify_and_auth_completed(&mut dec),
        // WSH
        incoming::WSH_META_DATA => decode_wsh_meta_data(&mut dec),
        incoming::WSH_EVENT_DATA => decode_wsh_event_data(&mut dec),
        // User Info
        incoming::USER_INFO => decode_user_info(&mut dec),
        // Unknown
        _ => Ok(IBEvent::Unknown {
            msg_id,
            data: data.to_vec(),
        }),
    }
}

// ============================================================================
// Individual Message Decoders
// ============================================================================

/// Decode ERR_MSG (4).
///
/// C++ `EDecoder::processErrMsgMsg`.
fn decode_err_msg(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let version = dec.decode_i32()?;

    if version < 2 {
        // Old format: just a message string
        let msg = dec.decode_string()?;
        return Ok(IBEvent::Error {
            req_id: -1,
            error_time: 0,
            code: 0,
            message: msg,
            advanced_order_reject_json: String::new(),
        });
    }

    let id = dec.decode_i32()?;
    let error_code = dec.decode_i32()?;
    let error_msg = dec.decode_string()?;

    let advanced_order_reject_json =
        if dec.server_version() >= server_version::ADVANCED_ORDER_REJECT {
            dec.decode_string()?
        } else {
            String::new()
        };

    let error_time = if dec.server_version() >= server_version::ERROR_TIME {
        dec.decode_time()?
    } else {
        0
    };

    Ok(IBEvent::Error {
        req_id: id,
        error_time,
        code: error_code,
        message: error_msg,
        advanced_order_reject_json,
    })
}

/// Decode NEXT_VALID_ID (9).
///
/// C++ `EDecoder::processNextValidIdMsg`.
fn decode_next_valid_id(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let order_id = dec.decode_i64()?;
    Ok(IBEvent::NextValidId { order_id })
}

/// Decode MANAGED_ACCTS (15).
///
/// C++ `EDecoder::processManagedAcctsMsg`.
fn decode_managed_accts(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let accounts = dec.decode_string()?;
    Ok(IBEvent::ManagedAccounts { accounts })
}

/// Decode CURRENT_TIME (49).
///
/// C++ `EDecoder::processCurrentTimeMsg`.
fn decode_current_time(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let time = dec.decode_time()?;
    Ok(IBEvent::CurrentTime { time })
}

/// Decode CURRENT_TIME_IN_MILLIS (109).
fn decode_current_time_in_millis(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let time_in_millis = dec.decode_time()?;
    Ok(IBEvent::CurrentTimeInMillis { time_in_millis })
}

// ============================================================================
// Helper: decode order/perm ID with PERM_ID_AS_LONG gate
// ============================================================================

/// Decode an order ID or perm ID: i64 if sv >= PERM_ID_AS_LONG, else i32 cast.
fn decode_id_long(dec: &mut MessageDecoder) -> Result<i64> {
    if dec.server_version() >= server_version::PERM_ID_AS_LONG {
        dec.decode_i64()
    } else {
        Ok(dec.decode_i32()? as i64)
    }
}

/// Decode a TickType from an i32 field.
fn decode_tick_type(dec: &mut MessageDecoder) -> Result<TickType> {
    let raw = dec.decode_i32()?;
    TickType::try_from(raw).map_err(|_| IBApiError::Decoding(format!("unknown tick type {raw}")))
}

/// Decode a contract from order messages (OPEN_ORDER / COMPLETED_ORDER / EXECUTION_DATA).
fn decode_order_contract(dec: &mut MessageDecoder) -> Result<Contract> {
    let mut c = Contract::default();
    c.con_id = dec.decode_i32()? as i64;
    c.symbol = dec.decode_string()?;
    c.sec_type = dec.decode_enum_opt()?;
    c.last_trade_date_or_contract_month = dec.decode_string()?;
    c.strike = dec.decode_f64_max()?;
    c.right = dec.decode_enum_opt()?;
    c.multiplier = dec.decode_string()?;
    c.exchange = dec.decode_string()?;
    c.currency = dec.decode_string()?;
    c.local_symbol = dec.decode_string()?;
    c.trading_class = dec.decode_string()?;
    Ok(c)
}

/// Decode an order condition from the wire.
fn decode_order_condition(dec: &mut MessageDecoder, condition_type: i32) -> Result<OrderCondition> {
    let conj_str = dec.decode_string()?;
    let is_conjunction = conj_str == "a";

    match OrderConditionType::try_from(condition_type) {
        Ok(OrderConditionType::Price) => {
            let is_more = dec.decode_bool()?;
            let con_id = dec.decode_i32()?;
            let exchange = dec.decode_string()?;
            let price = dec.decode_f64()?;
            let trigger_method = TriggerMethod::try_from(dec.decode_i32()?).unwrap_or(TriggerMethod::Default);
            Ok(OrderCondition::Price {
                is_conjunction_connection: is_conjunction,
                is_more, con_id, exchange, price, trigger_method,
            })
        }
        Ok(OrderConditionType::Time) => {
            let is_more = dec.decode_bool()?;
            let time = dec.decode_string()?;
            Ok(OrderCondition::Time {
                is_conjunction_connection: is_conjunction, is_more, time,
            })
        }
        Ok(OrderConditionType::Margin) => {
            let is_more = dec.decode_bool()?;
            let percent = dec.decode_i32()?;
            Ok(OrderCondition::Margin {
                is_conjunction_connection: is_conjunction, is_more, percent,
            })
        }
        Ok(OrderConditionType::Execution) => {
            let exchange = dec.decode_string()?;
            let sec_type = dec.decode_string()?;
            let symbol = dec.decode_string()?;
            Ok(OrderCondition::Execution {
                is_conjunction_connection: is_conjunction, exchange, sec_type, symbol,
            })
        }
        Ok(OrderConditionType::Volume) => {
            let is_more = dec.decode_bool()?;
            let con_id = dec.decode_i32()?;
            let exchange = dec.decode_string()?;
            let volume = dec.decode_i32()?;
            Ok(OrderCondition::Volume {
                is_conjunction_connection: is_conjunction, is_more, con_id, exchange, volume,
            })
        }
        Ok(OrderConditionType::PercentChange) => {
            let is_more = dec.decode_bool()?;
            let con_id = dec.decode_i32()?;
            let exchange = dec.decode_string()?;
            let change_percent = dec.decode_f64_max()?;
            Ok(OrderCondition::PercentChange {
                is_conjunction_connection: is_conjunction, is_more, con_id, exchange, change_percent,
            })
        }
        _ => Err(IBApiError::Decoding(format!("unknown condition type {condition_type}"))),
    }
}

// ============================================================================
// Phase 4: Tick Data Decoders
// ============================================================================

/// Decode TICK_PRICE (1). C++ `processTickPriceMsg`.
fn decode_tick_price(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let sv = dec.server_version();
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let tick_type = decode_tick_type(dec)?;
    let price = dec.decode_f64()?;
    let size = dec.decode_decimal()?;
    let attr_mask = dec.decode_i32()?;

    let mut attrib = TickAttrib::default();
    if sv >= server_version::PAST_LIMIT {
        attrib.can_auto_execute = (attr_mask & 1) != 0;
        attrib.past_limit = (attr_mask & 2) != 0;
    }
    if sv >= server_version::PRE_OPEN_BID_ASK {
        attrib.pre_open = (attr_mask & 4) != 0;
    }

    Ok(IBEvent::TickPrice { req_id, tick_type, price, size, attrib })
}

/// Decode TICK_SIZE (2). C++ `processTickSizeMsg`.
fn decode_tick_size(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let tick_type = decode_tick_type(dec)?;
    let size = dec.decode_decimal()?;
    Ok(IBEvent::TickSize { req_id, tick_type, size })
}

/// Decode TICK_OPTION_COMPUTATION (21). C++ `processTickOptionComputationMsg`.
fn decode_tick_option_computation(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let sv = dec.server_version();
    let version = if sv < server_version::PRICE_BASED_VOLATILITY {
        dec.decode_i32()?
    } else {
        i32::MAX
    };
    let req_id = dec.decode_i32()?;
    let tick_type = decode_tick_type(dec)?;

    let tick_attrib = if sv >= server_version::PRICE_BASED_VOLATILITY {
        dec.decode_i32()?
    } else {
        0
    };

    let mut implied_vol = dec.decode_f64()?;
    if implied_vol == -1.0 { implied_vol = f64::MAX; }
    let mut delta = dec.decode_f64()?;
    if delta == -2.0 { delta = f64::MAX; }

    let is_model = matches!(tick_type, TickType::ModelOption | TickType::DelayedModelOptionComputation);
    let (opt_price, pv_dividend, gamma, vega, theta, und_price) = if version >= 6 || is_model {
        let mut op = dec.decode_f64()?;
        if op == -1.0 { op = f64::MAX; }
        let mut pvd = dec.decode_f64()?;
        if pvd == -1.0 { pvd = f64::MAX; }
        let mut g = dec.decode_f64()?;
        if g == -2.0 { g = f64::MAX; }
        let mut v = dec.decode_f64()?;
        if v == -2.0 { v = f64::MAX; }
        let mut t = dec.decode_f64()?;
        if t == -2.0 { t = f64::MAX; }
        let mut u = dec.decode_f64()?;
        if u == -1.0 { u = f64::MAX; }
        (Some(op), Some(pvd), Some(g), Some(v), Some(t), Some(u))
    } else {
        (None, None, None, None, None, None)
    };

    let to_opt = |v: f64| if v == f64::MAX { None } else { Some(v) };

    Ok(IBEvent::TickOptionComputation {
        req_id, tick_type, tick_attrib,
        implied_vol: to_opt(implied_vol),
        delta: to_opt(delta),
        opt_price: opt_price.and_then(to_opt),
        pv_dividend: pv_dividend.and_then(to_opt),
        gamma: gamma.and_then(to_opt),
        vega: vega.and_then(to_opt),
        theta: theta.and_then(to_opt),
        und_price: und_price.and_then(to_opt),
    })
}

/// Decode TICK_GENERIC (45). C++ `processTickGenericMsg`.
fn decode_tick_generic(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let tick_type = decode_tick_type(dec)?;
    let value = dec.decode_f64()?;
    Ok(IBEvent::TickGeneric { req_id, tick_type, value })
}

/// Decode TICK_STRING (46). C++ `processTickStringMsg`.
fn decode_tick_string(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let tick_type = decode_tick_type(dec)?;
    let value = dec.decode_string()?;
    Ok(IBEvent::TickString { req_id, tick_type, value })
}

/// Decode TICK_EFP (47). C++ `processTickEfpMsg`.
fn decode_tick_efp(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let tick_type = decode_tick_type(dec)?;
    let basis_points = dec.decode_f64()?;
    let formatted_basis_points = dec.decode_string()?;
    let total_dividends = dec.decode_f64()?;
    let hold_days = dec.decode_i32()?;
    let future_last_trade_date = dec.decode_string()?;
    let dividend_impact = dec.decode_f64()?;
    let dividends_to_last_trade_date = dec.decode_f64()?;
    Ok(IBEvent::TickEfp {
        req_id, tick_type, basis_points, formatted_basis_points,
        total_dividends, hold_days, future_last_trade_date,
        dividend_impact, dividends_to_last_trade_date,
    })
}

/// Decode TICK_SNAPSHOT_END (57).
fn decode_tick_snapshot_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    Ok(IBEvent::TickSnapshotEnd { req_id })
}

/// Decode TICK_REQ_PARAMS (81). C++ `processTickReqParamsMsg`.
fn decode_tick_req_params(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let min_tick = dec.decode_f64()?;
    let bbo_exchange = dec.decode_string()?;
    let snapshot_permissions = dec.decode_i32()?;
    Ok(IBEvent::TickReqParams { req_id, min_tick, bbo_exchange, snapshot_permissions })
}

/// Decode TICK_NEWS (84). C++ `processTickNewsMsg`.
fn decode_tick_news(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let timestamp = dec.decode_time()?;
    let provider_code = dec.decode_string()?;
    let article_id = dec.decode_string()?;
    let headline = dec.decode_string()?;
    let extra_data = dec.decode_string()?;
    Ok(IBEvent::TickNews { req_id, timestamp, provider_code, article_id, headline, extra_data })
}

/// Decode MARKET_DATA_TYPE (58).
fn decode_market_data_type(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let market_data_type = dec.decode_i32()?;
    Ok(IBEvent::MarketDataType { req_id, market_data_type })
}

/// Decode TICK_BY_TICK (99). C++ `processTickByTickDataMsg`.
fn decode_tick_by_tick(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let tick_type = dec.decode_i32()?;
    let time = dec.decode_time()?;

    match tick_type {
        1 | 2 => {
            let price = dec.decode_f64()?;
            let size = dec.decode_decimal()?;
            let attr_mask = dec.decode_i32()?;
            let exchange = dec.decode_string()?;
            let special_conditions = dec.decode_string()?;
            Ok(IBEvent::TickByTickAllLast {
                req_id, tick_type, time, price, size,
                attrib: TickAttribLast {
                    past_limit: (attr_mask & 1) != 0,
                    unreported: (attr_mask & 2) != 0,
                },
                exchange, special_conditions,
            })
        }
        3 => {
            let bid_price = dec.decode_f64()?;
            let ask_price = dec.decode_f64()?;
            let bid_size = dec.decode_decimal()?;
            let ask_size = dec.decode_decimal()?;
            let attr_mask = dec.decode_i32()?;
            Ok(IBEvent::TickByTickBidAsk {
                req_id, time, bid_price, ask_price, bid_size, ask_size,
                attrib: TickAttribBidAsk {
                    bid_past_low: (attr_mask & 1) != 0,
                    ask_past_high: (attr_mask & 2) != 0,
                },
            })
        }
        4 => {
            let mid_point = dec.decode_f64()?;
            Ok(IBEvent::TickByTickMidPoint { req_id, time, mid_point })
        }
        _ => Ok(IBEvent::Unknown { msg_id: incoming::TICK_BY_TICK, data: Vec::new() }),
    }
}

// ============================================================================
// Phase 4: Order Decoders
// ============================================================================

/// Decode ORDER_STATUS (3). C++ `processOrderStatusMsg`.
fn decode_order_status(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let sv = dec.server_version();
    if sv < server_version::MARKET_CAP_PRICE {
        dec.skip_field()?; // version
    }
    let order_id = decode_id_long(dec)?;
    let status = dec.decode_string()?;
    let filled = dec.decode_decimal()?;
    let remaining = dec.decode_decimal()?;
    let avg_fill_price = dec.decode_f64()?;
    let perm_id = decode_id_long(dec)?;
    let parent_id = dec.decode_i32()?;
    let last_fill_price = dec.decode_f64()?;
    let client_id = dec.decode_i32()?;
    let why_held = dec.decode_string()?;
    let mkt_cap_price = if sv >= server_version::MARKET_CAP_PRICE {
        dec.decode_f64()?
    } else {
        0.0
    };
    Ok(IBEvent::OrderStatus {
        order_id, status, filled, remaining, avg_fill_price,
        perm_id, parent_id, last_fill_price, client_id, why_held, mkt_cap_price,
    })
}

/// Decode ORDER_BOUND (100). C++ `processOrderBoundMsg`.
fn decode_order_bound(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let perm_id = dec.decode_i64()?;
    let client_id = dec.decode_i32()?;
    let order_id = dec.decode_i32()?;
    Ok(IBEvent::OrderBound { perm_id, client_id, order_id })
}

/// Decode OPEN_ORDER (5). C++ `processOpenOrderMsg`.
fn decode_open_order(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let sv = dec.server_version();
    let version = if sv < server_version::ORDER_CONTAINER { dec.decode_i32()? } else { sv };

    // Order ID
    let order_id = decode_id_long(dec)?;

    // Contract
    let mut contract = decode_order_contract(dec)?;

    // Order fields
    let mut order = Order::default();
    order.order_id = order_id;
    order.action = dec.decode_enum_opt()?;
    order.total_quantity = dec.decode_decimal_max()?;
    order.order_type = dec.decode_enum_opt()?;
    order.lmt_price = dec.decode_f64_max()?;
    order.aux_price = dec.decode_f64_max()?;
    order.tif = dec.decode_enum_opt()?;
    order.oca_group = dec.decode_string()?;
    order.account = dec.decode_string()?;
    order.open_close = dec.decode_string()?;
    order.origin = Origin::try_from(dec.decode_i32()?).unwrap_or(Origin::Customer);
    order.order_ref = dec.decode_string()?;
    order.client_id = dec.decode_i32()? as i64;
    order.perm_id = decode_id_long(dec)?;
    order.outside_rth = dec.decode_bool()?;
    order.hidden = dec.decode_bool()?;
    order.discretionary_amt = dec.decode_f64()?;
    order.good_after_time = dec.decode_string()?;
    dec.skip_field()?; // sharesAllocation (deprecated)

    // FA params
    order.fa_group = dec.decode_string()?;
    order.fa_method = dec.decode_string()?;
    order.fa_percentage = dec.decode_string()?;
    if sv < server_version::FA_PROFILE_DESUPPORT {
        dec.skip_field()?; // deprecated faProfile
    }

    if sv >= server_version::MODELS_SUPPORT {
        order.model_code = dec.decode_string()?;
    }

    order.good_till_date = dec.decode_string()?;
    order.rule_80a = dec.decode_string()?;
    order.percent_offset = dec.decode_f64_max()?;
    order.settling_firm = dec.decode_string()?;

    // Short sale params
    order.short_sale_slot = dec.decode_i32()?;
    order.designated_location = dec.decode_string()?;
    if sv == server_version::SSHORTX_OLD {
        dec.skip_field()?;
    } else if version >= 23 {
        order.exempt_code = dec.decode_i32()?;
    }

    order.auction_strategy = AuctionStrategy::try_from(dec.decode_i32()?).unwrap_or(AuctionStrategy::Unset);

    // Box order params
    order.starting_price = dec.decode_f64_max()?;
    order.stock_ref_price = dec.decode_f64_max()?;
    order.delta = dec.decode_f64_max()?;

    // Peg to stk / vol order params
    order.stock_range_lower = dec.decode_f64_max()?;
    order.stock_range_upper = dec.decode_f64_max()?;

    order.display_size = dec.decode_i32()?;
    order.block_order = dec.decode_bool()?;
    order.sweep_to_fill = dec.decode_bool()?;
    order.all_or_none = dec.decode_bool()?;
    order.min_qty = dec.decode_i32_max()?;
    order.oca_type = dec.decode_i32()?;

    // Skip eTradeOnly, firmQuoteOnly, nbboPriceCap
    dec.skip_fields(3)?;

    order.parent_id = dec.decode_i32()? as i64;
    order.trigger_method = dec.decode_i32()?;

    // Vol order params (decodeOpenOrderAttribs = true)
    order.volatility = dec.decode_f64_max()?;
    order.volatility_type = dec.decode_i32_max()?;
    order.delta_neutral_order_type = dec.decode_string()?;
    order.delta_neutral_aux_price = dec.decode_f64_max()?;
    if version >= 27 && !order.delta_neutral_order_type.is_empty() {
        order.delta_neutral_con_id = dec.decode_i64()?;
        order.delta_neutral_settling_firm = dec.decode_string()?;
        order.delta_neutral_clearing_account = dec.decode_string()?;
        order.delta_neutral_clearing_intent = dec.decode_string()?;
    }
    if version >= 31 && !order.delta_neutral_order_type.is_empty() {
        order.delta_neutral_open_close = dec.decode_string()?;
        order.delta_neutral_short_sale = dec.decode_bool()?;
        order.delta_neutral_short_sale_slot = dec.decode_i32()?;
        order.delta_neutral_designated_location = dec.decode_string()?;
    }
    order.continuous_update = dec.decode_bool()?;
    order.reference_price_type = dec.decode_i32_max()?;

    // Trail params
    order.trail_stop_price = dec.decode_f64_max()?;
    if version >= 30 {
        order.trailing_percent = dec.decode_f64_max()?;
    }

    // Basis points
    order.basis_points = dec.decode_f64_max()?;
    order.basis_points_type = dec.decode_i32_max()?;

    // Combo legs
    contract.combo_legs_descrip = dec.decode_string()?;
    if version >= 29 {
        let count = dec.decode_i32()?;
        if count > 0 {
            let mut legs = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let mut leg = ComboLeg::default();
                leg.con_id = dec.decode_i32()? as i64;
                leg.ratio = dec.decode_i32()? as i64;
                leg.action = dec.decode_enum_opt()?;
                leg.exchange = dec.decode_string()?;
                leg.open_close = LegOpenClose::try_from(dec.decode_i32()?).unwrap_or(LegOpenClose::Same);
                if version >= 26 {
                    leg.short_sale_slot = dec.decode_i32()?;
                    leg.designated_location = dec.decode_string()?;
                    leg.exempt_code = dec.decode_i32()?;
                }
                legs.push(leg);
            }
            contract.combo_legs = Some(legs);
        }
        let ocl_count = dec.decode_i32()?;
        if ocl_count > 0 {
            let mut ocls = Vec::with_capacity(ocl_count as usize);
            for _ in 0..ocl_count {
                ocls.push(OrderComboLeg { price: dec.decode_f64_max()? });
            }
            order.order_combo_legs = Some(ocls);
        }
    }

    // Smart combo routing params
    if version >= 26 {
        let count = dec.decode_i32()?;
        if count > 0 {
            let mut params = Vec::with_capacity(count as usize);
            for _ in 0..count {
                params.push(TagValue { tag: dec.decode_string()?, value: dec.decode_string()? });
            }
            order.smart_combo_routing_params = Some(params);
        }
    }

    // Scale order params
    if version >= 20 {
        order.scale_init_level_size = dec.decode_i32_max()?;
        order.scale_subs_level_size = dec.decode_i32_max()?;
    } else {
        dec.skip_field()?; // notSuppScaleNumComponents
        order.scale_init_level_size = dec.decode_i32_max()?;
    }
    order.scale_price_increment = dec.decode_f64_max()?;
    if version >= 28 {
        if let Some(inc) = order.scale_price_increment {
            if inc > 0.0 {
                order.scale_price_adjust_value = dec.decode_f64_max()?;
                order.scale_price_adjust_interval = dec.decode_i32_max()?;
                order.scale_profit_offset = dec.decode_f64_max()?;
                order.scale_auto_reset = dec.decode_bool()?;
                order.scale_init_position = dec.decode_i32_max()?;
                order.scale_init_fill_qty = dec.decode_i32_max()?;
                order.scale_random_percent = dec.decode_bool()?;
            }
        }
    }

    // Hedge params
    if version >= 24 {
        order.hedge_type = dec.decode_string()?;
        if !order.hedge_type.is_empty() {
            order.hedge_param = dec.decode_string()?;
        }
    }

    if version >= 25 {
        order.opt_out_smart_routing = dec.decode_bool()?;
    }

    // Clearing params
    order.clearing_account = dec.decode_string()?;
    order.clearing_intent = dec.decode_string()?;

    if version >= 22 {
        order.not_held = dec.decode_bool()?;
    }

    // Delta neutral contract on Contract
    if version >= 20 {
        let has_dn = dec.decode_bool()?;
        if has_dn {
            contract.delta_neutral_contract = Some(DeltaNeutralContract {
                con_id: dec.decode_i32()? as i64,
                delta: dec.decode_f64()?,
                price: dec.decode_f64()?,
            });
        }
    }

    // Algo params
    if version >= 21 {
        order.algo_strategy = dec.decode_string()?;
        if !order.algo_strategy.is_empty() {
            let count = dec.decode_i32()?;
            if count > 0 {
                let mut params = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    params.push(TagValue { tag: dec.decode_string()?, value: dec.decode_string()? });
                }
                order.algo_params = Some(params);
            }
        }
    }

    if version >= 33 {
        order.solicited = dec.decode_bool()?;
    }

    // WhatIf info + commission
    let mut order_state = OrderState::default();
    order.what_if = dec.decode_bool()?;
    order_state.status = dec.decode_string()?;

    if sv >= server_version::WHAT_IF_EXT_FIELDS {
        order_state.init_margin_before = dec.decode_string()?;
        order_state.maint_margin_before = dec.decode_string()?;
        order_state.equity_with_loan_before = dec.decode_string()?;
        order_state.init_margin_change = dec.decode_string()?;
        order_state.maint_margin_change = dec.decode_string()?;
        order_state.equity_with_loan_change = dec.decode_string()?;
    }

    order_state.init_margin_after = dec.decode_string()?;
    order_state.maint_margin_after = dec.decode_string()?;
    order_state.equity_with_loan_after = dec.decode_string()?;
    order_state.commission_and_fees = dec.decode_f64_max()?;
    order_state.min_commission_and_fees = dec.decode_f64_max()?;
    order_state.max_commission_and_fees = dec.decode_f64_max()?;
    order_state.commission_and_fees_currency = dec.decode_string()?;

    if sv >= server_version::FULL_ORDER_PREVIEW_FIELDS {
        order_state.margin_currency = dec.decode_string()?;
        order_state.init_margin_before_outside_rth = dec.decode_f64_max()?;
        order_state.maint_margin_before_outside_rth = dec.decode_f64_max()?;
        order_state.equity_with_loan_before_outside_rth = dec.decode_f64_max()?;
        order_state.init_margin_change_outside_rth = dec.decode_f64_max()?;
        order_state.maint_margin_change_outside_rth = dec.decode_f64_max()?;
        order_state.equity_with_loan_change_outside_rth = dec.decode_f64_max()?;
        order_state.init_margin_after_outside_rth = dec.decode_f64_max()?;
        order_state.maint_margin_after_outside_rth = dec.decode_f64_max()?;
        order_state.equity_with_loan_after_outside_rth = dec.decode_f64_max()?;
        order_state.suggested_size = dec.decode_decimal_max()?;
        order_state.reject_reason = dec.decode_string()?;
        let alloc_count = dec.decode_i32()?;
        if alloc_count > 0 {
            let mut allocs = Vec::with_capacity(alloc_count as usize);
            for _ in 0..alloc_count {
                allocs.push(OrderAllocation {
                    account: dec.decode_string()?,
                    position: dec.decode_decimal_max()?,
                    position_desired: dec.decode_decimal_max()?,
                    position_after: dec.decode_decimal_max()?,
                    desired_alloc_qty: dec.decode_decimal_max()?,
                    allowed_alloc_qty: dec.decode_decimal_max()?,
                    is_monetary: dec.decode_bool()?,
                });
            }
            order_state.order_allocations = Some(allocs);
        }
    }

    order_state.warning_text = dec.decode_string()?;

    if version >= 34 {
        order.randomize_size = dec.decode_bool()?;
        order.randomize_price = dec.decode_bool()?;
    }

    // Peg benchmark params
    if sv >= server_version::PEGGED_TO_BENCHMARK
        && order.order_type == Some(OrderType::PeggedToBenchmark)
    {
        order.reference_contract_id = dec.decode_i32_max()?;
        order.is_pegged_change_amount_decrease = dec.decode_bool()?;
        order.pegged_change_amount = dec.decode_f64_max()?;
        order.reference_change_amount = dec.decode_f64_max()?;
        order.reference_exchange_id = dec.decode_string()?;
    }

    // Conditions
    if sv >= server_version::PEGGED_TO_BENCHMARK {
        let cond_size = dec.decode_i32()?;
        if cond_size > 0 {
            let mut conds = Vec::with_capacity(cond_size as usize);
            for _ in 0..cond_size {
                let ct = dec.decode_i32()?;
                conds.push(decode_order_condition(dec, ct)?);
            }
            order.conditions = conds;
            order.conditions_ignore_rth = dec.decode_bool()?;
            order.conditions_cancel_order = dec.decode_bool()?;
        }
    }

    // Adjusted order params
    if sv >= server_version::PEGGED_TO_BENCHMARK {
        order.adjusted_order_type = dec.decode_string()?;
        order.trigger_price = dec.decode_f64_max()?;
        order.trail_stop_price = dec.decode_f64_max()?;
        order.lmt_price_offset = dec.decode_f64_max()?;
        order.adjusted_stop_price = dec.decode_f64_max()?;
        order.adjusted_stop_limit_price = dec.decode_f64_max()?;
        order.adjusted_trailing_amount = dec.decode_f64_max()?;
        order.adjustable_trailing_unit = dec.decode_i32_max()?;
    }

    if sv >= server_version::SOFT_DOLLAR_TIER {
        order.soft_dollar_tier = SoftDollarTier {
            name: dec.decode_string()?,
            val: dec.decode_string()?,
            display_name: dec.decode_string()?,
        };
    }
    if sv >= server_version::CASH_QTY { order.cash_qty = dec.decode_f64_max()?; }
    if sv >= server_version::AUTO_PRICE_FOR_HEDGE { order.dont_use_auto_price_for_hedge = dec.decode_bool()?; }
    if sv >= server_version::ORDER_CONTAINER { order.is_oms_container = dec.decode_bool()?; }
    if sv >= server_version::D_PEG_ORDERS { order.discretionary_up_to_limit_price = dec.decode_bool()?; }
    if sv >= server_version::PRICE_MGMT_ALGO {
        order.use_price_mgmt_algo = UsePriceMgmtAlgo::try_from(dec.decode_i32()?).unwrap_or(UsePriceMgmtAlgo::Default);
    }
    if sv >= server_version::DURATION { order.duration = dec.decode_i32_max()?; }
    if sv >= server_version::POST_TO_ATS { order.post_to_ats = dec.decode_i32_max()?; }
    if sv >= server_version::AUTO_CANCEL_PARENT {
        order.auto_cancel_date = dec.decode_string()?;
        order.filled_quantity = dec.decode_decimal_max()?;
        order.ref_futures_con_id = dec.decode_i32_max()?;
        order.auto_cancel_parent = dec.decode_bool()?;
        order.shareholder = dec.decode_string()?;
        order.imbalance_only = dec.decode_bool()?;
        order.route_marketable_to_bbo = dec.decode_bool()?;
        order.parent_perm_id = dec.decode_i64_max()?;
    }
    if sv >= server_version::PEGBEST_PEGMID_OFFSETS {
        order.min_trade_qty = dec.decode_i32_max()?;
        order.min_compete_size = dec.decode_i32_max()?;
        order.compete_against_best_offset = dec.decode_f64_max()?;
        order.mid_offset_at_whole = dec.decode_f64_max()?;
        order.mid_offset_at_half = dec.decode_f64_max()?;
    }
    if sv >= server_version::CUSTOMER_ACCOUNT { order.customer_account = dec.decode_string()?; }
    if sv >= server_version::PROFESSIONAL_CUSTOMER { order.professional_customer = dec.decode_bool()?; }
    if sv >= server_version::BOND_ACCRUED_INTEREST { order.bond_accrued_interest = dec.decode_string()?; }
    if sv >= server_version::INCLUDE_OVERNIGHT { order.include_overnight = dec.decode_bool()?; }
    if sv >= server_version::CME_TAGGING_FIELDS_IN_OPEN_ORDER {
        order.ext_operator = dec.decode_string()?;
        order.manual_order_indicator = dec.decode_i32_max()?;
    }
    if sv >= server_version::SUBMITTER { order.submitter = dec.decode_string()?; }
    if sv >= server_version::IMBALANCE_ONLY && version >= server_version::IMBALANCE_ONLY {
        order.imbalance_only = dec.decode_bool()?;
    }

    Ok(IBEvent::OpenOrder {
        order_id,
        contract: Box::new(contract),
        order: Box::new(order),
        order_state: Box::new(order_state),
    })
}

/// Decode COMPLETED_ORDER (101). C++ `processCompletedOrderMsg`.
fn decode_completed_order(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let sv = dec.server_version();
    let version = i32::MAX; // COMPLETED_ORDER always uses UNSET_INTEGER

    let mut contract = decode_order_contract(dec)?;
    let mut order = Order::default();

    order.action = dec.decode_enum_opt()?;
    order.total_quantity = dec.decode_decimal_max()?;
    order.order_type = dec.decode_enum_opt()?;
    order.lmt_price = dec.decode_f64_max()?;
    order.aux_price = dec.decode_f64_max()?;
    order.tif = dec.decode_enum_opt()?;
    order.oca_group = dec.decode_string()?;
    order.account = dec.decode_string()?;
    order.open_close = dec.decode_string()?;
    order.origin = Origin::try_from(dec.decode_i32()?).unwrap_or(Origin::Customer);
    order.order_ref = dec.decode_string()?;
    order.perm_id = decode_id_long(dec)?;
    order.outside_rth = dec.decode_bool()?;
    order.hidden = dec.decode_bool()?;
    order.discretionary_amt = dec.decode_f64()?;
    order.good_after_time = dec.decode_string()?;

    // FA params
    order.fa_group = dec.decode_string()?;
    order.fa_method = dec.decode_string()?;
    order.fa_percentage = dec.decode_string()?;
    if sv < server_version::FA_PROFILE_DESUPPORT {
        dec.skip_field()?;
    }

    if sv >= server_version::MODELS_SUPPORT {
        order.model_code = dec.decode_string()?;
    }

    order.good_till_date = dec.decode_string()?;
    order.rule_80a = dec.decode_string()?;
    order.percent_offset = dec.decode_f64_max()?;
    order.settling_firm = dec.decode_string()?;

    order.short_sale_slot = dec.decode_i32()?;
    order.designated_location = dec.decode_string()?;
    order.exempt_code = dec.decode_i32()?;

    // Box order params
    order.starting_price = dec.decode_f64_max()?;
    order.stock_ref_price = dec.decode_f64_max()?;
    order.delta = dec.decode_f64_max()?;

    order.stock_range_lower = dec.decode_f64_max()?;
    order.stock_range_upper = dec.decode_f64_max()?;

    order.display_size = dec.decode_i32()?;
    order.sweep_to_fill = dec.decode_bool()?;
    order.all_or_none = dec.decode_bool()?;
    order.min_qty = dec.decode_i32_max()?;
    order.oca_type = dec.decode_i32()?;
    order.trigger_method = dec.decode_i32()?;

    // Vol order params (decodeOpenOrderAttribs = false)
    order.volatility = dec.decode_f64_max()?;
    order.volatility_type = dec.decode_i32_max()?;
    order.delta_neutral_order_type = dec.decode_string()?;
    order.delta_neutral_aux_price = dec.decode_f64_max()?;
    if version >= 27 && !order.delta_neutral_order_type.is_empty() {
        order.delta_neutral_con_id = dec.decode_i64()?;
        // decodeOpenOrderAttribs = false: skip settling/clearing
    }
    if version >= 31 && !order.delta_neutral_order_type.is_empty() {
        order.delta_neutral_open_close = dec.decode_string()?;
        order.delta_neutral_short_sale = dec.decode_bool()?;
        order.delta_neutral_short_sale_slot = dec.decode_i32()?;
        order.delta_neutral_designated_location = dec.decode_string()?;
    }
    order.continuous_update = dec.decode_bool()?;
    order.reference_price_type = dec.decode_i32_max()?;

    order.trail_stop_price = dec.decode_f64_max()?;
    order.trailing_percent = dec.decode_f64_max()?;

    // Combo legs
    contract.combo_legs_descrip = dec.decode_string()?;
    let count = dec.decode_i32()?;
    if count > 0 {
        let mut legs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let mut leg = ComboLeg::default();
            leg.con_id = dec.decode_i32()? as i64;
            leg.ratio = dec.decode_i32()? as i64;
            leg.action = dec.decode_enum_opt()?;
            leg.exchange = dec.decode_string()?;
            leg.open_close = LegOpenClose::try_from(dec.decode_i32()?).unwrap_or(LegOpenClose::Same);
            leg.short_sale_slot = dec.decode_i32()?;
            leg.designated_location = dec.decode_string()?;
            leg.exempt_code = dec.decode_i32()?;
            legs.push(leg);
        }
        contract.combo_legs = Some(legs);
    }
    let ocl_count = dec.decode_i32()?;
    if ocl_count > 0 {
        let mut ocls = Vec::with_capacity(ocl_count as usize);
        for _ in 0..ocl_count {
            ocls.push(OrderComboLeg { price: dec.decode_f64_max()? });
        }
        order.order_combo_legs = Some(ocls);
    }

    // Smart combo routing params
    let sc_count = dec.decode_i32()?;
    if sc_count > 0 {
        let mut params = Vec::with_capacity(sc_count as usize);
        for _ in 0..sc_count {
            params.push(TagValue { tag: dec.decode_string()?, value: dec.decode_string()? });
        }
        order.smart_combo_routing_params = Some(params);
    }

    // Scale order params
    order.scale_init_level_size = dec.decode_i32_max()?;
    order.scale_subs_level_size = dec.decode_i32_max()?;
    order.scale_price_increment = dec.decode_f64_max()?;
    if let Some(inc) = order.scale_price_increment {
        if inc > 0.0 {
            order.scale_price_adjust_value = dec.decode_f64_max()?;
            order.scale_price_adjust_interval = dec.decode_i32_max()?;
            order.scale_profit_offset = dec.decode_f64_max()?;
            order.scale_auto_reset = dec.decode_bool()?;
            order.scale_init_position = dec.decode_i32_max()?;
            order.scale_init_fill_qty = dec.decode_i32_max()?;
            order.scale_random_percent = dec.decode_bool()?;
        }
    }

    // Hedge params
    order.hedge_type = dec.decode_string()?;
    if !order.hedge_type.is_empty() {
        order.hedge_param = dec.decode_string()?;
    }

    order.clearing_account = dec.decode_string()?;
    order.clearing_intent = dec.decode_string()?;
    order.not_held = dec.decode_bool()?;

    // Delta neutral contract
    let has_dn = dec.decode_bool()?;
    if has_dn {
        contract.delta_neutral_contract = Some(DeltaNeutralContract {
            con_id: dec.decode_i32()? as i64,
            delta: dec.decode_f64()?,
            price: dec.decode_f64()?,
        });
    }

    // Algo params
    order.algo_strategy = dec.decode_string()?;
    if !order.algo_strategy.is_empty() {
        let cnt = dec.decode_i32()?;
        if cnt > 0 {
            let mut params = Vec::with_capacity(cnt as usize);
            for _ in 0..cnt {
                params.push(TagValue { tag: dec.decode_string()?, value: dec.decode_string()? });
            }
            order.algo_params = Some(params);
        }
    }

    order.solicited = dec.decode_bool()?;

    let mut order_state = OrderState::default();
    order_state.status = dec.decode_string()?;

    // Vol randomize
    order.randomize_size = dec.decode_bool()?;
    order.randomize_price = dec.decode_bool()?;

    // Peg bench
    if sv >= server_version::PEGGED_TO_BENCHMARK
        && order.order_type == Some(OrderType::PeggedToBenchmark)
    {
        order.reference_contract_id = dec.decode_i32_max()?;
        order.is_pegged_change_amount_decrease = dec.decode_bool()?;
        order.pegged_change_amount = dec.decode_f64_max()?;
        order.reference_change_amount = dec.decode_f64_max()?;
        order.reference_exchange_id = dec.decode_string()?;
    }

    // Conditions
    if sv >= server_version::PEGGED_TO_BENCHMARK {
        let cond_size = dec.decode_i32()?;
        if cond_size > 0 {
            let mut conds = Vec::with_capacity(cond_size as usize);
            for _ in 0..cond_size {
                let ct = dec.decode_i32()?;
                conds.push(decode_order_condition(dec, ct)?);
            }
            order.conditions = conds;
            order.conditions_ignore_rth = dec.decode_bool()?;
            order.conditions_cancel_order = dec.decode_bool()?;
        }
    }

    // Stop price and lmt price offset (COMPLETED_ORDER specific)
    order.trail_stop_price = dec.decode_f64_max()?;
    order.lmt_price_offset = dec.decode_f64_max()?;

    if sv >= server_version::CASH_QTY { order.cash_qty = dec.decode_f64_max()?; }
    if sv >= server_version::AUTO_PRICE_FOR_HEDGE { order.dont_use_auto_price_for_hedge = dec.decode_bool()?; }
    if sv >= server_version::ORDER_CONTAINER { order.is_oms_container = dec.decode_bool()?; }

    order.auto_cancel_date = dec.decode_string()?;
    order.filled_quantity = dec.decode_decimal_max()?;
    order.ref_futures_con_id = dec.decode_i32_max()?;
    order.auto_cancel_parent = dec.decode_bool()?;
    order.shareholder = dec.decode_string()?;
    order.imbalance_only = dec.decode_bool()?;
    order.route_marketable_to_bbo = dec.decode_bool()?;
    order.parent_perm_id = dec.decode_i64_max()?;

    order_state.completed_time = dec.decode_string()?;
    order_state.completed_status = dec.decode_string()?;

    if sv >= server_version::PEGBEST_PEGMID_OFFSETS {
        order.min_trade_qty = dec.decode_i32_max()?;
        order.min_compete_size = dec.decode_i32_max()?;
        order.compete_against_best_offset = dec.decode_f64_max()?;
        order.mid_offset_at_whole = dec.decode_f64_max()?;
        order.mid_offset_at_half = dec.decode_f64_max()?;
    }
    if sv >= server_version::CUSTOMER_ACCOUNT { order.customer_account = dec.decode_string()?; }
    if sv >= server_version::PROFESSIONAL_CUSTOMER { order.professional_customer = dec.decode_bool()?; }
    if sv >= server_version::SUBMITTER { order.submitter = dec.decode_string()?; }

    Ok(IBEvent::CompletedOrder {
        contract: Box::new(contract),
        order: Box::new(order),
        order_state: Box::new(order_state),
    })
}

// ============================================================================
// Phase 4: Execution Decoders
// ============================================================================

/// Decode EXECUTION_DATA (11). C++ `processExecutionDetailsMsg`.
fn decode_execution_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let sv = dec.server_version();
    let version = if sv < server_version::LAST_LIQUIDITY { dec.decode_i32()? } else { sv };
    let req_id = if version >= 7 { dec.decode_i32()? } else { -1 };

    let order_id = dec.decode_i32()?;

    let mut contract = Contract::default();
    if version >= 5 { contract.con_id = dec.decode_i32()? as i64; }
    contract.symbol = dec.decode_string()?;
    contract.sec_type = dec.decode_enum_opt()?;
    contract.last_trade_date_or_contract_month = dec.decode_string()?;
    contract.strike = dec.decode_f64_max()?;
    contract.right = dec.decode_enum_opt()?;
    if version >= 9 { contract.multiplier = dec.decode_string()?; }
    contract.exchange = dec.decode_string()?;
    contract.currency = dec.decode_string()?;
    contract.local_symbol = dec.decode_string()?;
    if version >= 10 { contract.trading_class = dec.decode_string()?; }

    let mut exec = Execution::default();
    exec.order_id = order_id as i64;
    exec.exec_id = dec.decode_string()?;
    exec.time = dec.decode_string()?;
    exec.acct_number = dec.decode_string()?;
    exec.exchange = dec.decode_string()?;
    exec.side = dec.decode_string()?;
    exec.shares = dec.decode_decimal_max()?;
    exec.price = dec.decode_f64()?;
    if version >= 2 { exec.perm_id = dec.decode_i32()? as i64; }
    if version >= 3 { exec.client_id = dec.decode_i32()? as i64; }
    if version >= 4 { exec.liquidation = dec.decode_i32()?; }
    if version >= 6 {
        exec.cum_qty = dec.decode_decimal_max()?;
        exec.avg_price = dec.decode_f64()?;
    }
    if version >= 8 { exec.order_ref = dec.decode_string()?; }
    if version >= 9 {
        exec.ev_rule = dec.decode_string()?;
        exec.ev_multiplier = dec.decode_f64()?;
    }
    if sv >= server_version::MODELS_SUPPORT { exec.model_code = dec.decode_string()?; }
    if sv >= server_version::LAST_LIQUIDITY { exec.last_liquidity = dec.decode_i32()?; }
    if sv >= server_version::PENDING_PRICE_REVISION { exec.pending_price_revision = dec.decode_bool()?; }
    if sv >= server_version::SUBMITTER { exec.submitter = dec.decode_string()?; }

    Ok(IBEvent::ExecDetails {
        req_id,
        contract: Box::new(contract),
        execution: Box::new(exec),
    })
}

/// Decode EXECUTION_DATA_END (55).
fn decode_execution_data_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    Ok(IBEvent::ExecDetailsEnd { req_id })
}

/// Decode COMMISSION_AND_FEES_REPORT (59). C++ `processCommissionAndFeesReportMsg`.
fn decode_commission_report(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    Ok(IBEvent::CommissionReport {
        report: CommissionAndFeesReport {
            exec_id: dec.decode_string()?,
            commission_and_fees: dec.decode_f64()?,
            currency: dec.decode_string()?,
            realized_pnl: dec.decode_f64()?,
            r#yield: dec.decode_f64()?,
            yield_redemption_date: dec.decode_i32()?,
        },
    })
}

// ============================================================================
// Phase 4: Account Decoders
// ============================================================================

/// Decode ACCT_VALUE (6). C++ `processAcctValueMsg`.
fn decode_acct_value(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let key = dec.decode_string()?;
    let value = dec.decode_string()?;
    let currency = dec.decode_string()?;
    let account_name = dec.decode_string()?;
    Ok(IBEvent::UpdateAccountValue { key, value, currency, account_name })
}

/// Decode PORTFOLIO_VALUE (7). C++ `processPortfolioValueMsg`.
fn decode_portfolio_value(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let version = dec.decode_i32()?;
    let mut contract = Contract::default();
    if version >= 6 { contract.con_id = dec.decode_i32()? as i64; }
    contract.symbol = dec.decode_string()?;
    contract.sec_type = dec.decode_enum_opt()?;
    contract.last_trade_date_or_contract_month = dec.decode_string()?;
    contract.strike = dec.decode_f64_max()?;
    contract.right = dec.decode_enum_opt()?;
    if version >= 7 {
        contract.multiplier = dec.decode_string()?;
        contract.primary_exchange = dec.decode_string()?;
    }
    contract.currency = dec.decode_string()?;
    if version >= 2 { contract.local_symbol = dec.decode_string()?; }
    if version >= 8 { contract.trading_class = dec.decode_string()?; }

    let position = dec.decode_decimal()?;
    let market_price = dec.decode_f64()?;
    let market_value = dec.decode_f64()?;
    let (average_cost, unrealized_pnl, realized_pnl) = if version >= 3 {
        (dec.decode_f64()?, dec.decode_f64()?, dec.decode_f64()?)
    } else {
        (0.0, 0.0, 0.0)
    };
    let account_name = if version >= 4 { dec.decode_string()? } else { String::new() };

    Ok(IBEvent::UpdatePortfolio {
        contract: Box::new(contract),
        position, market_price, market_value, average_cost,
        unrealized_pnl, realized_pnl, account_name,
    })
}

/// Decode ACCT_UPDATE_TIME (8). C++ `processAcctUpdateTimeMsg`.
fn decode_acct_update_time(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let timestamp = dec.decode_string()?;
    Ok(IBEvent::UpdateAccountTime { timestamp })
}

/// Decode ACCT_DOWNLOAD_END (54).
fn decode_acct_download_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let account = dec.decode_string()?;
    Ok(IBEvent::AccountDownloadEnd { account })
}

/// Decode ACCOUNT_SUMMARY (63). C++ `processAccountSummaryMsg`.
fn decode_account_summary(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let account = dec.decode_string()?;
    let tag = dec.decode_string()?;
    let value = dec.decode_string()?;
    let currency = dec.decode_string()?;
    Ok(IBEvent::AccountSummary { req_id, account, tag, value, currency })
}

/// Decode ACCOUNT_SUMMARY_END (64).
fn decode_account_summary_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    Ok(IBEvent::AccountSummaryEnd { req_id })
}

/// Decode POSITION_DATA (61). C++ `processPositionDataMsg`.
fn decode_position_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let version = dec.decode_i32()?;
    let account = dec.decode_string()?;
    let mut contract = Contract::default();
    contract.con_id = dec.decode_i32()? as i64;
    contract.symbol = dec.decode_string()?;
    contract.sec_type = dec.decode_enum_opt()?;
    contract.last_trade_date_or_contract_month = dec.decode_string()?;
    contract.strike = dec.decode_f64_max()?;
    contract.right = dec.decode_enum_opt()?;
    contract.multiplier = dec.decode_string()?;
    contract.exchange = dec.decode_string()?;
    contract.currency = dec.decode_string()?;
    contract.local_symbol = dec.decode_string()?;
    if version >= 2 { contract.trading_class = dec.decode_string()?; }
    let position = dec.decode_decimal()?;
    let avg_cost = if version >= 3 { dec.decode_f64()? } else { 0.0 };
    Ok(IBEvent::Position { account, contract: Box::new(contract), position, avg_cost })
}

/// Decode POSITION_MULTI (71).
fn decode_position_multi(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let account = dec.decode_string()?;
    let contract = decode_order_contract(dec)?;
    let pos = dec.decode_decimal()?;
    let avg_cost = dec.decode_f64()?;
    let model_code = dec.decode_string()?;
    Ok(IBEvent::PositionMulti { req_id, account, model_code, contract: Box::new(contract), pos, avg_cost })
}

/// Decode POSITION_MULTI_END (72).
fn decode_position_multi_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    Ok(IBEvent::PositionMultiEnd { req_id })
}

/// Decode ACCOUNT_UPDATE_MULTI (73).
fn decode_account_update_multi(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let account = dec.decode_string()?;
    let model_code = dec.decode_string()?;
    let key = dec.decode_string()?;
    let value = dec.decode_string()?;
    let currency = dec.decode_string()?;
    Ok(IBEvent::AccountUpdateMulti { req_id, account, model_code, key, value, currency })
}

/// Decode ACCOUNT_UPDATE_MULTI_END (74).
fn decode_account_update_multi_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    Ok(IBEvent::AccountUpdateMultiEnd { req_id })
}

// ============================================================================
// Phase 4: Contract Data Decoders
// ============================================================================

/// Decode CONTRACT_DATA (10). C++ `processContractDataMsg`.
fn decode_contract_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let sv = dec.server_version();
    let version = if sv < server_version::SIZE_RULES { dec.decode_i32()? } else { sv };
    let req_id = if version >= 3 { dec.decode_i32()? } else { -1 };

    let mut d = ContractDetails::default();
    d.contract.symbol = dec.decode_string()?;
    d.contract.sec_type = dec.decode_enum_opt()?;
    if sv >= server_version::LAST_TRADE_DATE {
        d.contract.last_trade_date = dec.decode_string()?;
    }
    d.contract.last_trade_date_or_contract_month = dec.decode_string()?;
    d.contract.strike = dec.decode_f64_max()?;
    d.contract.right = dec.decode_enum_opt()?;
    d.contract.exchange = dec.decode_string()?;
    d.contract.currency = dec.decode_string()?;
    d.contract.local_symbol = dec.decode_string()?;
    d.market_name = dec.decode_string()?;
    d.contract.trading_class = dec.decode_string()?;
    d.contract.con_id = dec.decode_i32()? as i64;
    d.min_tick = dec.decode_f64()?;
    if (server_version::MD_SIZE_MULTIPLIER..server_version::SIZE_RULES).contains(&sv) {
        dec.skip_field()?; // mdSizeMultiplier
    }
    d.contract.multiplier = dec.decode_string()?;
    d.order_types = dec.decode_string()?;
    d.valid_exchanges = dec.decode_string()?;
    d.price_magnifier = dec.decode_i64()?;
    if version >= 4 { d.under_con_id = dec.decode_i32()?; }
    if version >= 5 {
        d.long_name = dec.decode_string()?;
        d.contract.primary_exchange = dec.decode_string()?;
    }
    if version >= 6 {
        d.contract_month = dec.decode_string()?;
        d.industry = dec.decode_string()?;
        d.category = dec.decode_string()?;
        d.subcategory = dec.decode_string()?;
        d.time_zone_id = dec.decode_string()?;
        d.trading_hours = dec.decode_string()?;
        d.liquid_hours = dec.decode_string()?;
    }
    if version >= 8 {
        d.ev_rule = dec.decode_string()?;
        d.ev_multiplier = dec.decode_f64()?;
    }
    if version >= 7 {
        let count = dec.decode_i32()?;
        if count > 0 {
            let mut sids = Vec::with_capacity(count as usize);
            for _ in 0..count {
                sids.push(TagValue { tag: dec.decode_string()?, value: dec.decode_string()? });
            }
            d.sec_id_list = Some(sids);
        }
    }
    if sv >= server_version::AGG_GROUP { d.agg_group = dec.decode_i32_max()?; }
    if sv >= server_version::UNDERLYING_INFO {
        d.under_symbol = dec.decode_string()?;
        d.under_sec_type = dec.decode_string()?;
    }
    if sv >= server_version::MARKET_RULES { d.market_rule_ids = dec.decode_string()?; }
    if sv >= server_version::REAL_EXPIRATION_DATE { d.real_expiration_date = dec.decode_string()?; }
    if sv >= server_version::STOCK_TYPE { d.stock_type = dec.decode_string()?; }
    if (server_version::FRACTIONAL_SIZE_SUPPORT..server_version::SIZE_RULES).contains(&sv) {
        dec.skip_field()?; // sizeMinTick
    }
    if sv >= server_version::SIZE_RULES {
        d.min_size = dec.decode_decimal_max()?;
        d.size_increment = dec.decode_decimal_max()?;
        d.suggested_size_increment = dec.decode_decimal_max()?;
    }
    if sv >= server_version::FUND_DATA_FIELDS && d.contract.sec_type == Some(SecType::Fund) {
        d.fund_name = dec.decode_string()?;
        d.fund_family = dec.decode_string()?;
        d.fund_type = dec.decode_string()?;
        d.fund_front_load = dec.decode_string()?;
        d.fund_back_load = dec.decode_string()?;
        d.fund_back_load_time_interval = dec.decode_string()?;
        d.fund_management_fee = dec.decode_string()?;
        d.fund_closed = dec.decode_bool()?;
        d.fund_closed_for_new_investors = dec.decode_bool()?;
        d.fund_closed_for_new_money = dec.decode_bool()?;
        d.fund_notify_amount = dec.decode_string()?;
        d.fund_minimum_initial_purchase = dec.decode_string()?;
        d.fund_subsequent_minimum_purchase = dec.decode_string()?;
        d.fund_blue_sky_states = dec.decode_string()?;
        d.fund_blue_sky_territories = dec.decode_string()?;
        let dp = dec.decode_string()?;
        d.fund_distribution_policy_indicator = match dp.as_str() {
            "Y" | "1" => FundDistributionPolicyIndicator::AccumulationFund,
            "D" | "2" => FundDistributionPolicyIndicator::IncomeFund,
            _ => FundDistributionPolicyIndicator::None,
        };
        let at = dec.decode_string()?;
        d.fund_asset_type = match at.as_str() {
            "001" => FundAssetType::Others,
            "002" => FundAssetType::MoneyMarket,
            "003" => FundAssetType::FixedIncome,
            "004" => FundAssetType::MultiAsset,
            "005" => FundAssetType::Equity,
            "006" => FundAssetType::Sector,
            "007" => FundAssetType::Guaranteed,
            "008" => FundAssetType::Alternative,
            _ => FundAssetType::None,
        };
    }
    if sv >= server_version::INELIGIBILITY_REASONS {
        let count = dec.decode_i32()?;
        if count > 0 {
            let mut reasons = Vec::with_capacity(count as usize);
            for _ in 0..count {
                reasons.push(IneligibilityReason { id: dec.decode_string()?, description: dec.decode_string()? });
            }
            d.ineligibility_reason_list = Some(reasons);
        }
    }
    Ok(IBEvent::ContractDetails { req_id, details: Box::new(d) })
}

/// Decode BOND_CONTRACT_DATA (18). C++ `processBondContractDataMsg`.
fn decode_bond_contract_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let sv = dec.server_version();
    let version = if sv < server_version::SIZE_RULES { dec.decode_i32()? } else { sv };
    let req_id = if version >= 3 { dec.decode_i32()? } else { -1 };

    let mut d = ContractDetails::default();
    d.contract.symbol = dec.decode_string()?;
    d.contract.sec_type = dec.decode_enum_opt()?;
    d.cusip = dec.decode_string()?;
    d.coupon = dec.decode_f64()?;
    if sv >= server_version::LAST_TRADE_DATE {
        d.contract.last_trade_date = dec.decode_string()?;
    }
    d.contract.last_trade_date_or_contract_month = dec.decode_string()?;
    d.issue_date = dec.decode_string()?;
    d.ratings = dec.decode_string()?;
    d.bond_type = dec.decode_string()?;
    d.coupon_type = dec.decode_string()?;
    d.convertible = dec.decode_bool()?;
    d.callable = dec.decode_bool()?;
    d.putable = dec.decode_bool()?;
    d.desc_append = dec.decode_string()?;
    d.contract.exchange = dec.decode_string()?;
    d.contract.currency = dec.decode_string()?;
    d.market_name = dec.decode_string()?;
    d.contract.trading_class = dec.decode_string()?;
    d.contract.con_id = dec.decode_i32()? as i64;
    d.min_tick = dec.decode_f64()?;
    if (server_version::MD_SIZE_MULTIPLIER..server_version::SIZE_RULES).contains(&sv) {
        dec.skip_field()?;
    }
    d.order_types = dec.decode_string()?;
    d.valid_exchanges = dec.decode_string()?;
    if version >= 2 {
        d.next_option_date = dec.decode_string()?;
        d.next_option_type = dec.decode_string()?;
        d.next_option_partial = dec.decode_bool()?;
        d.notes = dec.decode_string()?;
    }
    if version >= 4 { d.long_name = dec.decode_string()?; }
    if sv >= server_version::BOND_TRADING_HOURS {
        d.time_zone_id = dec.decode_string()?;
        d.trading_hours = dec.decode_string()?;
        d.liquid_hours = dec.decode_string()?;
    }
    if version >= 6 {
        d.ev_rule = dec.decode_string()?;
        d.ev_multiplier = dec.decode_f64()?;
    }
    if version >= 5 {
        let count = dec.decode_i32()?;
        if count > 0 {
            let mut sids = Vec::with_capacity(count as usize);
            for _ in 0..count {
                sids.push(TagValue { tag: dec.decode_string()?, value: dec.decode_string()? });
            }
            d.sec_id_list = Some(sids);
        }
    }
    if sv >= server_version::AGG_GROUP { d.agg_group = dec.decode_i32_max()?; }
    if sv >= server_version::MARKET_RULES { d.market_rule_ids = dec.decode_string()?; }
    if sv >= server_version::SIZE_RULES {
        d.min_size = dec.decode_decimal_max()?;
        d.size_increment = dec.decode_decimal_max()?;
        d.suggested_size_increment = dec.decode_decimal_max()?;
    }
    Ok(IBEvent::BondContractDetails { req_id, details: Box::new(d) })
}

/// Decode CONTRACT_DATA_END (52).
fn decode_contract_data_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    Ok(IBEvent::ContractDetailsEnd { req_id })
}

/// Decode SYMBOL_SAMPLES (79).
fn decode_symbol_samples(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let count = dec.decode_i32()?;
    let mut descriptions = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut c = Contract::default();
        c.con_id = dec.decode_i32()? as i64;
        c.symbol = dec.decode_string()?;
        c.sec_type = dec.decode_enum_opt()?;
        c.primary_exchange = dec.decode_string()?;
        c.currency = dec.decode_string()?;
        let n_types = dec.decode_i32()?;
        let mut types = Vec::with_capacity(n_types as usize);
        for _ in 0..n_types { types.push(dec.decode_string()?); }
        c.description = dec.decode_string()?;
        c.issuer_id = dec.decode_string()?;
        descriptions.push(ContractDescription { contract: c, derivative_sec_types: types });
    }
    Ok(IBEvent::SymbolSamples { req_id, descriptions })
}

/// Decode DELTA_NEUTRAL_VALIDATION (56).
fn decode_delta_neutral_validation(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let delta_neutral_contract = DeltaNeutralContract {
        con_id: dec.decode_i32()? as i64,
        delta: dec.decode_f64()?,
        price: dec.decode_f64()?,
    };
    Ok(IBEvent::DeltaNeutralValidation { req_id, delta_neutral_contract })
}

/// Decode SECURITY_DEFINITION_OPTION_PARAMETER (75).
fn decode_sec_def_opt_params(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let exchange = dec.decode_string()?;
    let underlying_con_id = dec.decode_i32()?;
    let trading_class = dec.decode_string()?;
    let multiplier = dec.decode_string()?;
    let exp_count = dec.decode_i32()?;
    let mut expirations = Vec::with_capacity(exp_count as usize);
    for _ in 0..exp_count { expirations.push(dec.decode_string()?); }
    let strike_count = dec.decode_i32()?;
    let mut strikes = Vec::with_capacity(strike_count as usize);
    for _ in 0..strike_count { strikes.push(dec.decode_f64()?); }
    Ok(IBEvent::SecurityDefinitionOptionalParameter {
        req_id, exchange, underlying_con_id, trading_class, multiplier, expirations, strikes,
    })
}

/// Decode SECURITY_DEFINITION_OPTION_PARAMETER_END (76).
fn decode_sec_def_opt_params_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    Ok(IBEvent::SecurityDefinitionOptionalParameterEnd { req_id })
}

// ============================================================================
// Phase 4: Market Depth Decoders
// ============================================================================

/// Decode MARKET_DEPTH (12). C++ `processMarketDepthMsg`.
fn decode_market_depth(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let position = dec.decode_i32()?;
    let operation = dec.decode_i32()?;
    let side = dec.decode_i32()?;
    let price = dec.decode_f64()?;
    let size = dec.decode_decimal()?;
    Ok(IBEvent::UpdateMktDepth { req_id, position, operation, side, price, size })
}

/// Decode MARKET_DEPTH_L2 (13). C++ `processMarketDepthL2Msg`.
fn decode_market_depth_l2(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let position = dec.decode_i32()?;
    let market_maker = dec.decode_string()?;
    let operation = dec.decode_i32()?;
    let side = dec.decode_i32()?;
    let price = dec.decode_f64()?;
    let size = dec.decode_decimal()?;
    let is_smart_depth = if dec.server_version() >= server_version::SMART_DEPTH {
        dec.decode_bool()?
    } else {
        false
    };
    Ok(IBEvent::UpdateMktDepthL2 { req_id, position, market_maker, operation, side, price, size, is_smart_depth })
}

/// Decode MKT_DEPTH_EXCHANGES (80).
fn decode_mkt_depth_exchanges(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let count = dec.decode_i32()?;
    let mut descriptions = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut d = DepthMktDataDescription::default();
        d.exchange = dec.decode_string()?;
        d.sec_type = dec.decode_string()?;
        d.listing_exch = dec.decode_string()?;
        d.service_data_type = dec.decode_string()?;
        if dec.server_version() >= server_version::SERVICE_DATA_TYPE {
            d.agg_group = dec.decode_i32_max()?;
        }
        descriptions.push(d);
    }
    Ok(IBEvent::MktDepthExchanges { descriptions })
}

// ============================================================================
// Phase 4: Historical Data Decoders
// ============================================================================

/// Decode HISTORICAL_DATA (17). C++ `processHistoricalDataMsg`.
fn decode_historical_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let sv = dec.server_version();
    if sv < server_version::SYNT_REALTIME_BARS { dec.skip_field()?; }
    let req_id = dec.decode_i32()?;
    if sv < server_version::HISTORICAL_DATA_END {
        dec.skip_fields(2)?; // startDateStr, endDateStr
    }
    let item_count = dec.decode_i32()?;
    let mut bars = Vec::with_capacity(item_count as usize);
    for _ in 0..item_count {
        let time = dec.decode_string()?;
        let open = dec.decode_f64()?;
        let high = dec.decode_f64()?;
        let low = dec.decode_f64()?;
        let close = dec.decode_f64()?;
        let volume = dec.decode_decimal_max()?;
        let wap = dec.decode_decimal_max()?;
        if sv < server_version::SYNT_REALTIME_BARS { dec.skip_field()?; } // hasGaps
        let count = dec.decode_i32()?;
        bars.push(Bar { time, open, high, low, close, volume, wap, count });
    }
    Ok(IBEvent::HistoricalData { req_id, bars })
}

/// Decode HISTORICAL_DATA_UPDATE (90).
fn decode_historical_data_update(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let bar = Bar {
        time: dec.decode_string()?,
        open: dec.decode_f64()?,
        high: dec.decode_f64()?,
        low: dec.decode_f64()?,
        close: dec.decode_f64()?,
        volume: dec.decode_decimal_max()?,
        wap: dec.decode_decimal_max()?,
        count: dec.decode_i32()?,
    };
    Ok(IBEvent::HistoricalDataUpdate { req_id, bar })
}

/// Decode HISTORICAL_DATA_END (108).
fn decode_historical_data_end_msg(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let start = dec.decode_string()?;
    let end = dec.decode_string()?;
    Ok(IBEvent::HistoricalDataEnd { req_id, start, end })
}

/// Decode HEAD_TIMESTAMP (88).
fn decode_head_timestamp(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let head_timestamp = dec.decode_string()?;
    Ok(IBEvent::HeadTimestamp { req_id, head_timestamp })
}

/// Decode HISTORICAL_TICKS (96).
fn decode_historical_ticks(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let count = dec.decode_i32()?;
    let mut ticks = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let time = dec.decode_i64()?;
        dec.skip_field()?; // unused
        let price = dec.decode_f64()?;
        let size = dec.decode_decimal_max()?;
        ticks.push(HistoricalTick { time, price, size });
    }
    let done = dec.decode_bool()?;
    Ok(IBEvent::HistoricalTicks { req_id, ticks, done })
}

/// Decode HISTORICAL_TICKS_BID_ASK (97).
fn decode_historical_ticks_bid_ask(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let count = dec.decode_i32()?;
    let mut ticks = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let time = dec.decode_i64()?;
        let attr_mask = dec.decode_i32()?;
        let price_bid = dec.decode_f64()?;
        let price_ask = dec.decode_f64()?;
        let size_bid = dec.decode_decimal_max()?;
        let size_ask = dec.decode_decimal_max()?;
        ticks.push(HistoricalTickBidAsk {
            time,
            tick_attrib_bid_ask: TickAttribBidAsk {
                ask_past_high: (attr_mask & 1) != 0,
                bid_past_low: (attr_mask & 2) != 0,
            },
            price_bid, price_ask, size_bid, size_ask,
        });
    }
    let done = dec.decode_bool()?;
    Ok(IBEvent::HistoricalTicksBidAsk { req_id, ticks, done })
}

/// Decode HISTORICAL_TICKS_LAST (98).
fn decode_historical_ticks_last(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let count = dec.decode_i32()?;
    let mut ticks = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let time = dec.decode_i64()?;
        let attr_mask = dec.decode_i32()?;
        let price = dec.decode_f64()?;
        let size = dec.decode_decimal_max()?;
        let exchange = dec.decode_string()?;
        let special_conditions = dec.decode_string()?;
        ticks.push(HistoricalTickLast {
            time,
            tick_attrib_last: TickAttribLast {
                past_limit: (attr_mask & 1) != 0,
                unreported: (attr_mask & 2) != 0,
            },
            price, size, exchange, special_conditions,
        });
    }
    let done = dec.decode_bool()?;
    Ok(IBEvent::HistoricalTicksLast { req_id, ticks, done })
}

/// Decode HISTORICAL_SCHEDULE (106).
fn decode_historical_schedule(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let start_date_time = dec.decode_string()?;
    let end_date_time = dec.decode_string()?;
    let time_zone = dec.decode_string()?;
    let count = dec.decode_i32()?;
    let mut sessions = Vec::with_capacity(count as usize);
    for _ in 0..count {
        sessions.push(HistoricalSession {
            start_date_time: dec.decode_string()?,
            end_date_time: dec.decode_string()?,
            ref_date: dec.decode_string()?,
        });
    }
    Ok(IBEvent::HistoricalSchedule { req_id, start_date_time, end_date_time, time_zone, sessions })
}

// ============================================================================
// Phase 4: Real-time Bars, Scanner, P&L, News, Fundamental Decoders
// ============================================================================

/// Decode REAL_TIME_BARS (50). C++ `processRealTimeBarsMsg`.
fn decode_real_time_bars(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let time = dec.decode_i64()?;
    let open = dec.decode_f64()?;
    let high = dec.decode_f64()?;
    let low = dec.decode_f64()?;
    let close = dec.decode_f64()?;
    let volume = dec.decode_decimal()?;
    let wap = dec.decode_decimal()?;
    let count = dec.decode_i32()?;
    Ok(IBEvent::RealtimeBar { req_id, time, open, high, low, close, volume, wap, count })
}

/// Decode SCANNER_DATA (20). C++ `processScannerDataMsg`.
fn decode_scanner_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let count = dec.decode_i32()?;
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let rank = dec.decode_i32()?;
        let mut d = ContractDetails::default();
        d.contract.con_id = dec.decode_i32()? as i64;
        d.contract.symbol = dec.decode_string()?;
        d.contract.sec_type = dec.decode_enum_opt()?;
        d.contract.last_trade_date_or_contract_month = dec.decode_string()?;
        d.contract.strike = dec.decode_f64_max()?;
        d.contract.right = dec.decode_enum_opt()?;
        d.contract.exchange = dec.decode_string()?;
        d.contract.currency = dec.decode_string()?;
        d.contract.local_symbol = dec.decode_string()?;
        d.market_name = dec.decode_string()?;
        d.contract.trading_class = dec.decode_string()?;
        let distance = dec.decode_string()?;
        let benchmark = dec.decode_string()?;
        let projection = dec.decode_string()?;
        let legs_str = dec.decode_string()?;
        items.push(ScannerDataItem { rank, contract_details: d, distance, benchmark, projection, legs_str });
    }
    Ok(IBEvent::ScannerData { req_id, items })
}

/// Decode SCANNER_PARAMETERS (19).
fn decode_scanner_parameters(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let xml = dec.decode_string()?;
    Ok(IBEvent::ScannerParameters { xml })
}

/// Decode PNL (94). C++ `processPnLMsg`.
fn decode_pnl(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let daily_pnl = dec.decode_f64()?;
    let unrealized_pnl = if dec.server_version() >= server_version::UNREALIZED_PNL {
        dec.decode_f64()?
    } else { f64::MAX };
    let realized_pnl = if dec.server_version() >= server_version::REALIZED_PNL {
        dec.decode_f64()?
    } else { f64::MAX };
    Ok(IBEvent::Pnl { req_id, daily_pnl, unrealized_pnl, realized_pnl })
}

/// Decode PNL_SINGLE (95). C++ `processPnLSingleMsg`.
fn decode_pnl_single(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let pos = dec.decode_decimal()?;
    let daily_pnl = dec.decode_f64()?;
    let unrealized_pnl = if dec.server_version() >= server_version::UNREALIZED_PNL {
        dec.decode_f64()?
    } else { f64::MAX };
    let realized_pnl = if dec.server_version() >= server_version::REALIZED_PNL {
        dec.decode_f64()?
    } else { f64::MAX };
    let value = dec.decode_f64()?;
    Ok(IBEvent::PnlSingle { req_id, pos, daily_pnl, unrealized_pnl, realized_pnl, value })
}

/// Decode NEWS_BULLETINS (14). C++ `processNewsBulletinsMsg`.
fn decode_news_bulletins(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let msg_id = dec.decode_i32()?;
    let msg_type = dec.decode_i32()?;
    let message = dec.decode_string()?;
    let origin_exch = dec.decode_string()?;
    Ok(IBEvent::UpdateNewsBulletin { msg_id, msg_type, message, origin_exch })
}

/// Decode NEWS_ARTICLE (83).
fn decode_news_article(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let article_type = dec.decode_i32()?;
    let article_text = dec.decode_string()?;
    Ok(IBEvent::NewsArticle { req_id, article_type, article_text })
}

/// Decode NEWS_PROVIDERS (85).
fn decode_news_providers(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let count = dec.decode_i32()?;
    let mut providers = Vec::with_capacity(count as usize);
    for _ in 0..count {
        providers.push(NewsProvider {
            provider_code: dec.decode_string()?,
            provider_name: dec.decode_string()?,
        });
    }
    Ok(IBEvent::NewsProviders { providers })
}

/// Decode HISTORICAL_NEWS (86).
fn decode_historical_news(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let time = dec.decode_string()?;
    let provider_code = dec.decode_string()?;
    let article_id = dec.decode_string()?;
    let headline = dec.decode_string()?;
    Ok(IBEvent::HistoricalNews { req_id, time, provider_code, article_id, headline })
}

/// Decode HISTORICAL_NEWS_END (87).
fn decode_historical_news_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let has_more = dec.decode_bool()?;
    Ok(IBEvent::HistoricalNewsEnd { req_id, has_more })
}

/// Decode FUNDAMENTAL_DATA (51).
fn decode_fundamental_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let data = dec.decode_string()?;
    Ok(IBEvent::FundamentalData { req_id, data })
}

// ============================================================================
// Phase 4: Market Rules, Infrastructure, FA, Display, Verification, WSH, User
// ============================================================================

/// Decode MARKET_RULE (93).
fn decode_market_rule(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let market_rule_id = dec.decode_i32()?;
    let count = dec.decode_i32()?;
    let mut price_increments = Vec::with_capacity(count as usize);
    for _ in 0..count {
        price_increments.push(PriceIncrement { low_edge: dec.decode_f64()?, increment: dec.decode_f64()? });
    }
    Ok(IBEvent::MarketRule { market_rule_id, price_increments })
}

/// Decode SMART_COMPONENTS (82).
fn decode_smart_components(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let count = dec.decode_i32()?;
    let mut components = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let bit_number = dec.decode_i32()?;
        let exchange = dec.decode_string()?;
        let letter = dec.decode_string()?;
        let exchange_letter = letter.chars().next().unwrap_or(' ');
        components.push(SmartComponent { bit_number, exchange, exchange_letter });
    }
    Ok(IBEvent::SmartComponents { req_id, components })
}

/// Decode FAMILY_CODES (78).
fn decode_family_codes(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let count = dec.decode_i32()?;
    let mut codes = Vec::with_capacity(count as usize);
    for _ in 0..count {
        codes.push(FamilyCode { account_id: dec.decode_string()?, family_code_str: dec.decode_string()? });
    }
    Ok(IBEvent::FamilyCodes { codes })
}

/// Decode SOFT_DOLLAR_TIERS (77).
fn decode_soft_dollar_tiers(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let count = dec.decode_i32()?;
    let mut tiers = Vec::with_capacity(count as usize);
    for _ in 0..count {
        tiers.push(SoftDollarTier {
            name: dec.decode_string()?,
            val: dec.decode_string()?,
            display_name: dec.decode_string()?,
        });
    }
    Ok(IBEvent::SoftDollarTiers { req_id, tiers })
}

/// Decode HISTOGRAM_DATA (89).
fn decode_histogram_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let count = dec.decode_i32()?;
    let mut data = Vec::with_capacity(count as usize);
    for _ in 0..count {
        data.push(HistogramEntry { price: dec.decode_f64()?, size: dec.decode_decimal_max()? });
    }
    Ok(IBEvent::HistogramData { req_id, data })
}

/// Decode REROUTE_MKT_DATA_REQ (91).
fn decode_reroute_mkt_data_req(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let con_id = dec.decode_i32()?;
    let exchange = dec.decode_string()?;
    Ok(IBEvent::RerouteMktDataReq { req_id, con_id, exchange })
}

/// Decode REROUTE_MKT_DEPTH_REQ (92).
fn decode_reroute_mkt_depth_req(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let con_id = dec.decode_i32()?;
    let exchange = dec.decode_string()?;
    Ok(IBEvent::RerouteMktDepthReq { req_id, con_id, exchange })
}

/// Decode RECEIVE_FA (16).
fn decode_receive_fa(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let fa_data_type = dec.decode_i32()?;
    let xml = dec.decode_string()?;
    Ok(IBEvent::ReceiveFa { fa_data_type, xml })
}

/// Decode REPLACE_FA_END (103).
fn decode_replace_fa_end(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let text = dec.decode_string()?;
    Ok(IBEvent::ReplaceFaEnd { req_id, text })
}

/// Decode DISPLAY_GROUP_LIST (67).
fn decode_display_group_list(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let groups = dec.decode_string()?;
    Ok(IBEvent::DisplayGroupList { req_id, groups })
}

/// Decode DISPLAY_GROUP_UPDATED (68).
fn decode_display_group_updated(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let req_id = dec.decode_i32()?;
    let contract_info = dec.decode_string()?;
    Ok(IBEvent::DisplayGroupUpdated { req_id, contract_info })
}

/// Decode VERIFY_MESSAGE_API (65).
fn decode_verify_message_api(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let api_data = dec.decode_string()?;
    Ok(IBEvent::VerifyMessageApi { api_data })
}

/// Decode VERIFY_COMPLETED (66).
fn decode_verify_completed(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let is_successful = dec.decode_string()? == "true";
    let error_text = dec.decode_string()?;
    Ok(IBEvent::VerifyCompleted { is_successful, error_text })
}

/// Decode VERIFY_AND_AUTH_MESSAGE_API (69).
fn decode_verify_and_auth_message_api(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let api_data = dec.decode_string()?;
    let xyz_challenge = dec.decode_string()?;
    Ok(IBEvent::VerifyAndAuthMessageApi { api_data, xyz_challenge })
}

/// Decode VERIFY_AND_AUTH_COMPLETED (70).
fn decode_verify_and_auth_completed(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let _version = dec.decode_i32()?;
    let is_successful = dec.decode_string()? == "true";
    let error_text = dec.decode_string()?;
    Ok(IBEvent::VerifyAndAuthCompleted { is_successful, error_text })
}

/// Decode WSH_META_DATA (104).
fn decode_wsh_meta_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let data_json = dec.decode_string()?;
    Ok(IBEvent::WshMetaData { req_id, data_json })
}

/// Decode WSH_EVENT_DATA (105).
fn decode_wsh_event_data(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let data_json = dec.decode_string()?;
    Ok(IBEvent::WshEventData { req_id, data_json })
}

/// Decode USER_INFO (107).
fn decode_user_info(dec: &mut MessageDecoder) -> Result<IBEvent> {
    let req_id = dec.decode_i32()?;
    let white_branding_id = dec.decode_string()?;
    Ok(IBEvent::UserInfo { req_id, white_branding_id })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::enums::SecType;
    use crate::wrapper::IBEvent;

    /// Helper: build a byte buffer from null-terminated fields.
    fn make_fields(fields: &[&str]) -> Vec<u8> {
        let mut buf = Vec::new();
        for f in fields {
            buf.extend_from_slice(f.as_bytes());
            buf.push(0);
        }
        buf
    }

    #[test]
    fn decode_string_basic() {
        let data = make_fields(&["hello"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_string().unwrap(), "hello");
        assert!(!dec.has_remaining());
    }

    #[test]
    fn decode_string_empty() {
        let data = make_fields(&[""]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_string().unwrap(), "");
    }

    #[test]
    fn decode_i32_basic() {
        let data = make_fields(&["42"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_i32().unwrap(), 42);
    }

    #[test]
    fn decode_i32_negative() {
        let data = make_fields(&["-7"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_i32().unwrap(), -7);
    }

    #[test]
    fn decode_i32_empty_is_zero() {
        let data = make_fields(&[""]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_i32().unwrap(), 0);
    }

    #[test]
    fn decode_i64_basic() {
        let data = make_fields(&["1234567890123"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_i64().unwrap(), 1234567890123);
    }

    #[test]
    fn decode_f64_normal() {
        let data = make_fields(&["1.23"]);
        let mut dec = MessageDecoder::new(&data, 150);
        let v = dec.decode_f64().unwrap();
        assert!((v - 1.23).abs() < 1e-10);
    }

    #[test]
    fn decode_f64_infinity() {
        let data = make_fields(&["Infinity"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_f64().unwrap(), f64::INFINITY);
    }

    #[test]
    fn decode_f64_empty_is_zero() {
        let data = make_fields(&[""]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_f64().unwrap(), 0.0);
    }

    #[test]
    fn decode_bool_true() {
        let data = make_fields(&["1"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert!(dec.decode_bool().unwrap());
    }

    #[test]
    fn decode_bool_false() {
        let data = make_fields(&["0"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert!(!dec.decode_bool().unwrap());
    }

    #[test]
    fn decode_bool_greater_than_one() {
        // C++ semantics: > 0 → true
        let data = make_fields(&["2"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert!(dec.decode_bool().unwrap());
    }

    #[test]
    fn decode_decimal_basic() {
        let data = make_fields(&["123.456"]);
        let mut dec = MessageDecoder::new(&data, 150);
        let d = dec.decode_decimal().unwrap();
        assert_eq!(d, Decimal::from_str("123.456").unwrap());
    }

    #[test]
    fn decode_decimal_empty_is_zero() {
        let data = make_fields(&[""]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_decimal().unwrap(), Decimal::ZERO);
    }

    #[test]
    fn decode_i32_max_empty_is_none() {
        let data = make_fields(&[""]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_i32_max().unwrap(), None);
    }

    #[test]
    fn decode_i32_max_value() {
        let data = make_fields(&["42"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_i32_max().unwrap(), Some(42));
    }

    #[test]
    fn decode_f64_max_empty_is_none() {
        let data = make_fields(&[""]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_f64_max().unwrap(), None);
    }

    #[test]
    fn decode_f64_max_value() {
        let data = make_fields(&["1.23"]);
        let mut dec = MessageDecoder::new(&data, 150);
        let v = dec.decode_f64_max().unwrap();
        assert!((v.unwrap() - 1.23).abs() < 1e-10);
    }

    #[test]
    fn decode_f64_max_infinity() {
        let data = make_fields(&["Infinity"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_f64_max().unwrap(), Some(f64::INFINITY));
    }

    #[test]
    fn decode_decimal_max_empty_is_none() {
        let data = make_fields(&[""]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_decimal_max().unwrap(), None);
    }

    #[test]
    fn decode_decimal_max_value() {
        let data = make_fields(&["99.99"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(
            dec.decode_decimal_max().unwrap(),
            Some(Decimal::from_str("99.99").unwrap())
        );
    }

    #[test]
    fn decode_raw_int_basic() {
        let data = 42_i32.to_be_bytes();
        let mut dec = MessageDecoder::new(&data, 201);
        assert_eq!(dec.decode_raw_int().unwrap(), 42);
    }

    #[test]
    fn decode_raw_int_large() {
        let data = 0x01020304_i32.to_be_bytes();
        let mut dec = MessageDecoder::new(&data, 201);
        assert_eq!(dec.decode_raw_int().unwrap(), 0x01020304);
    }

    #[test]
    fn decode_raw_int_insufficient_bytes() {
        let data = [0u8, 0, 0]; // only 3 bytes
        let mut dec = MessageDecoder::new(&data, 201);
        assert!(dec.decode_raw_int().is_err());
    }

    #[test]
    fn decode_msg_id_text_mode() {
        let data = make_fields(&["3"]);
        let mut dec = MessageDecoder::new(&data, 150); // < PROTOBUF
        assert_eq!(dec.decode_msg_id().unwrap(), 3);
    }

    #[test]
    fn decode_msg_id_raw_mode() {
        let data = 3_i32.to_be_bytes();
        let mut dec = MessageDecoder::new(&data, 201); // >= PROTOBUF
        assert_eq!(dec.decode_msg_id().unwrap(), 3);
    }

    #[test]
    fn decode_enum_sec_type() {
        let data = make_fields(&["STK"]);
        let mut dec = MessageDecoder::new(&data, 150);
        let st: SecType = dec.decode_enum().unwrap();
        assert_eq!(st, SecType::Stock);
    }

    #[test]
    fn decode_enum_opt_some() {
        let data = make_fields(&["OPT"]);
        let mut dec = MessageDecoder::new(&data, 150);
        let st: Option<SecType> = dec.decode_enum_opt().unwrap();
        assert_eq!(st, Some(SecType::Option));
    }

    #[test]
    fn decode_enum_opt_none() {
        let data = make_fields(&[""]);
        let mut dec = MessageDecoder::new(&data, 150);
        let st: Option<SecType> = dec.decode_enum_opt().unwrap();
        assert_eq!(st, None);
    }

    #[test]
    fn decode_multiple_fields() {
        let data = make_fields(&["42", "AAPL", "1"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.decode_i32().unwrap(), 42);
        assert_eq!(dec.decode_string().unwrap(), "AAPL");
        assert!(dec.decode_bool().unwrap());
        assert!(!dec.has_remaining());
    }

    #[test]
    fn skip_field() {
        let data = make_fields(&["skip_me", "42"]);
        let mut dec = MessageDecoder::new(&data, 150);
        dec.skip_field().unwrap();
        assert_eq!(dec.decode_i32().unwrap(), 42);
    }

    #[test]
    fn skip_fields_multiple() {
        let data = make_fields(&["a", "b", "c", "42"]);
        let mut dec = MessageDecoder::new(&data, 150);
        dec.skip_fields(3).unwrap();
        assert_eq!(dec.decode_i32().unwrap(), 42);
    }

    #[test]
    fn decode_past_end_errors() {
        let data: &[u8] = &[];
        let mut dec = MessageDecoder::new(data, 150);
        assert!(dec.decode_string().is_err());
    }

    #[test]
    fn decode_no_null_terminator_errors() {
        let data = b"no null here";
        let mut dec = MessageDecoder::new(data, 150);
        assert!(dec.decode_string().is_err());
    }

    #[test]
    fn roundtrip_encode_decode() {
        use crate::encoder::MessageEncoder;
        use crate::protocol::HEADER_LEN;

        let mut enc = MessageEncoder::new(150);
        enc.encode_field_i32(42)
            .encode_field_str("hello")
            .encode_field_f64(1.23)
            .encode_field_bool(true)
            .encode_field_max_i32(None)
            .encode_field_max_f64(Some(1.5));
        let buf = enc.finalize().unwrap();
        let body = &buf[HEADER_LEN..];

        let mut dec = MessageDecoder::new(body, 150);
        assert_eq!(dec.decode_i32().unwrap(), 42);
        assert_eq!(dec.decode_string().unwrap(), "hello");
        let f = dec.decode_f64().unwrap();
        assert!((f - 1.23).abs() < 1e-10);
        assert!(dec.decode_bool().unwrap());
        assert_eq!(dec.decode_i32_max().unwrap(), None);
        let f2 = dec.decode_f64_max().unwrap();
        assert!((f2.unwrap() - 1.5).abs() < 1e-10);
        assert!(!dec.has_remaining());
    }

    #[test]
    fn position_tracking() {
        let data = make_fields(&["AB", "CD"]);
        let mut dec = MessageDecoder::new(&data, 150);
        assert_eq!(dec.position(), 0);
        dec.decode_string().unwrap();
        assert_eq!(dec.position(), 3); // "AB" + \0 = 3 bytes
        dec.decode_string().unwrap();
        assert_eq!(dec.position(), 6); // "CD" + \0 = 3 more bytes
    }

    // ========================================================================
    // decode_server_msg tests
    // ========================================================================

    #[test]
    fn decode_server_msg_err_msg_v2() {
        // ERR_MSG: msg_id=4, version=2, id=1, errorCode=200, errorMsg="no security"
        let data = make_fields(&["4", "2", "1", "200", "no security"]);
        let event = super::decode_server_msg(&data, 150);
        match event {
            IBEvent::Error {
                req_id,
                code,
                message,
                ..
            } => {
                assert_eq!(req_id, 1);
                assert_eq!(code, 200);
                assert_eq!(message, "no security");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn decode_server_msg_err_msg_v1() {
        // Old format: version=1, just a message
        let data = make_fields(&["4", "1", "some error"]);
        let event = super::decode_server_msg(&data, 150);
        match event {
            IBEvent::Error {
                req_id, message, ..
            } => {
                assert_eq!(req_id, -1);
                assert_eq!(message, "some error");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn decode_server_msg_next_valid_id() {
        // NEXT_VALID_ID: msg_id=9, version=1, orderId=100
        let data = make_fields(&["9", "1", "100"]);
        let event = super::decode_server_msg(&data, 150);
        match event {
            IBEvent::NextValidId { order_id } => {
                assert_eq!(order_id, 100);
            }
            other => panic!("expected NextValidId, got {other:?}"),
        }
    }

    #[test]
    fn decode_server_msg_managed_accts() {
        // MANAGED_ACCTS: msg_id=15, version=1, accounts="DU123,DU456"
        let data = make_fields(&["15", "1", "DU123,DU456"]);
        let event = super::decode_server_msg(&data, 150);
        match event {
            IBEvent::ManagedAccounts { accounts } => {
                assert_eq!(accounts, "DU123,DU456");
            }
            other => panic!("expected ManagedAccounts, got {other:?}"),
        }
    }

    #[test]
    fn decode_server_msg_current_time() {
        // CURRENT_TIME: msg_id=49, version=1, time=1708876800
        let data = make_fields(&["49", "1", "1708876800"]);
        let event = super::decode_server_msg(&data, 150);
        match event {
            IBEvent::CurrentTime { time } => {
                assert_eq!(time, 1708876800);
            }
            other => panic!("expected CurrentTime, got {other:?}"),
        }
    }

    #[test]
    fn decode_server_msg_current_time_in_millis() {
        // CURRENT_TIME_IN_MILLIS: msg_id=109, time=1708876800000
        let data = make_fields(&["109", "1708876800000"]);
        let event = super::decode_server_msg(&data, 150);
        match event {
            IBEvent::CurrentTimeInMillis { time_in_millis } => {
                assert_eq!(time_in_millis, 1708876800000);
            }
            other => panic!("expected CurrentTimeInMillis, got {other:?}"),
        }
    }

    #[test]
    fn decode_server_msg_unknown() {
        // Unknown message ID (below PROTOBUF_MSG_ID=200 to avoid protobuf dispatch)
        let data = make_fields(&["199", "some", "data"]);
        let event = super::decode_server_msg(&data, 150);
        match event {
            IBEvent::Unknown { msg_id, .. } => {
                assert_eq!(msg_id, 199);
            }
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn decode_server_msg_invalid_data() {
        // Empty data should return Unknown with msg_id=-1
        let event = super::decode_server_msg(&[], 150);
        match event {
            IBEvent::Unknown { msg_id, .. } => {
                assert_eq!(msg_id, -1);
            }
            other => panic!("expected Unknown for empty data, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Tick data decoder tests
    // ========================================================================

    #[test]
    fn decode_tick_price_msg() {
        // TICK_PRICE: msg_id=1, version=6, req_id=1, tick_type=1(BID), price=150.25, size=100, attrib_mask=0
        let data = make_fields(&["1", "6", "1", "1", "150.25", "100", "0"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::TickPrice { req_id, tick_type, price, attrib, size } => {
                assert_eq!(req_id, 1);
                assert_eq!(tick_type, crate::protocol::TickType::Bid);
                assert!((price - 150.25).abs() < 1e-10);
                assert!(!attrib.can_auto_execute);
                assert_eq!(size, rust_decimal::Decimal::from(100));
            }
            other => panic!("expected TickPrice, got {other:?}"),
        }
    }

    #[test]
    fn decode_tick_size_msg() {
        // TICK_SIZE: msg_id=2, version=2, req_id=1, tick_type=0(BID_SIZE), size=500
        let data = make_fields(&["2", "2", "1", "0", "500"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::TickSize { req_id, tick_type, size } => {
                assert_eq!(req_id, 1);
                assert_eq!(tick_type, crate::protocol::TickType::BidSize);
                assert_eq!(size, rust_decimal::Decimal::from(500));
            }
            other => panic!("expected TickSize, got {other:?}"),
        }
    }

    #[test]
    fn decode_tick_generic_msg() {
        // TICK_GENERIC: msg_id=45, version=2, req_id=1, tick_type=49(HALTED), value=0.0
        let data = make_fields(&["45", "2", "1", "49", "0.0"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::TickGeneric { req_id, tick_type, value } => {
                assert_eq!(req_id, 1);
                assert_eq!(tick_type, crate::protocol::TickType::Halted);
                assert!((value - 0.0).abs() < 1e-10);
            }
            other => panic!("expected TickGeneric, got {other:?}"),
        }
    }

    #[test]
    fn decode_tick_string_msg() {
        // TICK_STRING: msg_id=46, version=2, req_id=1, tick_type=45(LAST_TIMESTAMP), value="1708876800"
        let data = make_fields(&["46", "2", "1", "45", "1708876800"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::TickString { req_id, tick_type, value } => {
                assert_eq!(req_id, 1);
                assert_eq!(tick_type, crate::protocol::TickType::LastTimestamp);
                assert_eq!(value, "1708876800");
            }
            other => panic!("expected TickString, got {other:?}"),
        }
    }

    #[test]
    fn decode_tick_snapshot_end_msg() {
        // TICK_SNAPSHOT_END: msg_id=57, version=1, req_id=5
        let data = make_fields(&["57", "1", "5"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::TickSnapshotEnd { req_id } => assert_eq!(req_id, 5),
            other => panic!("expected TickSnapshotEnd, got {other:?}"),
        }
    }

    #[test]
    fn decode_market_data_type_msg() {
        // MARKET_DATA_TYPE: msg_id=58, version=1, req_id=1, data_type=3 (Delayed)
        let data = make_fields(&["58", "1", "1", "3"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::MarketDataType { req_id, market_data_type } => {
                assert_eq!(req_id, 1);
                assert_eq!(market_data_type, 3);
            }
            other => panic!("expected MarketDataType, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Order decoder tests
    // ========================================================================

    #[test]
    fn decode_order_status_msg() {
        // ORDER_STATUS: msg_id=3, sv=176 >= MARKET_CAP_PRICE(131) → no version field
        // sv=176 < PERM_ID_AS_LONG(191) → orderId/permId as i32
        // orderId=100, status="Filled", filled=10, remaining=0,
        // avgFillPrice=150.50, permId=200, parentId=0, lastFillPrice=150.50,
        // clientId=1, whyHeld="", mktCapPrice=0.0
        let data = make_fields(&["3", "100", "Filled", "10", "0",
            "150.50", "200", "0", "150.50", "1", "", "0.0"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::OrderStatus { order_id, status, filled, remaining, avg_fill_price, perm_id, .. } => {
                assert_eq!(order_id, 100);
                assert_eq!(status, "Filled");
                assert_eq!(filled, rust_decimal::Decimal::from(10));
                assert_eq!(remaining, rust_decimal::Decimal::from(0));
                assert!((avg_fill_price - 150.50).abs() < 1e-10);
                assert_eq!(perm_id, 200);
            }
            other => panic!("expected OrderStatus, got {other:?}"),
        }
    }

    #[test]
    fn decode_order_bound_msg() {
        // ORDER_BOUND: msg_id=100, permId=200, clientId=1, orderId=50
        let data = make_fields(&["100", "200", "1", "50"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::OrderBound { perm_id, client_id, order_id } => {
                assert_eq!(perm_id, 200);
                assert_eq!(client_id, 1);
                assert_eq!(order_id, 50);
            }
            other => panic!("expected OrderBound, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Account decoder tests
    // ========================================================================

    #[test]
    fn decode_acct_value_msg() {
        // ACCT_VALUE: msg_id=6, version=2, key="NetLiquidation", value="100000", currency="USD", accountName="DU123"
        let data = make_fields(&["6", "2", "NetLiquidation", "100000", "USD", "DU123"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::UpdateAccountValue { key, value, currency, account_name } => {
                assert_eq!(key, "NetLiquidation");
                assert_eq!(value, "100000");
                assert_eq!(currency, "USD");
                assert_eq!(account_name, "DU123");
            }
            other => panic!("expected UpdateAccountValue, got {other:?}"),
        }
    }

    #[test]
    fn decode_acct_update_time_msg() {
        // ACCT_UPDATE_TIME: msg_id=8, version=1, timestamp="15:30:00"
        let data = make_fields(&["8", "1", "15:30:00"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::UpdateAccountTime { timestamp } => {
                assert_eq!(timestamp, "15:30:00");
            }
            other => panic!("expected UpdateAccountTime, got {other:?}"),
        }
    }

    #[test]
    fn decode_account_summary_msg() {
        // ACCOUNT_SUMMARY: msg_id=63, version=1, req_id=1, account="DU123",
        // tag="NetLiquidation", value="100000", currency="USD"
        let data = make_fields(&["63", "1", "1", "DU123", "NetLiquidation", "100000", "USD"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::AccountSummary { req_id, account, tag, value, currency } => {
                assert_eq!(req_id, 1);
                assert_eq!(account, "DU123");
                assert_eq!(tag, "NetLiquidation");
                assert_eq!(value, "100000");
                assert_eq!(currency, "USD");
            }
            other => panic!("expected AccountSummary, got {other:?}"),
        }
    }

    #[test]
    fn decode_account_summary_end_msg() {
        // ACCOUNT_SUMMARY_END: msg_id=64, version=1, req_id=1
        let data = make_fields(&["64", "1", "1"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::AccountSummaryEnd { req_id } => assert_eq!(req_id, 1),
            other => panic!("expected AccountSummaryEnd, got {other:?}"),
        }
    }

    #[test]
    fn decode_acct_download_end_msg() {
        // ACCT_DOWNLOAD_END: msg_id=54, version=1, accountName="DU123"
        let data = make_fields(&["54", "1", "DU123"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::AccountDownloadEnd { account } => assert_eq!(account, "DU123"),
            other => panic!("expected AccountDownloadEnd, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Position decoder tests
    // ========================================================================

    #[test]
    fn decode_position_data_msg() {
        // POSITION_DATA: msg_id=61, version=3,
        // account="DU123", conId=265598, symbol="AAPL", secType="STK",
        // lastTradeDateOrContractMonth="", strike=0.0, right="", multiplier="",
        // exchange="", currency="USD", localSymbol="AAPL", tradingClass="AAPL",
        // pos=100, avgCost=150.00
        let data = make_fields(&["61", "3",
            "DU123", "265598", "AAPL", "STK", "", "0", "", "", "", "USD", "AAPL", "AAPL",
            "100", "150.00"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::Position { account, contract, position, avg_cost } => {
                assert_eq!(account, "DU123");
                assert_eq!(contract.symbol, "AAPL");
                assert_eq!(position, rust_decimal::Decimal::from(100));
                assert!((avg_cost - 150.0).abs() < 1e-10);
            }
            other => panic!("expected Position, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Execution decoder tests
    // ========================================================================

    #[test]
    fn decode_commission_report_msg() {
        // COMMISSION_REPORT: msg_id=59, version=1,
        // execId="0001", commission=1.0, currency="USD",
        // realizedPnl=0.0, yield_=0.0, yieldRedemptionDate=0
        let data = make_fields(&["59", "1", "0001", "1.0", "USD", "0.0", "0.0", "0"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::CommissionReport { report } => {
                assert_eq!(report.exec_id, "0001");
                assert!((report.commission_and_fees - 1.0).abs() < 1e-10);
                assert_eq!(report.currency, "USD");
            }
            other => panic!("expected CommissionAndFeesReport, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Market depth decoder tests
    // ========================================================================

    #[test]
    fn decode_market_depth_msg() {
        // MARKET_DEPTH: msg_id=12, version=1, req_id=1, position=0, operation=0, side=1, price=150.0, size=100
        let data = make_fields(&["12", "1", "1", "0", "0", "1", "150.0", "100"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::UpdateMktDepth { req_id, position, operation, side, price, size } => {
                assert_eq!(req_id, 1);
                assert_eq!(position, 0);
                assert_eq!(operation, 0);
                assert_eq!(side, 1);
                assert!((price - 150.0).abs() < 1e-10);
                assert_eq!(size, rust_decimal::Decimal::from(100));
            }
            other => panic!("expected UpdateMktDepth, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Historical data decoder tests
    // ========================================================================

    #[test]
    fn decode_historical_data_msg() {
        // HISTORICAL_DATA: msg_id=17, sv=176
        // sv >= SYNT_REALTIME_BARS(124) → no version field, no hasGaps in bars
        // sv < HISTORICAL_DATA_END(196) → startDateStr + endDateStr skipped
        // req_id=1, startDate, endDate (skipped),
        // itemCount=2, bars with time/open/high/low/close/volume/wap/count
        let data = make_fields(&["17",
            "1", "20260101", "20260201",
            "2",
            "20260101", "100.0", "105.0", "99.0", "104.0", "1000000", "102.5", "500",
            "20260102", "104.0", "106.0", "103.0", "105.5", "900000", "104.5", "450"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::HistoricalData { req_id, bars } => {
                assert_eq!(req_id, 1);
                assert_eq!(bars.len(), 2);
                assert_eq!(bars[0].time, "20260101");
                assert!((bars[0].open - 100.0).abs() < 1e-10);
                assert!((bars[0].high - 105.0).abs() < 1e-10);
                assert_eq!(bars[1].time, "20260102");
                assert!((bars[1].close - 105.5).abs() < 1e-10);
            }
            other => panic!("expected HistoricalData, got {other:?}"),
        }
    }

    #[test]
    fn decode_real_time_bars_msg() {
        // REAL_TIME_BARS: msg_id=50, version=3,
        // req_id=1, time=1708876800, open=150.0, high=151.0, low=149.0,
        // close=150.5, volume=1000, wap=150.25, count=50
        let data = make_fields(&["50", "3", "1", "1708876800",
            "150.0", "151.0", "149.0", "150.5", "1000", "150.25", "50"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::RealtimeBar { req_id, time, open, high, close, .. } => {
                assert_eq!(req_id, 1);
                assert_eq!(time, 1708876800);
                assert!((open - 150.0).abs() < 1e-10);
                assert!((high - 151.0).abs() < 1e-10);
                assert!((close - 150.5).abs() < 1e-10);
            }
            other => panic!("expected RealtimeBar, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: P&L decoder tests
    // ========================================================================

    #[test]
    fn decode_pnl_msg() {
        // PNL: msg_id=94, req_id=1, daily_pnl=250.50, unrealized_pnl=500.0, realized_pnl=100.0
        let data = make_fields(&["94", "1", "250.50", "500.0", "100.0"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::Pnl { req_id, daily_pnl, unrealized_pnl, realized_pnl } => {
                assert_eq!(req_id, 1);
                assert!((daily_pnl - 250.50).abs() < 1e-10);
                assert!((unrealized_pnl - 500.0).abs() < 1e-10);
                assert!((realized_pnl - 100.0).abs() < 1e-10);
            }
            other => panic!("expected Pnl, got {other:?}"),
        }
    }

    #[test]
    fn decode_pnl_single_msg() {
        // PNL_SINGLE: msg_id=95, req_id=1, pos=100, daily_pnl=25.50,
        // unrealized_pnl=50.0, realized_pnl=10.0, value=15025.0
        let data = make_fields(&["95", "1", "100", "25.50", "50.0", "10.0", "15025.0"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::PnlSingle { req_id, pos, daily_pnl, unrealized_pnl, realized_pnl, value } => {
                assert_eq!(req_id, 1);
                assert_eq!(pos, rust_decimal::Decimal::from(100));
                assert!((daily_pnl - 25.50).abs() < 1e-10);
                assert!((unrealized_pnl - 50.0).abs() < 1e-10);
                assert!((realized_pnl - 10.0).abs() < 1e-10);
                assert!((value - 15025.0).abs() < 1e-10);
            }
            other => panic!("expected PnlSingle, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: News decoder tests
    // ========================================================================

    #[test]
    fn decode_news_article_msg() {
        // NEWS_ARTICLE: msg_id=83, req_id=1, articleType=0, articleText="Breaking news..."
        let data = make_fields(&["83", "1", "0", "Breaking news..."]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::NewsArticle { req_id, article_type, article_text } => {
                assert_eq!(req_id, 1);
                assert_eq!(article_type, 0);
                assert_eq!(article_text, "Breaking news...");
            }
            other => panic!("expected NewsArticle, got {other:?}"),
        }
    }

    #[test]
    fn decode_news_bulletins_msg() {
        // NEWS_BULLETINS: msg_id=14, version=1, msg_id=1,
        // msg_type=1, message="System message", origin_exch="NYSE"
        let data = make_fields(&["14", "1", "1", "1", "System message", "NYSE"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::UpdateNewsBulletin { msg_id, msg_type, message, origin_exch } => {
                assert_eq!(msg_id, 1);
                assert_eq!(msg_type, 1);
                assert_eq!(message, "System message");
                assert_eq!(origin_exch, "NYSE");
            }
            other => panic!("expected UpdateNewsBulletin, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Infrastructure decoder tests
    // ========================================================================

    #[test]
    fn decode_market_rule_msg() {
        // MARKET_RULE: msg_id=93, market_rule_id=1, count=2,
        // rule1: low_edge=0.0, increment=0.01
        // rule2: low_edge=1.0, increment=0.05
        let data = make_fields(&["93", "1", "2", "0.0", "0.01", "1.0", "0.05"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::MarketRule { market_rule_id, price_increments } => {
                assert_eq!(market_rule_id, 1);
                assert_eq!(price_increments.len(), 2);
                assert!((price_increments[0].low_edge - 0.0).abs() < 1e-10);
                assert!((price_increments[0].increment - 0.01).abs() < 1e-10);
                assert!((price_increments[1].low_edge - 1.0).abs() < 1e-10);
            }
            other => panic!("expected MarketRule, got {other:?}"),
        }
    }

    #[test]
    fn decode_family_codes_msg() {
        // FAMILY_CODES: msg_id=78, count=2,
        // code1: accountId="DU123", familyCodeStr="F1"
        // code2: accountId="DU456", familyCodeStr="F2"
        let data = make_fields(&["78", "2", "DU123", "F1", "DU456", "F2"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::FamilyCodes { codes } => {
                assert_eq!(codes.len(), 2);
                assert_eq!(codes[0].account_id, "DU123");
                assert_eq!(codes[0].family_code_str, "F1");
                assert_eq!(codes[1].account_id, "DU456");
            }
            other => panic!("expected FamilyCodes, got {other:?}"),
        }
    }

    #[test]
    fn decode_fundamental_data_msg() {
        // FUNDAMENTAL_DATA: msg_id=51, version=1, req_id=1, data="<xml>...</xml>"
        let data = make_fields(&["51", "1", "1", "<xml>data</xml>"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::FundamentalData { req_id, data } => {
                assert_eq!(req_id, 1);
                assert_eq!(data, "<xml>data</xml>");
            }
            other => panic!("expected FundamentalData, got {other:?}"),
        }
    }

    #[test]
    fn decode_scanner_parameters_msg() {
        // SCANNER_PARAMETERS: msg_id=19, version=1, xml="<params/>"
        let data = make_fields(&["19", "1", "<params/>"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::ScannerParameters { xml } => {
                assert_eq!(xml, "<params/>");
            }
            other => panic!("expected ScannerParameters, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: WSH / User Info decoder tests
    // ========================================================================

    #[test]
    fn decode_wsh_meta_data_msg() {
        // WSH_META_DATA: msg_id=104, req_id=1, data_json="{}"
        let data = make_fields(&["104", "1", "{}"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::WshMetaData { req_id, data_json } => {
                assert_eq!(req_id, 1);
                assert_eq!(data_json, "{}");
            }
            other => panic!("expected WshMetaData, got {other:?}"),
        }
    }

    #[test]
    fn decode_wsh_event_data_msg() {
        // WSH_EVENT_DATA: msg_id=105, req_id=1, data_json="{\"event\":\"test\"}"
        let data = make_fields(&["105", "1", "{\"event\":\"test\"}"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::WshEventData { req_id, data_json } => {
                assert_eq!(req_id, 1);
                assert!(data_json.contains("event"));
            }
            other => panic!("expected WshEventData, got {other:?}"),
        }
    }

    #[test]
    fn decode_user_info_msg() {
        // USER_INFO: msg_id=107, req_id=1, whiteBrandingId="WB123"
        let data = make_fields(&["107", "1", "WB123"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::UserInfo { req_id, white_branding_id } => {
                assert_eq!(req_id, 1);
                assert_eq!(white_branding_id, "WB123");
            }
            other => panic!("expected UserInfo, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Verification decoder tests
    // ========================================================================

    #[test]
    fn decode_verify_completed_msg() {
        // VERIFY_COMPLETED: msg_id=66, version=1, isSuccessful="true", errorText=""
        let data = make_fields(&["66", "1", "true", ""]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::VerifyCompleted { is_successful, error_text } => {
                assert!(is_successful);
                assert_eq!(error_text, "");
            }
            other => panic!("expected VerifyCompleted, got {other:?}"),
        }
    }

    #[test]
    fn decode_verify_completed_failure_msg() {
        // VERIFY_COMPLETED: msg_id=66, version=1, isSuccessful="false", errorText="auth failed"
        let data = make_fields(&["66", "1", "false", "auth failed"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::VerifyCompleted { is_successful, error_text } => {
                assert!(!is_successful);
                assert_eq!(error_text, "auth failed");
            }
            other => panic!("expected VerifyCompleted, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Display group decoder tests
    // ========================================================================

    #[test]
    fn decode_display_group_list_msg() {
        // DISPLAY_GROUP_LIST: msg_id=67, version=1, req_id=1, groups="1|2|3"
        let data = make_fields(&["67", "1", "1", "1|2|3"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::DisplayGroupList { req_id, groups } => {
                assert_eq!(req_id, 1);
                assert_eq!(groups, "1|2|3");
            }
            other => panic!("expected DisplayGroupList, got {other:?}"),
        }
    }

    #[test]
    fn decode_display_group_updated_msg() {
        // DISPLAY_GROUP_UPDATED: msg_id=68, version=1, req_id=1, contractInfo="265598@SMART"
        let data = make_fields(&["68", "1", "1", "265598@SMART"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::DisplayGroupUpdated { req_id, contract_info } => {
                assert_eq!(req_id, 1);
                assert_eq!(contract_info, "265598@SMART");
            }
            other => panic!("expected DisplayGroupUpdated, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: FA decoder tests
    // ========================================================================

    #[test]
    fn decode_receive_fa_msg() {
        // RECEIVE_FA: msg_id=16, version=1, faDataType=1, xml="<groups/>"
        let data = make_fields(&["16", "1", "1", "<groups/>"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::ReceiveFa { fa_data_type, xml } => {
                assert_eq!(fa_data_type, 1);
                assert_eq!(xml, "<groups/>");
            }
            other => panic!("expected ReceiveFa, got {other:?}"),
        }
    }

    #[test]
    fn decode_replace_fa_end_msg() {
        // REPLACE_FA_END: msg_id=103, req_id=1, text="success"
        let data = make_fields(&["103", "1", "success"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::ReplaceFaEnd { req_id, text } => {
                assert_eq!(req_id, 1);
                assert_eq!(text, "success");
            }
            other => panic!("expected ReplaceFaEnd, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Reroute decoder tests
    // ========================================================================

    #[test]
    fn decode_reroute_mkt_data_req_msg() {
        // REROUTE_MKT_DATA_REQ: msg_id=91, req_id=1, con_id=265598, exchange="ISLAND"
        let data = make_fields(&["91", "1", "265598", "ISLAND"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::RerouteMktDataReq { req_id, con_id, exchange } => {
                assert_eq!(req_id, 1);
                assert_eq!(con_id, 265598);
                assert_eq!(exchange, "ISLAND");
            }
            other => panic!("expected RerouteMktDataReq, got {other:?}"),
        }
    }

    #[test]
    fn decode_reroute_mkt_depth_req_msg() {
        // REROUTE_MKT_DEPTH_REQ: msg_id=92, req_id=1, con_id=265598, exchange="ISLAND"
        let data = make_fields(&["92", "1", "265598", "ISLAND"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::RerouteMktDepthReq { req_id, con_id, exchange } => {
                assert_eq!(req_id, 1);
                assert_eq!(con_id, 265598);
                assert_eq!(exchange, "ISLAND");
            }
            other => panic!("expected RerouteMktDepthReq, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: Histogram decoder test
    // ========================================================================

    #[test]
    fn decode_histogram_data_msg() {
        // HISTOGRAM_DATA: msg_id=89, req_id=1, count=2,
        // entry1: price=150.0, size=1000, entry2: price=151.0, size=500
        let data = make_fields(&["89", "1", "2", "150.0", "1000", "151.0", "500"]);
        let event = super::decode_server_msg(&data, 176);
        match event {
            IBEvent::HistogramData { req_id, data: entries } => {
                assert_eq!(req_id, 1);
                assert_eq!(entries.len(), 2);
                assert!((entries[0].price - 150.0).abs() < 1e-10);
                assert!((entries[1].price - 151.0).abs() < 1e-10);
            }
            other => panic!("expected HistogramData, got {other:?}"),
        }
    }

    // ========================================================================
    // Phase 4: TryFrom<i32> enum tests
    // ========================================================================

    #[test]
    fn try_from_i32_enums() {
        use crate::models::enums::*;
        // Origin
        assert_eq!(Origin::try_from(0).unwrap(), Origin::Customer);
        assert_eq!(Origin::try_from(1).unwrap(), Origin::Firm);
        assert!(Origin::try_from(99).is_err());
        // AuctionStrategy
        assert_eq!(AuctionStrategy::try_from(0).unwrap(), AuctionStrategy::Unset);
        assert_eq!(AuctionStrategy::try_from(3).unwrap(), AuctionStrategy::Transparent);
        assert!(AuctionStrategy::try_from(99).is_err());
        // LegOpenClose
        assert_eq!(LegOpenClose::try_from(0).unwrap(), LegOpenClose::Same);
        assert_eq!(LegOpenClose::try_from(2).unwrap(), LegOpenClose::Close);
        assert!(LegOpenClose::try_from(99).is_err());
        // TriggerMethod
        assert_eq!(TriggerMethod::try_from(0).unwrap(), TriggerMethod::Default);
        assert_eq!(TriggerMethod::try_from(7).unwrap(), TriggerMethod::LastOrBidAsk);
        assert!(TriggerMethod::try_from(99).is_err());
        // OrderConditionType
        assert_eq!(OrderConditionType::try_from(1).unwrap(), OrderConditionType::Price);
        assert_eq!(OrderConditionType::try_from(6).unwrap(), OrderConditionType::Volume);
        assert!(OrderConditionType::try_from(0).is_err());
        // UsePriceMgmtAlgo
        assert_eq!(UsePriceMgmtAlgo::try_from(0).unwrap(), UsePriceMgmtAlgo::DontUse);
        assert_eq!(UsePriceMgmtAlgo::try_from(1).unwrap(), UsePriceMgmtAlgo::Use);
        assert!(UsePriceMgmtAlgo::try_from(99).is_err());
    }
}
