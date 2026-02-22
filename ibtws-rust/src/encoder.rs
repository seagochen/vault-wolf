//! IB TWS API message encoder.
//!
//! Encodes outgoing messages in the IB wire format: null-terminated ASCII fields
//! with a 4-byte big-endian length prefix (V100+ framing).
//!
//! Ported from: `EClient::EncodeField*`, `EClientSocket::prepareBuffer`,
//! `EClientSocket::closeAndSend`, `EClient::EncodeContract`.

use bytes::{BufMut, BytesMut};
use rust_decimal::Decimal;
use std::fmt;

use crate::errors::{IBApiError, Result};
use crate::models::common::TagValue;
use crate::models::contract::Contract;
use crate::protocol::{HEADER_LEN, MAX_MSG_LEN, server_version};

// ============================================================================
// Helpers
// ============================================================================

/// Check if a string contains only ASCII printable characters (32-126)
/// plus tab (9), LF (10), CR (13).
///
/// Mirrors C++ `EClient::isAsciiPrintable`.
fn is_ascii_printable(s: &str) -> bool {
    s.bytes()
        .all(|b| (32..127).contains(&b) || b == 9 || b == 10 || b == 13)
}

// ============================================================================
// MessageEncoder
// ============================================================================

/// Encodes IB API wire-format messages.
///
/// Each instance represents a single outgoing message being built.
/// Fields are encoded as ASCII text followed by a null byte (0x00).
///
/// Mirrors C++ `EClient::EncodeField`, `EncodeFieldMax`, `EncodeRawInt`,
/// `EClientSocket::prepareBuffer`, `EClientSocket::encodeMsgLen`.
pub struct MessageEncoder {
    buf: BytesMut,
    server_version: i32,
}

impl MessageEncoder {
    /// Create a new encoder for a single message.
    ///
    /// Reserves 4 bytes at the start for the length header (V100+ framing).
    /// Mirrors C++ `EClientSocket::prepareBuffer`.
    pub fn new(server_version: i32) -> Self {
        let mut buf = BytesMut::with_capacity(256);
        // Reserve space for 4-byte big-endian length prefix.
        buf.put_bytes(0, HEADER_LEN);
        Self {
            buf,
            server_version,
        }
    }

    pub fn server_version(&self) -> i32 {
        self.server_version
    }

    /// Finalize the message: compute length, write big-endian length header,
    /// return the complete framed message as bytes.
    ///
    /// Mirrors C++ `EClientSocket::encodeMsgLen` + `closeAndSend`.
    pub fn finalize(mut self) -> Result<BytesMut> {
        let msg_len = self.buf.len() - HEADER_LEN;
        if msg_len > MAX_MSG_LEN {
            return Err(IBApiError::Encoding(format!(
                "message too long: {msg_len} bytes (max {MAX_MSG_LEN})"
            )));
        }
        // Write big-endian length at offset 0.
        let len_bytes = (msg_len as u32).to_be_bytes();
        self.buf[0..HEADER_LEN].copy_from_slice(&len_bytes);
        Ok(self.buf)
    }

    // ========================================================================
    // Core field encoders
    // ========================================================================

    /// Encode a string field: bytes + '\0'.
    ///
    /// Validates ASCII printability for non-empty strings.
    /// Mirrors C++ `EncodeField<std::string>`.
    pub fn encode_field_str(&mut self, value: &str) -> &mut Self {
        if !value.is_empty() && !is_ascii_printable(value) {
            tracing::warn!(value, "non-ASCII-printable string in field encoding");
        }
        self.buf.extend_from_slice(value.as_bytes());
        self.buf.put_u8(0);
        self
    }

    /// Encode an i32 field: ASCII decimal + '\0'.
    ///
    /// Mirrors C++ `EncodeField<int>`.
    pub fn encode_field_i32(&mut self, value: i32) -> &mut Self {
        self.write_display(value);
        self.buf.put_u8(0);
        self
    }

    /// Encode an i64 field: ASCII decimal + '\0'.
    ///
    /// Mirrors C++ `EncodeField<long long>`.
    pub fn encode_field_i64(&mut self, value: i64) -> &mut Self {
        self.write_display(value);
        self.buf.put_u8(0);
        self
    }

    /// Encode a f64 field: decimal string or "Infinity" + '\0'.
    ///
    /// Mirrors C++ `EncodeField<double>` which uses `snprintf(str, 128, "%.14g", value)`.
    /// Rust's default f64 Display (ryu algorithm) produces round-trip representations
    /// that are compatible with `atof()` on the server side.
    pub fn encode_field_f64(&mut self, value: f64) -> &mut Self {
        if value.is_infinite() && value.is_sign_positive() {
            self.buf.extend_from_slice(b"Infinity");
        } else {
            self.write_display(value);
        }
        self.buf.put_u8(0);
        self
    }

    /// Encode a bool field: "1\0" for true, "0\0" for false.
    ///
    /// Mirrors C++ `EncodeField<bool>` which converts to int first.
    pub fn encode_field_bool(&mut self, value: bool) -> &mut Self {
        self.buf.extend_from_slice(if value { b"1" } else { b"0" });
        self.buf.put_u8(0);
        self
    }

    /// Encode a Decimal field: string representation + '\0'.
    ///
    /// Mirrors C++ `EncodeField<Decimal>`.
    pub fn encode_field_decimal(&mut self, value: &Decimal) -> &mut Self {
        self.write_display(value);
        self.buf.put_u8(0);
        self
    }

    // ========================================================================
    // "Max" encoders — Option<T> maps to C++ UNSET sentinel → empty string
    // ========================================================================

    /// Encode Option<i32>: None → "\0" (empty field), Some(v) → value.
    ///
    /// Mirrors C++ `EncodeFieldMax(int)` where INT_MAX → empty string.
    pub fn encode_field_max_i32(&mut self, value: Option<i32>) -> &mut Self {
        match value {
            Some(v) => self.encode_field_i32(v),
            None => {
                self.buf.put_u8(0);
                self
            }
        }
    }

    /// Encode Option<i64>: None → "\0" (empty field), Some(v) → value.
    pub fn encode_field_max_i64(&mut self, value: Option<i64>) -> &mut Self {
        match value {
            Some(v) => self.encode_field_i64(v),
            None => {
                self.buf.put_u8(0);
                self
            }
        }
    }

    /// Encode Option<f64>: None → "\0" (empty field), Some(v) → value.
    ///
    /// Mirrors C++ `EncodeFieldMax(double)` where DBL_MAX → empty string.
    pub fn encode_field_max_f64(&mut self, value: Option<f64>) -> &mut Self {
        match value {
            Some(v) => self.encode_field_f64(v),
            None => {
                self.buf.put_u8(0);
                self
            }
        }
    }

    /// Encode Option<Decimal>: None → "\0", Some(v) → value.
    pub fn encode_field_max_decimal(&mut self, value: Option<&Decimal>) -> &mut Self {
        match value {
            Some(v) => self.encode_field_decimal(v),
            None => {
                self.buf.put_u8(0);
                self
            }
        }
    }

    // ========================================================================
    // Raw integer encoder (4-byte big-endian, no null terminator)
    // ========================================================================

    /// Encode a raw 4-byte big-endian integer.
    ///
    /// Used for message IDs when `server_version >= MIN_SERVER_VER_PROTOBUF`.
    /// Mirrors C++ `EClient::EncodeRawInt`.
    pub fn encode_raw_int(&mut self, value: i32) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    // ========================================================================
    // Context-dependent message ID encoding
    // ========================================================================

    /// Encode a message ID: raw int for protobuf-capable servers, else text.
    ///
    /// Mirrors C++ `EClient::EncodeMsgId`.
    pub fn encode_msg_id(&mut self, msg_id: i32) -> &mut Self {
        if self.server_version >= server_version::PROTOBUF {
            self.encode_raw_int(msg_id)
        } else {
            self.encode_field_i32(msg_id)
        }
    }

    // ========================================================================
    // Display-based encoders (for enums)
    // ========================================================================

    /// Encode a type implementing Display: uses its Display output + '\0'.
    ///
    /// Works with all Phase 1 enums (SecType, OrderType, etc.) whose Display
    /// impls output the wire-format strings ("STK", "LMT", etc.).
    pub fn encode_field_display<T: fmt::Display>(&mut self, value: &T) -> &mut Self {
        self.write_display(value);
        self.buf.put_u8(0);
        self
    }

    /// Encode an optional Display type: None → empty string "\0".
    pub fn encode_field_opt_display<T: fmt::Display>(
        &mut self,
        value: Option<&T>,
    ) -> &mut Self {
        match value {
            Some(v) => self.encode_field_display(v),
            None => {
                self.buf.put_u8(0);
                self
            }
        }
    }

    // ========================================================================
    // Composite encoders
    // ========================================================================

    /// Encode a Contract struct.
    ///
    /// Mirrors C++ `EClient::EncodeContract`. Encodes the standard set of
    /// contract fields used in most request messages.
    pub fn encode_contract(&mut self, contract: &Contract) -> &mut Self {
        self.encode_field_i64(contract.con_id)
            .encode_field_str(&contract.symbol)
            .encode_field_opt_display(contract.sec_type.as_ref())
            .encode_field_str(&contract.last_trade_date_or_contract_month)
            .encode_field_max_f64(contract.strike)
            .encode_field_opt_display(contract.right.as_ref())
            .encode_field_str(&contract.multiplier)
            .encode_field_str(&contract.exchange)
            .encode_field_str(&contract.primary_exchange)
            .encode_field_str(&contract.currency)
            .encode_field_str(&contract.local_symbol)
            .encode_field_str(&contract.trading_class)
            .encode_field_bool(contract.include_expired)
    }

    /// Encode a TagValue list as `"key1=val1;key2=val2;\0"`.
    ///
    /// Mirrors C++ `EClient::EncodeTagValueList`.
    pub fn encode_tag_value_list(&mut self, tags: &[TagValue]) -> &mut Self {
        let mut combined = String::new();
        for tv in tags {
            combined.push_str(&tv.tag);
            combined.push('=');
            combined.push_str(&tv.value);
            combined.push(';');
        }
        self.encode_field_str(&combined)
    }

    // ========================================================================
    // Raw byte writing (for connect request special case)
    // ========================================================================

    /// Write raw bytes directly to the buffer (no null terminator).
    ///
    /// Used for the connect request version string which is not
    /// a null-terminated field.
    pub fn write_raw(&mut self, data: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(data);
        self
    }

    // ========================================================================
    // Private helpers
    // ========================================================================

    /// Write the Display representation of a value to the buffer.
    fn write_display<T: fmt::Display>(&mut self, value: T) {
        let s = value.to_string();
        self.buf.extend_from_slice(s.as_bytes());
    }
}

// ============================================================================
// Connect request builder (special case — not null-terminated fields)
// ============================================================================

/// Build the V100+ connection request bytes.
///
/// Wire format: `b"API\0"` + `[4-byte BE length]` + `b"v100..203[ connectOptions]"`.
///
/// The version string is NOT null-terminated (raw bytes in a length-prefixed frame).
/// Mirrors C++ `EClient::sendConnectRequest`.
pub fn build_connect_request(connect_options: Option<&str>) -> Result<BytesMut> {
    use crate::protocol::{API_SIGN, MAX_CLIENT_VER, MIN_CLIENT_VER};

    let body = if MIN_CLIENT_VER < MAX_CLIENT_VER {
        format!("v{MIN_CLIENT_VER}..{MAX_CLIENT_VER}")
    } else {
        format!("v{MIN_CLIENT_VER}")
    };

    let body = match connect_options {
        Some(opts) if !opts.is_empty() => format!("{body} {opts}"),
        _ => body,
    };

    let body_bytes = body.as_bytes();
    let body_len = body_bytes.len();
    if body_len > MAX_MSG_LEN {
        return Err(IBApiError::Encoding(
            "connect request too long".into(),
        ));
    }

    let mut buf = BytesMut::with_capacity(API_SIGN.len() + HEADER_LEN + body_len);
    buf.extend_from_slice(API_SIGN);
    buf.extend_from_slice(&(body_len as u32).to_be_bytes());
    buf.extend_from_slice(body_bytes);
    Ok(buf)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create an encoder and extract the message body (skip 4-byte header).
    fn encode_body(f: impl FnOnce(&mut MessageEncoder)) -> Vec<u8> {
        let mut enc = MessageEncoder::new(150); // non-protobuf server version
        f(&mut enc);
        let buf = enc.finalize().unwrap();
        buf[HEADER_LEN..].to_vec()
    }

    #[test]
    fn encode_field_i32_basic() {
        let body = encode_body(|enc| {
            enc.encode_field_i32(42);
        });
        assert_eq!(body, b"42\0");
    }

    #[test]
    fn encode_field_i32_negative() {
        let body = encode_body(|enc| {
            enc.encode_field_i32(-7);
        });
        assert_eq!(body, b"-7\0");
    }

    #[test]
    fn encode_field_i64_basic() {
        let body = encode_body(|enc| {
            enc.encode_field_i64(1234567890123);
        });
        assert_eq!(body, b"1234567890123\0");
    }

    #[test]
    fn encode_field_bool_true() {
        let body = encode_body(|enc| {
            enc.encode_field_bool(true);
        });
        assert_eq!(body, b"1\0");
    }

    #[test]
    fn encode_field_bool_false() {
        let body = encode_body(|enc| {
            enc.encode_field_bool(false);
        });
        assert_eq!(body, b"0\0");
    }

    #[test]
    fn encode_field_f64_normal() {
        let body = encode_body(|enc| {
            enc.encode_field_f64(1.23);
        });
        let s = std::str::from_utf8(&body[..body.len() - 1]).unwrap();
        let parsed: f64 = s.parse().unwrap();
        assert!((parsed - 1.23).abs() < 1e-10);
    }

    #[test]
    fn encode_field_f64_infinity() {
        let body = encode_body(|enc| {
            enc.encode_field_f64(f64::INFINITY);
        });
        assert_eq!(body, b"Infinity\0");
    }

    #[test]
    fn encode_field_f64_integer() {
        let body = encode_body(|enc| {
            enc.encode_field_f64(100.0);
        });
        // Should produce "100" or "100.0" followed by \0
        let s = std::str::from_utf8(&body[..body.len() - 1]).unwrap();
        let parsed: f64 = s.parse().unwrap();
        assert_eq!(parsed, 100.0);
    }

    #[test]
    fn encode_field_str_basic() {
        let body = encode_body(|enc| {
            enc.encode_field_str("AAPL");
        });
        assert_eq!(body, b"AAPL\0");
    }

    #[test]
    fn encode_field_str_empty() {
        let body = encode_body(|enc| {
            enc.encode_field_str("");
        });
        assert_eq!(body, b"\0");
    }

    #[test]
    fn encode_field_str_with_tabs() {
        // Tab (9) is allowed.
        let body = encode_body(|enc| {
            enc.encode_field_str("hello\tworld");
        });
        assert_eq!(body, b"hello\tworld\0");
    }

    #[test]
    fn encode_field_decimal() {
        let d = Decimal::new(12345, 2); // 123.45
        let body = encode_body(|enc| {
            enc.encode_field_decimal(&d);
        });
        assert_eq!(body, b"123.45\0");
    }

    #[test]
    fn encode_field_max_i32_none() {
        let body = encode_body(|enc| {
            enc.encode_field_max_i32(None);
        });
        assert_eq!(body, b"\0");
    }

    #[test]
    fn encode_field_max_i32_some() {
        let body = encode_body(|enc| {
            enc.encode_field_max_i32(Some(42));
        });
        assert_eq!(body, b"42\0");
    }

    #[test]
    fn encode_field_max_f64_none() {
        let body = encode_body(|enc| {
            enc.encode_field_max_f64(None);
        });
        assert_eq!(body, b"\0");
    }

    #[test]
    fn encode_field_max_f64_some() {
        let body = encode_body(|enc| {
            enc.encode_field_max_f64(Some(1.5));
        });
        let s = std::str::from_utf8(&body[..body.len() - 1]).unwrap();
        let parsed: f64 = s.parse().unwrap();
        assert!((parsed - 1.5).abs() < 1e-10);
    }

    #[test]
    fn encode_raw_int() {
        let body = encode_body(|enc| {
            enc.encode_raw_int(42);
        });
        assert_eq!(body, &42_i32.to_be_bytes());
    }

    #[test]
    fn encode_raw_int_large() {
        let body = encode_body(|enc| {
            enc.encode_raw_int(0x01020304);
        });
        assert_eq!(body, &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn encode_msg_id_text_mode() {
        // server_version=150 (< PROTOBUF=201): text encoding.
        let body = encode_body(|enc| {
            enc.encode_msg_id(3);
        });
        assert_eq!(body, b"3\0");
    }

    #[test]
    fn encode_msg_id_raw_mode() {
        // server_version >= 201: raw int encoding.
        let mut enc = MessageEncoder::new(201);
        enc.encode_msg_id(3);
        let buf = enc.finalize().unwrap();
        let body = &buf[HEADER_LEN..];
        assert_eq!(body, &3_i32.to_be_bytes());
    }

    #[test]
    fn encode_tag_value_list_basic() {
        let tags = vec![
            TagValue::new("algo", "VWAP"),
            TagValue::new("limit", "100"),
        ];
        let body = encode_body(|enc| {
            enc.encode_tag_value_list(&tags);
        });
        assert_eq!(body, b"algo=VWAP;limit=100;\0");
    }

    #[test]
    fn encode_tag_value_list_empty() {
        let body = encode_body(|enc| {
            enc.encode_tag_value_list(&[]);
        });
        assert_eq!(body, b"\0");
    }

    #[test]
    fn encode_contract_basic() {
        use crate::models::enums::SecType;

        let contract = Contract {
            con_id: 265598,
            symbol: "AAPL".to_string(),
            sec_type: Some(SecType::Stock),
            currency: "USD".to_string(),
            exchange: "SMART".to_string(),
            ..Default::default()
        };
        let body = encode_body(|enc| {
            enc.encode_contract(&contract);
        });
        // Verify the field sequence: conId, symbol, secType, lastTradeDate, strike,
        // right, multiplier, exchange, primaryExchange, currency, localSymbol,
        // tradingClass, includeExpired
        let fields: Vec<&[u8]> = body.split(|&b| b == 0).collect();
        // Last element is empty because body ends with \0.
        assert_eq!(fields[0], b"265598");    // conId
        assert_eq!(fields[1], b"AAPL");      // symbol
        assert_eq!(fields[2], b"STK");       // secType
        assert_eq!(fields[3], b"");          // lastTradeDateOrContractMonth
        assert_eq!(fields[4], b"");          // strike (None)
        assert_eq!(fields[5], b"");          // right (None)
        assert_eq!(fields[6], b"");          // multiplier
        assert_eq!(fields[7], b"SMART");     // exchange
        assert_eq!(fields[8], b"");          // primaryExchange
        assert_eq!(fields[9], b"USD");       // currency
        assert_eq!(fields[10], b"");         // localSymbol
        assert_eq!(fields[11], b"");         // tradingClass
        assert_eq!(fields[12], b"0");        // includeExpired = false
    }

    #[test]
    fn finalize_message_length() {
        let mut enc = MessageEncoder::new(150);
        enc.encode_field_i32(1);   // "1\0" = 2 bytes
        enc.encode_field_str("hi"); // "hi\0" = 3 bytes
        let buf = enc.finalize().unwrap();

        // Total body = 5 bytes
        let len_bytes: [u8; 4] = buf[..4].try_into().unwrap();
        let msg_len = u32::from_be_bytes(len_bytes);
        assert_eq!(msg_len, 5);
        assert_eq!(buf.len(), HEADER_LEN + 5);
    }

    #[test]
    fn build_connect_request_basic() {
        let buf = build_connect_request(None).unwrap();
        // Starts with "API\0"
        assert_eq!(&buf[..4], b"API\0");
        // Then 4-byte length
        let len_bytes: [u8; 4] = buf[4..8].try_into().unwrap();
        let body_len = u32::from_be_bytes(len_bytes) as usize;
        // Then body
        let body = &buf[8..];
        assert_eq!(body.len(), body_len);
        let body_str = std::str::from_utf8(body).unwrap();
        assert!(body_str.starts_with("v100.."));
    }

    #[test]
    fn build_connect_request_with_options() {
        let buf = build_connect_request(Some("key=val")).unwrap();
        let body = &buf[8..];
        let body_str = std::str::from_utf8(body).unwrap();
        assert!(body_str.contains("key=val"));
        assert!(body_str.contains(' '));
    }

    #[test]
    fn ascii_printable_validation() {
        assert!(is_ascii_printable("hello world"));
        assert!(is_ascii_printable("hello\tworld")); // tab allowed
        assert!(is_ascii_printable("line1\nline2")); // LF allowed
        assert!(is_ascii_printable(""));
        assert!(!is_ascii_printable("hello\x01world")); // control char
        assert!(!is_ascii_printable("hello\x7Fworld")); // DEL
    }

    #[test]
    fn method_chaining() {
        let mut enc = MessageEncoder::new(150);
        enc.encode_field_i32(1)
            .encode_field_str("AAPL")
            .encode_field_bool(true)
            .encode_field_max_f64(None);
        let buf = enc.finalize().unwrap();
        let body = &buf[HEADER_LEN..];
        assert_eq!(body, b"1\x00AAPL\x001\x00\x00");
    }
}
