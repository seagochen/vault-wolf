//! IB TWS API client.
//!
//! `IBClient` is the main entry point for interacting with IB TWS/Gateway.
//! It manages the connection lifecycle, sends request messages, and provides
//! an event channel for receiving server responses.
//!
//! Replaces C++ `EClient` + `EClientSocket` + `EReader` with a single async
//! struct backed by tokio.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::encoder::MessageEncoder;
use crate::errors::{IBApiError, Result};
use crate::models::common::TagValue;
use crate::models::contract::Contract;
use crate::models::execution::ExecutionFilter;
use crate::models::order::{Order, OrderCancel, OrderCondition};
use crate::models::scanner::ScannerSubscription;
use crate::protocol::{outgoing, server_version};
use crate::reader::MessageReader;
use crate::transport::{Transport, TransportWriter};
use crate::wrapper::IBEvent;

// ============================================================================
// IBClient
// ============================================================================

/// Async IB TWS API client.
///
/// Manages a single connection to TWS/Gateway. After calling `connect()`,
/// the client spawns a background reader task that decodes incoming messages
/// and sends them as `IBEvent`s through an mpsc channel.
///
/// ## Usage
///
/// ```rust,ignore
/// let (mut client, mut rx) = IBClient::connect("127.0.0.1", 4002, 0, None).await?;
///
/// // Send requests
/// client.req_current_time().await?;
///
/// // Receive events
/// while let Some(event) = rx.recv().await {
///     match event {
///         IBEvent::CurrentTime { time } => println!("Server time: {time}"),
///         IBEvent::Error { code, message, .. } => eprintln!("Error {code}: {message}"),
///         _ => {}
///     }
/// }
/// ```
pub struct IBClient {
    writer: TransportWriter,
    server_version: i32,
    tws_time: String,
    client_id: i32,
    next_req_id: AtomicI32,
    connected: AtomicBool,
    reader_handle: Option<JoinHandle<()>>,
}

impl IBClient {
    /// Connect to TWS/Gateway, perform handshake, send START_API, and spawn
    /// the background reader task.
    ///
    /// Returns the client and an unbounded event receiver. The client is used
    /// to send requests; the receiver delivers server responses as `IBEvent`s.
    ///
    /// The first events received are typically `NextValidId` and `ManagedAccounts`.
    pub async fn connect(
        host: &str,
        port: u16,
        client_id: i32,
        optional_capabilities: Option<&str>,
    ) -> Result<(Self, mpsc::UnboundedReceiver<IBEvent>)> {
        // 1. TCP connect + V100+ handshake
        let mut transport = Transport::connect(host, port, None).await?;
        let server_version = transport.server_version();
        let tws_time = transport.tws_time().to_string();

        tracing::info!(
            server_version,
            client_id,
            "IBClient connecting"
        );

        // 2. Send START_API (must happen before splitting, or we build it manually)
        transport
            .start_api(client_id, optional_capabilities)
            .await?;

        // 3. Split transport into reader/writer halves
        let (transport_reader, transport_writer) = transport.into_split();

        // 4. Spawn the reader task
        let reader = MessageReader::new(transport_reader, server_version);
        let (rx, reader_handle) = reader.spawn();

        let client = Self {
            writer: transport_writer,
            server_version,
            tws_time,
            client_id,
            next_req_id: AtomicI32::new(1),
            connected: AtomicBool::new(true),
            reader_handle: Some(reader_handle),
        };

        Ok((client, rx))
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Negotiated server version.
    pub fn server_version(&self) -> i32 {
        self.server_version
    }

    /// TWS connection time string from handshake.
    pub fn tws_time(&self) -> &str {
        &self.tws_time
    }

    /// Client ID used for this connection.
    pub fn client_id(&self) -> i32 {
        self.client_id
    }

    /// Whether the client is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Get the next request ID (atomic increment).
    ///
    /// Request IDs are used to correlate requests with their responses.
    pub fn next_req_id(&self) -> i32 {
        self.next_req_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Set the base for the next request ID.
    ///
    /// Typically called after receiving `NextValidId` to synchronize with
    /// the server's order ID sequence.
    pub fn set_next_req_id(&self, id: i32) {
        self.next_req_id.store(id, Ordering::Relaxed);
    }

    // ========================================================================
    // Connection Management
    // ========================================================================

    /// Disconnect from the server.
    ///
    /// Shuts down the write half of the TCP connection, which causes the
    /// reader task to receive an EOF and exit cleanly.
    pub async fn disconnect(&mut self) {
        if !self.connected.swap(false, Ordering::Relaxed) {
            return; // Already disconnected
        }

        tracing::info!("IBClient disconnecting");

        // Shut down writer — this triggers EOF on the server side,
        // and the reader task will exit when the server closes its end.
        self.writer.shutdown().await;

        // Wait for reader task to finish
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.await;
        }
    }

    // ========================================================================
    // Message Sending (low-level)
    // ========================================================================

    /// Send a pre-encoded framed message to the server.
    ///
    /// The data should already include the 4-byte length header
    /// (as produced by `MessageEncoder::finalize()`).
    pub async fn send_raw(&mut self, data: &[u8]) -> Result<()> {
        if !self.is_connected() {
            return Err(IBApiError::Connection(
                "not connected".into(),
            ));
        }
        self.writer.send_message(data).await
    }

    /// Create a new `MessageEncoder` configured with the server version.
    pub fn encoder(&self) -> MessageEncoder {
        MessageEncoder::new(self.server_version)
    }

    // ========================================================================
    // Private Helpers
    // ========================================================================

    fn check_server_version(&self, min_version: i32, feature: &str) -> Result<()> {
        if self.server_version < min_version {
            return Err(IBApiError::Encoding(format!(
                "{feature} requires server version >= {min_version} (current: {})",
                self.server_version
            )));
        }
        Ok(())
    }

    async fn send_encoded(&mut self, enc: MessageEncoder) -> Result<()> {
        let bytes = enc.finalize()?;
        self.send_raw(&bytes).await
    }

    // ========================================================================
    // Utility Requests
    // ========================================================================

    /// Request the current server time.
    /// Response: `IBEvent::CurrentTime`.
    pub async fn req_current_time(&mut self) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_CURRENT_TIME);
        enc.encode_field_i32(1); // version
        self.send_encoded(enc).await
    }

    /// Request the current server time in milliseconds.
    /// Response: `IBEvent::CurrentTimeInMillis`.
    pub async fn req_current_time_in_millis(&mut self) -> Result<()> {
        self.check_server_version(server_version::CURRENT_TIME_IN_MILLIS, "req_current_time_in_millis")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_CURRENT_TIME_IN_MILLIS);
        self.send_encoded(enc).await
    }

    /// Request the next valid order ID.
    /// Response: `IBEvent::NextValidId`.
    pub async fn req_ids(&mut self) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_IDS);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(1); // numIds
        self.send_encoded(enc).await
    }

    /// Set the server log level.
    pub async fn set_server_log_level(&mut self, log_level: i32) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::SET_SERVER_LOGLEVEL);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(log_level);
        self.send_encoded(enc).await
    }

    /// Request managed accounts list.
    /// Response: `IBEvent::ManagedAccounts`.
    pub async fn req_managed_accts(&mut self) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_MANAGED_ACCTS);
        enc.encode_field_i32(1); // version
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Market Data Requests
    // ========================================================================

    /// Request real-time market data.
    /// Response: `IBEvent::TickPrice`, `TickSize`, `TickString`, etc.
    pub async fn req_mkt_data(
        &mut self,
        ticker_id: i32,
        contract: &Contract,
        generic_ticks: &str,
        snapshot: bool,
        regulatory_snapshot: bool,
        mkt_data_options: &[TagValue],
    ) -> Result<()> {
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_MKT_DATA);
        enc.encode_field_i32(11); // version
        enc.encode_field_i32(ticker_id);

        // Contract fields
        if sv >= server_version::REQ_MKT_DATA_CONID {
            enc.encode_field_i64(contract.con_id);
        }
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);
        enc.encode_field_str(&contract.exchange);
        enc.encode_field_str(&contract.primary_exchange);
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_str(&contract.trading_class);
        }

        // Combo legs for BAG
        if contract.sec_type.as_ref().map(|s| s.to_string()).as_deref() == Some("BAG") {
            if let Some(ref legs) = contract.combo_legs {
                enc.encode_field_i32(legs.len() as i32);
                for leg in legs {
                    enc.encode_field_i64(leg.con_id);
                    enc.encode_field_i64(leg.ratio);
                    enc.encode_field_opt_display(leg.action.as_ref());
                    enc.encode_field_str(&leg.exchange);
                }
            } else {
                enc.encode_field_i32(0);
            }
        }

        // Delta neutral contract
        if sv >= server_version::DELTA_NEUTRAL {
            if let Some(ref dnc) = contract.delta_neutral_contract {
                enc.encode_field_bool(true);
                enc.encode_field_i64(dnc.con_id);
                enc.encode_field_f64(dnc.delta);
                enc.encode_field_f64(dnc.price);
            } else {
                enc.encode_field_bool(false);
            }
        }

        enc.encode_field_str(generic_ticks);
        enc.encode_field_bool(snapshot);

        if sv >= server_version::REQ_SMART_COMPONENTS {
            enc.encode_field_bool(regulatory_snapshot);
        }

        if sv >= server_version::LINKING {
            enc.encode_tag_value_list(mkt_data_options);
        }
        self.send_encoded(enc).await
    }

    /// Cancel market data subscription.
    pub async fn cancel_mkt_data(&mut self, ticker_id: i32) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_MKT_DATA);
        enc.encode_field_i32(2); // version
        enc.encode_field_i32(ticker_id);
        self.send_encoded(enc).await
    }

    /// Request market depth (Level II data).
    pub async fn req_mkt_depth(
        &mut self,
        ticker_id: i32,
        contract: &Contract,
        num_rows: i32,
        is_smart_depth: bool,
        mkt_depth_options: &[TagValue],
    ) -> Result<()> {
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_MKT_DEPTH);
        enc.encode_field_i32(5); // version
        enc.encode_field_i32(ticker_id);

        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_i64(contract.con_id);
        }
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);
        enc.encode_field_str(&contract.exchange);
        if sv >= server_version::MKT_DEPTH_PRIM_EXCHANGE {
            enc.encode_field_str(&contract.primary_exchange);
        }
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_str(&contract.trading_class);
        }
        enc.encode_field_i32(num_rows);
        if sv >= server_version::SMART_DEPTH {
            enc.encode_field_bool(is_smart_depth);
        }
        if sv >= server_version::LINKING {
            enc.encode_tag_value_list(mkt_depth_options);
        }
        self.send_encoded(enc).await
    }

    /// Cancel market depth subscription.
    pub async fn cancel_mkt_depth(&mut self, ticker_id: i32, is_smart_depth: bool) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_MKT_DEPTH);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(ticker_id);
        if self.server_version >= server_version::SMART_DEPTH {
            enc.encode_field_bool(is_smart_depth);
        }
        self.send_encoded(enc).await
    }

    /// Set market data type (real-time, frozen, delayed, delayed-frozen).
    pub async fn req_market_data_type(&mut self, market_data_type: i32) -> Result<()> {
        self.check_server_version(server_version::REQ_MARKET_DATA_TYPE, "req_market_data_type")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_MARKET_DATA_TYPE);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(market_data_type);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Order Requests
    // ========================================================================

    /// Place an order.
    /// Response: `IBEvent::OpenOrder`, `IBEvent::OrderStatus`.
    #[allow(clippy::too_many_lines)]
    pub async fn place_order(
        &mut self,
        id: i64,
        contract: &Contract,
        order: &Order,
    ) -> Result<()> {
        let sv = self.server_version;

        // Protobuf path for sv >= 203
        if sv >= server_version::PROTOBUF_PLACE_ORDER {
            return self.place_order_protobuf(id, contract, order).await;
        }

        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::PLACE_ORDER);

        // Version (only for older servers)
        if sv < server_version::ORDER_CONTAINER {
            let version = if sv < server_version::NOT_HELD { 27 } else { 45 };
            enc.encode_field_i32(version);
        }

        enc.encode_field_i64(id);

        // Contract
        if sv >= server_version::PLACE_ORDER_CONID {
            enc.encode_field_i64(contract.con_id);
        }
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);
        enc.encode_field_str(&contract.exchange);
        enc.encode_field_str(&contract.primary_exchange);
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_str(&contract.trading_class);
        }
        if sv >= server_version::SEC_ID_TYPE {
            enc.encode_field_opt_display(contract.sec_id_type.as_ref());
            enc.encode_field_str(&contract.sec_id);
        }

        // Order fields
        enc.encode_field_opt_display(order.action.as_ref());
        if sv >= server_version::FRACTIONAL_POSITIONS {
            enc.encode_field_max_decimal(order.total_quantity.as_ref());
        } else {
            let qty = order.total_quantity
                .map(|d| d.trunc().to_string().parse::<i64>().unwrap_or(0))
                .unwrap_or(0);
            enc.encode_field_i64(qty);
        }
        enc.encode_field_opt_display(order.order_type.as_ref());

        if sv >= server_version::ORDER_COMBO_LEGS_PRICE {
            enc.encode_field_max_f64(order.lmt_price);
        } else {
            enc.encode_field_f64(order.lmt_price.unwrap_or(0.0));
        }
        if sv >= server_version::TRAILING_PERCENT {
            enc.encode_field_max_f64(order.aux_price);
        } else {
            enc.encode_field_f64(order.aux_price.unwrap_or(0.0));
        }

        // TIF, OCA, Account, etc.
        enc.encode_field_opt_display(order.tif.as_ref());
        enc.encode_field_str(&order.oca_group);
        enc.encode_field_str(&order.account);
        enc.encode_field_str(&order.open_close);
        enc.encode_field_i32(order.origin as i32);
        enc.encode_field_str(&order.order_ref);
        enc.encode_field_bool(order.transmit);
        enc.encode_field_i64(order.parent_id);

        enc.encode_field_bool(order.block_order);
        enc.encode_field_bool(order.sweep_to_fill);
        enc.encode_field_i32(order.display_size);
        enc.encode_field_i32(order.trigger_method);
        enc.encode_field_bool(order.outside_rth);
        enc.encode_field_bool(order.hidden);

        // Combo legs for BAG
        if contract.sec_type.as_ref().map(|s| s.to_string()).as_deref() == Some("BAG") {
            if let Some(ref legs) = contract.combo_legs {
                enc.encode_field_i32(legs.len() as i32);
                for leg in legs {
                    enc.encode_field_i64(leg.con_id);
                    enc.encode_field_i64(leg.ratio);
                    enc.encode_field_opt_display(leg.action.as_ref());
                    enc.encode_field_str(&leg.exchange);
                    enc.encode_field_i32(leg.open_close as i32);
                    enc.encode_field_i32(leg.short_sale_slot);
                    enc.encode_field_str(&leg.designated_location);
                    if sv >= server_version::SSHORTX_OLD {
                        enc.encode_field_i32(leg.exempt_code);
                    }
                }
            } else {
                enc.encode_field_i32(0);
            }

            // Order combo legs
            if sv >= server_version::ORDER_COMBO_LEGS_PRICE {
                if let Some(ref ocl) = order.order_combo_legs {
                    enc.encode_field_i32(ocl.len() as i32);
                    for leg in ocl {
                        enc.encode_field_max_f64(leg.price);
                    }
                } else {
                    enc.encode_field_i32(0);
                }
            }

            // Smart combo routing params
            if sv >= server_version::SMART_COMBO_ROUTING_PARAMS {
                if let Some(ref params) = order.smart_combo_routing_params {
                    enc.encode_field_i32(params.len() as i32);
                    for tv in params {
                        enc.encode_field_str(&tv.tag);
                        enc.encode_field_str(&tv.value);
                    }
                } else {
                    enc.encode_field_i32(0);
                }
            }
        }

        enc.encode_field_str(""); // deprecated sharesAllocation
        enc.encode_field_f64(order.discretionary_amt);
        enc.encode_field_str(&order.good_after_time);
        enc.encode_field_str(&order.good_till_date);

        enc.encode_field_str(&order.fa_group);
        enc.encode_field_str(&order.fa_method);
        enc.encode_field_str(&order.fa_percentage);

        if sv < server_version::FA_PROFILE_DESUPPORT {
            enc.encode_field_str(""); // deprecated faProfile
        }

        if sv >= server_version::MODELS_SUPPORT {
            enc.encode_field_str(&order.model_code);
        }

        enc.encode_field_i32(order.short_sale_slot);
        enc.encode_field_str(&order.designated_location);
        if sv >= server_version::SSHORTX_OLD {
            enc.encode_field_i32(order.exempt_code);
        }

        enc.encode_field_i32(order.oca_type);
        enc.encode_field_str(&order.rule_80a);
        enc.encode_field_str(&order.settling_firm);
        enc.encode_field_bool(order.all_or_none);
        enc.encode_field_max_i32(order.min_qty);
        enc.encode_field_max_f64(order.percent_offset);

        // Deprecated fields (must still be sent)
        enc.encode_field_bool(false); // eTradeOnly
        enc.encode_field_bool(false); // firmQuoteOnly
        enc.encode_field_max_f64(None); // nbboPriceCap

        enc.encode_field_i32(order.auction_strategy as i32);
        enc.encode_field_max_f64(order.starting_price);
        enc.encode_field_max_f64(order.stock_ref_price);
        enc.encode_field_max_f64(order.delta);
        enc.encode_field_max_f64(order.stock_range_lower);
        enc.encode_field_max_f64(order.stock_range_upper);

        enc.encode_field_bool(order.override_percentage_constraints);

        // Volatility orders
        enc.encode_field_max_f64(order.volatility);
        enc.encode_field_max_i32(order.volatility_type);
        enc.encode_field_str(&order.delta_neutral_order_type);
        enc.encode_field_max_f64(order.delta_neutral_aux_price);

        if sv >= server_version::DELTA_NEUTRAL_CONID
            && !order.delta_neutral_order_type.is_empty()
        {
            enc.encode_field_i64(order.delta_neutral_con_id);
            enc.encode_field_str(&order.delta_neutral_settling_firm);
            enc.encode_field_str(&order.delta_neutral_clearing_account);
            enc.encode_field_str(&order.delta_neutral_clearing_intent);
        }
        if sv >= server_version::DELTA_NEUTRAL_OPEN_CLOSE
            && !order.delta_neutral_order_type.is_empty()
        {
            enc.encode_field_str(&order.delta_neutral_open_close);
            enc.encode_field_bool(order.delta_neutral_short_sale);
            enc.encode_field_i32(order.delta_neutral_short_sale_slot);
            enc.encode_field_str(&order.delta_neutral_designated_location);
        }

        enc.encode_field_bool(order.continuous_update);
        enc.encode_field_max_i32(order.reference_price_type);

        enc.encode_field_max_f64(order.trail_stop_price);
        if sv >= server_version::TRAILING_PERCENT {
            enc.encode_field_max_f64(order.trailing_percent);
        }

        // Scale orders
        if sv >= server_version::SCALE_ORDERS2 {
            enc.encode_field_max_i32(order.scale_init_level_size);
            enc.encode_field_max_i32(order.scale_subs_level_size);
        }
        enc.encode_field_max_f64(order.scale_price_increment);

        if sv >= server_version::SCALE_ORDERS3 {
            if let Some(incr) = order.scale_price_increment {
                if incr > 0.0 {
                    enc.encode_field_max_f64(order.scale_price_adjust_value);
                    enc.encode_field_max_i32(order.scale_price_adjust_interval);
                    enc.encode_field_max_f64(order.scale_profit_offset);
                    enc.encode_field_bool(order.scale_auto_reset);
                    enc.encode_field_max_i32(order.scale_init_position);
                    enc.encode_field_max_i32(order.scale_init_fill_qty);
                    enc.encode_field_bool(order.scale_random_percent);
                }
            }
        }

        if sv >= server_version::SCALE_TABLE {
            enc.encode_field_str(&order.scale_table);
            enc.encode_field_str(&order.active_start_time);
            enc.encode_field_str(&order.active_stop_time);
        }

        // Hedge orders
        if sv >= server_version::HEDGE_ORDERS {
            enc.encode_field_str(&order.hedge_type);
            if !order.hedge_type.is_empty() {
                enc.encode_field_str(&order.hedge_param);
            }
        }

        if sv >= server_version::OPT_OUT_SMART_ROUTING {
            enc.encode_field_bool(order.opt_out_smart_routing);
        }

        if sv >= server_version::PTA_ORDERS {
            enc.encode_field_str(&order.clearing_account);
            enc.encode_field_str(&order.clearing_intent);
        }

        if sv >= server_version::NOT_HELD {
            enc.encode_field_bool(order.not_held);
        }

        // Delta neutral contract
        if sv >= server_version::DELTA_NEUTRAL {
            if let Some(ref dnc) = contract.delta_neutral_contract {
                enc.encode_field_bool(true);
                enc.encode_field_i64(dnc.con_id);
                enc.encode_field_f64(dnc.delta);
                enc.encode_field_f64(dnc.price);
            } else {
                enc.encode_field_bool(false);
            }
        }

        // Algo orders
        if sv >= server_version::ALGO_ORDERS {
            enc.encode_field_str(&order.algo_strategy);
            if !order.algo_strategy.is_empty() {
                if let Some(ref params) = order.algo_params {
                    enc.encode_field_i32(params.len() as i32);
                    for tv in params {
                        enc.encode_field_str(&tv.tag);
                        enc.encode_field_str(&tv.value);
                    }
                } else {
                    enc.encode_field_i32(0);
                }
            }
        }

        if sv >= server_version::ALGO_ID {
            enc.encode_field_str(&order.algo_id);
        }

        enc.encode_field_bool(order.what_if);

        if sv >= server_version::LINKING {
            enc.encode_tag_value_list(
                order.order_misc_options.as_deref().unwrap_or(&[]),
            );
        }

        if sv >= server_version::ORDER_SOLICITED {
            enc.encode_field_bool(order.solicited);
        }

        if sv >= server_version::RANDOMIZE_SIZE_AND_PRICE {
            enc.encode_field_bool(order.randomize_size);
            enc.encode_field_bool(order.randomize_price);
        }

        // Pegged to benchmark
        if sv >= server_version::PEGGED_TO_BENCHMARK {
            let order_type_str = order.order_type.as_ref().map(|t| t.to_string()).unwrap_or_default();
            let is_peg_bench = order_type_str == "PEG BENCH";

            if is_peg_bench {
                enc.encode_field_max_i32(order.reference_contract_id);
                enc.encode_field_bool(order.is_pegged_change_amount_decrease);
                enc.encode_field_max_f64(order.pegged_change_amount);
                enc.encode_field_max_f64(order.reference_change_amount);
                enc.encode_field_str(&order.reference_exchange_id);
            }

            // Conditions
            enc.encode_field_i32(order.conditions.len() as i32);
            if !order.conditions.is_empty() {
                for cond in &order.conditions {
                    encode_condition(&mut enc, cond);
                }
                enc.encode_field_bool(order.conditions_ignore_rth);
                enc.encode_field_bool(order.conditions_cancel_order);
            }

            // Adjusted order fields
            enc.encode_field_str(&order.adjusted_order_type);
            enc.encode_field_max_f64(order.trigger_price);
            enc.encode_field_max_f64(order.lmt_price_offset);
            enc.encode_field_max_f64(order.adjusted_stop_price);
            enc.encode_field_max_f64(order.adjusted_stop_limit_price);
            enc.encode_field_max_f64(order.adjusted_trailing_amount);
            enc.encode_field_max_i32(order.adjustable_trailing_unit);
        }

        if sv >= server_version::EXT_OPERATOR {
            enc.encode_field_str(&order.ext_operator);
        }

        if sv >= server_version::SOFT_DOLLAR_TIER {
            enc.encode_field_str(&order.soft_dollar_tier.name);
            enc.encode_field_str(&order.soft_dollar_tier.val);
        }

        if sv >= server_version::CASH_QTY {
            enc.encode_field_max_f64(order.cash_qty);
        }

        if sv >= server_version::DECISION_MAKER {
            enc.encode_field_str(&order.mifid2_decision_maker);
            enc.encode_field_str(&order.mifid2_decision_algo);
        }
        if sv >= server_version::MIFID_EXECUTION {
            enc.encode_field_str(&order.mifid2_execution_trader);
            enc.encode_field_str(&order.mifid2_execution_algo);
        }

        if sv >= server_version::AUTO_PRICE_FOR_HEDGE {
            enc.encode_field_bool(order.dont_use_auto_price_for_hedge);
        }

        if sv >= server_version::ORDER_CONTAINER {
            enc.encode_field_bool(order.is_oms_container);
        }

        if sv >= server_version::D_PEG_ORDERS {
            enc.encode_field_bool(order.discretionary_up_to_limit_price);
        }

        if sv >= server_version::PRICE_MGMT_ALGO {
            let use_price = match order.use_price_mgmt_algo {
                crate::models::enums::UsePriceMgmtAlgo::Default => None,
                v => Some(v as i32),
            };
            enc.encode_field_max_i32(use_price);
        }

        if sv >= server_version::DURATION {
            enc.encode_field_max_i32(order.duration);
        }
        if sv >= server_version::POST_TO_ATS {
            enc.encode_field_max_i32(order.post_to_ats);
        }
        if sv >= server_version::AUTO_CANCEL_PARENT {
            enc.encode_field_bool(order.auto_cancel_parent);
        }
        if sv >= server_version::ADVANCED_ORDER_REJECT {
            enc.encode_field_str(&order.advanced_error_override);
        }
        if sv >= server_version::MANUAL_ORDER_TIME {
            enc.encode_field_str(&order.manual_order_time);
        }

        if sv >= server_version::PEGBEST_PEGMID_OFFSETS {
            let order_type_str = order.order_type.as_ref().map(|t| t.to_string()).unwrap_or_default();
            let is_peg_best = order_type_str == "PEG BEST";
            let is_peg_mid = order_type_str == "PEG MID";

            if contract.exchange == "IBKRATS" {
                enc.encode_field_max_i32(order.min_trade_qty);
            }
            if is_peg_best {
                enc.encode_field_max_i32(order.min_compete_size);
                enc.encode_field_max_f64(order.compete_against_best_offset);
            }
            if is_peg_best || is_peg_mid {
                enc.encode_field_max_f64(order.mid_offset_at_whole);
                enc.encode_field_max_f64(order.mid_offset_at_half);
            }
        }

        if sv >= server_version::CUSTOMER_ACCOUNT {
            enc.encode_field_str(&order.customer_account);
        }
        if sv >= server_version::PROFESSIONAL_CUSTOMER {
            enc.encode_field_bool(order.professional_customer);
        }

        // Deprecated RFQ fields
        if (server_version::RFQ_FIELDS..server_version::UNDO_RFQ_FIELDS).contains(&sv) {
            enc.encode_field_str(""); // bondAccruedInterest placeholder
            enc.encode_field_max_i32(None); // UNSET
        }

        if sv >= server_version::INCLUDE_OVERNIGHT {
            enc.encode_field_bool(order.include_overnight);
        }
        if sv >= server_version::CME_TAGGING_FIELDS {
            enc.encode_field_max_i32(order.manual_order_indicator);
        }
        if sv >= server_version::IMBALANCE_ONLY {
            enc.encode_field_bool(order.imbalance_only);
        }

        self.send_encoded(enc).await
    }

    /// Cancel an order.
    pub async fn cancel_order(&mut self, id: i64, order_cancel: &OrderCancel) -> Result<()> {
        let sv = self.server_version;

        if sv >= server_version::PROTOBUF_PLACE_ORDER {
            return self.cancel_order_protobuf(id, order_cancel).await;
        }

        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_ORDER);
        if sv < server_version::CME_TAGGING_FIELDS_IN_OPEN_ORDER {
            enc.encode_field_i32(1); // version
        }
        enc.encode_field_i64(id);

        if sv >= server_version::MANUAL_ORDER_TIME {
            enc.encode_field_str(&order_cancel.manual_order_cancel_time);
        }

        if (server_version::RFQ_FIELDS..server_version::UNDO_RFQ_FIELDS).contains(&sv) {
            enc.encode_field_str("");
            enc.encode_field_str("");
            enc.encode_field_max_i32(None);
        }

        if sv >= server_version::CME_TAGGING_FIELDS {
            enc.encode_field_str(&order_cancel.ext_operator);
            enc.encode_field_max_i32(order_cancel.manual_order_indicator);
        }

        self.send_encoded(enc).await
    }

    /// Request all open orders.
    pub async fn req_open_orders(&mut self) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_OPEN_ORDERS);
        enc.encode_field_i32(1); // version
        self.send_encoded(enc).await
    }

    /// Request auto open orders.
    pub async fn req_auto_open_orders(&mut self, auto_bind: bool) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_AUTO_OPEN_ORDERS);
        enc.encode_field_i32(1); // version
        enc.encode_field_bool(auto_bind);
        self.send_encoded(enc).await
    }

    /// Request all open orders from all clients.
    pub async fn req_all_open_orders(&mut self) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_ALL_OPEN_ORDERS);
        enc.encode_field_i32(1); // version
        self.send_encoded(enc).await
    }

    /// Cancel all orders globally.
    pub async fn req_global_cancel(&mut self, order_cancel: &OrderCancel) -> Result<()> {
        self.check_server_version(server_version::REQ_GLOBAL_CANCEL, "req_global_cancel")?;
        let sv = self.server_version;

        if sv >= server_version::PROTOBUF_PLACE_ORDER {
            return self.req_global_cancel_protobuf(order_cancel).await;
        }

        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_GLOBAL_CANCEL);
        enc.encode_field_i32(1); // version
        if sv >= server_version::CME_TAGGING_FIELDS {
            enc.encode_field_str(&order_cancel.ext_operator);
            enc.encode_field_max_i32(order_cancel.manual_order_indicator);
        }
        self.send_encoded(enc).await
    }

    /// Request completed orders.
    pub async fn req_completed_orders(&mut self, api_only: bool) -> Result<()> {
        self.check_server_version(server_version::COMPLETED_ORDERS, "req_completed_orders")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_COMPLETED_ORDERS);
        enc.encode_field_bool(api_only);
        self.send_encoded(enc).await
    }

    /// Request execution reports.
    pub async fn req_executions(&mut self, req_id: i32, filter: &ExecutionFilter) -> Result<()> {
        let sv = self.server_version;

        if sv >= server_version::PROTOBUF {
            return self.req_executions_protobuf(req_id, filter).await;
        }

        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_EXECUTIONS);
        enc.encode_field_i32(3); // version
        if sv >= server_version::EXECUTION_DATA_CHAIN {
            enc.encode_field_i32(req_id);
        }
        enc.encode_field_i64(filter.client_id);
        enc.encode_field_str(&filter.acct_code);
        enc.encode_field_str(&filter.time);
        enc.encode_field_str(&filter.symbol);
        enc.encode_field_str(&filter.sec_type);
        enc.encode_field_str(&filter.exchange);
        enc.encode_field_str(&filter.side);

        if sv >= server_version::PARAMETRIZED_DAYS_OF_EXECUTIONS {
            enc.encode_field_max_i32(filter.last_n_days);
            enc.encode_field_i32(filter.specific_dates.len() as i32);
            for date in &filter.specific_dates {
                enc.encode_field_i64(*date);
            }
        }

        self.send_encoded(enc).await
    }

    // ========================================================================
    // Account & Position Requests
    // ========================================================================

    /// Subscribe/unsubscribe to account updates.
    pub async fn req_account_updates(&mut self, subscribe: bool, acct_code: &str) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_ACCT_DATA);
        enc.encode_field_i32(2); // version
        enc.encode_field_bool(subscribe);
        enc.encode_field_str(acct_code);
        self.send_encoded(enc).await
    }

    /// Request account summary.
    pub async fn req_account_summary(
        &mut self,
        req_id: i32,
        group_name: &str,
        tags: &str,
    ) -> Result<()> {
        self.check_server_version(server_version::ACCOUNT_SUMMARY, "req_account_summary")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_ACCOUNT_SUMMARY);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        enc.encode_field_str(group_name);
        enc.encode_field_str(tags);
        self.send_encoded(enc).await
    }

    /// Cancel account summary.
    pub async fn cancel_account_summary(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::ACCOUNT_SUMMARY, "cancel_account_summary")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_ACCOUNT_SUMMARY);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Request account updates for multiple accounts/models.
    pub async fn req_account_updates_multi(
        &mut self,
        req_id: i32,
        account: &str,
        model_code: &str,
        ledger_and_nlv: bool,
    ) -> Result<()> {
        self.check_server_version(server_version::MODELS_SUPPORT, "req_account_updates_multi")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_ACCOUNT_UPDATES_MULTI);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        enc.encode_field_str(account);
        enc.encode_field_str(model_code);
        enc.encode_field_bool(ledger_and_nlv);
        self.send_encoded(enc).await
    }

    /// Cancel account updates multi.
    pub async fn cancel_account_updates_multi(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::MODELS_SUPPORT, "cancel_account_updates_multi")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_ACCOUNT_UPDATES_MULTI);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Request positions for all accounts.
    pub async fn req_positions(&mut self) -> Result<()> {
        self.check_server_version(server_version::POSITIONS, "req_positions")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_POSITIONS);
        enc.encode_field_i32(1); // version
        self.send_encoded(enc).await
    }

    /// Cancel positions subscription.
    pub async fn cancel_positions(&mut self) -> Result<()> {
        self.check_server_version(server_version::POSITIONS, "cancel_positions")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_POSITIONS);
        enc.encode_field_i32(1); // version
        self.send_encoded(enc).await
    }

    /// Request positions for specific account/model.
    pub async fn req_positions_multi(
        &mut self,
        req_id: i32,
        account: &str,
        model_code: &str,
    ) -> Result<()> {
        self.check_server_version(server_version::MODELS_SUPPORT, "req_positions_multi")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_POSITIONS_MULTI);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        enc.encode_field_str(account);
        enc.encode_field_str(model_code);
        self.send_encoded(enc).await
    }

    /// Cancel positions multi.
    pub async fn cancel_positions_multi(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::MODELS_SUPPORT, "cancel_positions_multi")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_POSITIONS_MULTI);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Contract & Symbol Requests
    // ========================================================================

    /// Request contract details.
    pub async fn req_contract_details(
        &mut self,
        req_id: i32,
        contract: &Contract,
    ) -> Result<()> {
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_CONTRACT_DATA);
        enc.encode_field_i32(8); // version
        if sv >= server_version::CONTRACT_DATA_CHAIN {
            enc.encode_field_i32(req_id);
        }
        enc.encode_field_i64(contract.con_id);
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);

        if sv >= server_version::PRIMARYEXCH {
            enc.encode_field_str(&contract.exchange);
            enc.encode_field_str(&contract.primary_exchange);
        } else {
            enc.encode_field_str(&contract.exchange);
        }

        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_str(&contract.trading_class);
        }
        enc.encode_field_bool(contract.include_expired);
        if sv >= server_version::SEC_ID_TYPE {
            enc.encode_field_opt_display(contract.sec_id_type.as_ref());
            enc.encode_field_str(&contract.sec_id);
        }
        if sv >= server_version::BOND_ISSUERID {
            enc.encode_field_str(&contract.issuer_id);
        }
        self.send_encoded(enc).await
    }

    /// Search for matching symbols.
    pub async fn req_matching_symbols(&mut self, req_id: i32, pattern: &str) -> Result<()> {
        self.check_server_version(server_version::REQ_MATCHING_SYMBOLS, "req_matching_symbols")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_MATCHING_SYMBOLS);
        enc.encode_field_i32(req_id);
        enc.encode_field_str(pattern);
        self.send_encoded(enc).await
    }

    /// Request smart components for an exchange.
    pub async fn req_smart_components(&mut self, req_id: i32, bbo_exchange: &str) -> Result<()> {
        self.check_server_version(server_version::REQ_SMART_COMPONENTS, "req_smart_components")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_SMART_COMPONENTS);
        enc.encode_field_i32(req_id);
        enc.encode_field_str(bbo_exchange);
        self.send_encoded(enc).await
    }

    /// Request security definition option parameters.
    pub async fn req_sec_def_opt_params(
        &mut self,
        req_id: i32,
        underlying_symbol: &str,
        fut_fop_exchange: &str,
        underlying_sec_type: &str,
        underlying_con_id: i64,
    ) -> Result<()> {
        self.check_server_version(server_version::SEC_DEF_OPT_PARAMS_REQ, "req_sec_def_opt_params")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_SEC_DEF_OPT_PARAMS);
        enc.encode_field_i32(req_id);
        enc.encode_field_str(underlying_symbol);
        enc.encode_field_str(fut_fop_exchange);
        enc.encode_field_str(underlying_sec_type);
        enc.encode_field_i64(underlying_con_id);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Historical Data Requests
    // ========================================================================

    /// Request historical data bars.
    #[allow(clippy::too_many_arguments)]
    pub async fn req_historical_data(
        &mut self,
        ticker_id: i32,
        contract: &Contract,
        end_date_time: &str,
        duration_str: &str,
        bar_size_setting: &str,
        what_to_show: &str,
        use_rth: bool,
        format_date: i32,
        keep_up_to_date: bool,
        chart_options: &[TagValue],
    ) -> Result<()> {
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_HISTORICAL_DATA);
        if sv < server_version::SYNT_REALTIME_BARS {
            enc.encode_field_i32(6); // version
        }
        enc.encode_field_i32(ticker_id);

        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_i64(contract.con_id);
        }
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);
        enc.encode_field_str(&contract.exchange);
        enc.encode_field_str(&contract.primary_exchange);
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_str(&contract.trading_class);
        }
        enc.encode_field_bool(contract.include_expired);
        enc.encode_field_str(end_date_time);
        enc.encode_field_str(bar_size_setting);
        enc.encode_field_str(duration_str);
        enc.encode_field_bool(use_rth);
        enc.encode_field_str(what_to_show);
        enc.encode_field_i32(format_date);

        // Combo legs for BAG
        if contract.sec_type.as_ref().map(|s| s.to_string()).as_deref() == Some("BAG") {
            if let Some(ref legs) = contract.combo_legs {
                enc.encode_field_i32(legs.len() as i32);
                for leg in legs {
                    enc.encode_field_i64(leg.con_id);
                    enc.encode_field_i64(leg.ratio);
                    enc.encode_field_opt_display(leg.action.as_ref());
                    enc.encode_field_str(&leg.exchange);
                }
            } else {
                enc.encode_field_i32(0);
            }
        }

        if sv >= server_version::SYNT_REALTIME_BARS {
            enc.encode_field_bool(keep_up_to_date);
        }
        if sv >= server_version::LINKING {
            enc.encode_tag_value_list(chart_options);
        }
        self.send_encoded(enc).await
    }

    /// Cancel historical data.
    pub async fn cancel_historical_data(&mut self, ticker_id: i32) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_HISTORICAL_DATA);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(ticker_id);
        self.send_encoded(enc).await
    }

    /// Request earliest available data point.
    pub async fn req_head_timestamp(
        &mut self,
        ticker_id: i32,
        contract: &Contract,
        what_to_show: &str,
        use_rth: bool,
        format_date: i32,
    ) -> Result<()> {
        self.check_server_version(server_version::REQ_HEAD_TIMESTAMP, "req_head_timestamp")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_HEAD_TIMESTAMP);
        enc.encode_field_i32(ticker_id);
        enc.encode_contract(contract);
        enc.encode_field_bool(use_rth);
        enc.encode_field_str(what_to_show);
        enc.encode_field_i32(format_date);
        self.send_encoded(enc).await
    }

    /// Cancel head timestamp request.
    pub async fn cancel_head_timestamp(&mut self, ticker_id: i32) -> Result<()> {
        self.check_server_version(server_version::CANCEL_HEADTIMESTAMP, "cancel_head_timestamp")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_HEAD_TIMESTAMP);
        enc.encode_field_i32(ticker_id);
        self.send_encoded(enc).await
    }

    /// Request historical ticks.
    #[allow(clippy::too_many_arguments)]
    pub async fn req_historical_ticks(
        &mut self,
        req_id: i32,
        contract: &Contract,
        start_date_time: &str,
        end_date_time: &str,
        number_of_ticks: i32,
        what_to_show: &str,
        use_rth: bool,
        ignore_size: bool,
        misc_options: &[TagValue],
    ) -> Result<()> {
        self.check_server_version(server_version::HISTORICAL_TICKS, "req_historical_ticks")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_HISTORICAL_TICKS);
        enc.encode_field_i32(req_id);
        enc.encode_contract(contract);
        enc.encode_field_str(start_date_time);
        enc.encode_field_str(end_date_time);
        enc.encode_field_i32(number_of_ticks);
        enc.encode_field_str(what_to_show);
        enc.encode_field_bool(use_rth);
        enc.encode_field_bool(ignore_size);
        enc.encode_tag_value_list(misc_options);
        self.send_encoded(enc).await
    }

    /// Request histogram data.
    pub async fn req_histogram_data(
        &mut self,
        req_id: i32,
        contract: &Contract,
        use_rth: bool,
        time_period: &str,
    ) -> Result<()> {
        self.check_server_version(server_version::REQ_HISTOGRAM, "req_histogram_data")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_HISTOGRAM_DATA);
        enc.encode_field_i32(req_id);
        enc.encode_contract(contract);
        enc.encode_field_bool(use_rth);
        enc.encode_field_str(time_period);
        self.send_encoded(enc).await
    }

    /// Cancel histogram data.
    pub async fn cancel_histogram_data(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::REQ_HISTOGRAM, "cancel_histogram_data")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_HISTOGRAM_DATA);
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Request historical news headlines.
    #[allow(clippy::too_many_arguments)]
    pub async fn req_historical_news(
        &mut self,
        req_id: i32,
        con_id: i64,
        provider_codes: &str,
        start_date_time: &str,
        end_date_time: &str,
        total_results: i32,
        historical_news_options: &[TagValue],
    ) -> Result<()> {
        self.check_server_version(server_version::REQ_HISTORICAL_NEWS, "req_historical_news")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_HISTORICAL_NEWS);
        enc.encode_field_i32(req_id);
        enc.encode_field_i64(con_id);
        enc.encode_field_str(provider_codes);
        enc.encode_field_str(start_date_time);
        enc.encode_field_str(end_date_time);
        enc.encode_field_i32(total_results);
        enc.encode_tag_value_list(historical_news_options);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Real-Time Data Requests
    // ========================================================================

    /// Request real-time 5-second bars.
    pub async fn req_real_time_bars(
        &mut self,
        ticker_id: i32,
        contract: &Contract,
        bar_size: i32,
        what_to_show: &str,
        use_rth: bool,
        real_time_bars_options: &[TagValue],
    ) -> Result<()> {
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_REAL_TIME_BARS);
        enc.encode_field_i32(3); // version
        enc.encode_field_i32(ticker_id);

        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_i64(contract.con_id);
        }
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);
        enc.encode_field_str(&contract.exchange);
        enc.encode_field_str(&contract.primary_exchange);
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_str(&contract.trading_class);
        }
        enc.encode_field_i32(bar_size);
        enc.encode_field_str(what_to_show);
        enc.encode_field_bool(use_rth);
        if sv >= server_version::LINKING {
            enc.encode_tag_value_list(real_time_bars_options);
        }
        self.send_encoded(enc).await
    }

    /// Cancel real-time bars.
    pub async fn cancel_real_time_bars(&mut self, ticker_id: i32) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_REAL_TIME_BARS);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(ticker_id);
        self.send_encoded(enc).await
    }

    /// Request tick-by-tick data.
    pub async fn req_tick_by_tick_data(
        &mut self,
        req_id: i32,
        contract: &Contract,
        tick_type: &str,
        number_of_ticks: i32,
        ignore_size: bool,
    ) -> Result<()> {
        self.check_server_version(server_version::TICK_BY_TICK, "req_tick_by_tick_data")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_TICK_BY_TICK_DATA);
        enc.encode_field_i32(req_id);
        enc.encode_field_i64(contract.con_id);
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);
        enc.encode_field_str(&contract.exchange);
        enc.encode_field_str(&contract.primary_exchange);
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        enc.encode_field_str(&contract.trading_class);
        enc.encode_field_str(tick_type);

        if self.server_version >= server_version::TICK_BY_TICK_IGNORE_SIZE {
            enc.encode_field_i32(number_of_ticks);
            enc.encode_field_bool(ignore_size);
        }
        self.send_encoded(enc).await
    }

    /// Cancel tick-by-tick data.
    pub async fn cancel_tick_by_tick_data(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::TICK_BY_TICK, "cancel_tick_by_tick_data")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_TICK_BY_TICK_DATA);
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Scanner Requests
    // ========================================================================

    /// Request scanner parameters XML.
    pub async fn req_scanner_parameters(&mut self) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_SCANNER_PARAMETERS);
        enc.encode_field_i32(1); // version
        self.send_encoded(enc).await
    }

    /// Request scanner subscription.
    pub async fn req_scanner_subscription(
        &mut self,
        ticker_id: i32,
        subscription: &ScannerSubscription,
        scanner_subscription_options: &[TagValue],
        scanner_subscription_filter_options: &[TagValue],
    ) -> Result<()> {
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_SCANNER_SUBSCRIPTION);
        if sv < server_version::SCANNER_GENERIC_OPTS {
            enc.encode_field_i32(4); // version
        }
        enc.encode_field_i32(ticker_id);
        enc.encode_field_max_i32(subscription.number_of_rows);
        enc.encode_field_str(&subscription.instrument);
        enc.encode_field_str(&subscription.location_code);
        enc.encode_field_str(&subscription.scan_code);
        enc.encode_field_max_f64(subscription.above_price);
        enc.encode_field_max_f64(subscription.below_price);
        enc.encode_field_max_i32(subscription.above_volume);
        enc.encode_field_max_f64(subscription.market_cap_above);
        enc.encode_field_max_f64(subscription.market_cap_below);
        enc.encode_field_str(&subscription.moody_rating_above);
        enc.encode_field_str(&subscription.moody_rating_below);
        enc.encode_field_str(&subscription.sp_rating_above);
        enc.encode_field_str(&subscription.sp_rating_below);
        enc.encode_field_str(&subscription.maturity_date_above);
        enc.encode_field_str(&subscription.maturity_date_below);
        enc.encode_field_max_f64(subscription.coupon_rate_above);
        enc.encode_field_max_f64(subscription.coupon_rate_below);
        enc.encode_field_max_i32(subscription.exclude_convertible);
        enc.encode_field_max_i32(subscription.average_option_volume_above);
        enc.encode_field_str(&subscription.scanner_setting_pairs);
        enc.encode_field_str(&subscription.stock_type_filter);

        if sv >= server_version::SCANNER_GENERIC_OPTS {
            enc.encode_tag_value_list(scanner_subscription_filter_options);
        }
        if sv >= server_version::LINKING {
            enc.encode_tag_value_list(scanner_subscription_options);
        }
        self.send_encoded(enc).await
    }

    /// Cancel scanner subscription.
    pub async fn cancel_scanner_subscription(&mut self, ticker_id: i32) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_SCANNER_SUBSCRIPTION);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(ticker_id);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Options / Calculations Requests
    // ========================================================================

    /// Calculate implied volatility.
    pub async fn calculate_implied_volatility(
        &mut self,
        req_id: i32,
        contract: &Contract,
        option_price: f64,
        under_price: f64,
        misc_options: &[TagValue],
    ) -> Result<()> {
        self.check_server_version(server_version::REQ_CALC_IMPLIED_VOLAT, "calculate_implied_volatility")?;
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_CALC_IMPLIED_VOLAT);
        enc.encode_field_i32(2); // version
        enc.encode_field_i32(req_id);
        enc.encode_field_i64(contract.con_id);
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);
        enc.encode_field_str(&contract.exchange);
        enc.encode_field_str(&contract.primary_exchange);
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_str(&contract.trading_class);
        }
        enc.encode_field_f64(option_price);
        enc.encode_field_f64(under_price);
        if sv >= server_version::LINKING {
            enc.encode_tag_value_list(misc_options);
        }
        self.send_encoded(enc).await
    }

    /// Cancel calculate implied volatility.
    pub async fn cancel_calculate_implied_volatility(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::CANCEL_CALC_IMPLIED_VOLAT, "cancel_calculate_implied_volatility")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_CALC_IMPLIED_VOLAT);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Calculate option price.
    pub async fn calculate_option_price(
        &mut self,
        req_id: i32,
        contract: &Contract,
        volatility: f64,
        under_price: f64,
        misc_options: &[TagValue],
    ) -> Result<()> {
        self.check_server_version(server_version::REQ_CALC_OPTION_PRICE, "calculate_option_price")?;
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_CALC_OPTION_PRICE);
        enc.encode_field_i32(2); // version
        enc.encode_field_i32(req_id);
        enc.encode_field_i64(contract.con_id);
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);
        enc.encode_field_str(&contract.exchange);
        enc.encode_field_str(&contract.primary_exchange);
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_str(&contract.trading_class);
        }
        enc.encode_field_f64(volatility);
        enc.encode_field_f64(under_price);
        if sv >= server_version::LINKING {
            enc.encode_tag_value_list(misc_options);
        }
        self.send_encoded(enc).await
    }

    /// Cancel calculate option price.
    pub async fn cancel_calculate_option_price(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::CANCEL_CALC_OPTION_PRICE, "cancel_calculate_option_price")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_CALC_OPTION_PRICE);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Exercise options.
    #[allow(clippy::too_many_arguments)]
    pub async fn exercise_options(
        &mut self,
        ticker_id: i32,
        contract: &Contract,
        exercise_action: i32,
        exercise_quantity: i32,
        account: &str,
        override_: i32,
        manual_order_time: &str,
        customer_account: &str,
        professional_customer: bool,
    ) -> Result<()> {
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::EXERCISE_OPTIONS);
        enc.encode_field_i32(2); // version
        enc.encode_field_i32(ticker_id);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_i64(contract.con_id);
        }
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.last_trade_date_or_contract_month);
        enc.encode_field_max_f64(contract.strike);
        enc.encode_field_opt_display(contract.right.as_ref());
        enc.encode_field_str(&contract.multiplier);
        enc.encode_field_str(&contract.exchange);
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_str(&contract.trading_class);
        }
        enc.encode_field_i32(exercise_action);
        enc.encode_field_i32(exercise_quantity);
        enc.encode_field_str(account);
        enc.encode_field_i32(override_);
        if sv >= server_version::MANUAL_ORDER_TIME_EXERCISE_OPTIONS {
            enc.encode_field_str(manual_order_time);
        }
        if sv >= server_version::CUSTOMER_ACCOUNT {
            enc.encode_field_str(customer_account);
        }
        if sv >= server_version::PROFESSIONAL_CUSTOMER {
            enc.encode_field_bool(professional_customer);
        }
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Fundamental Data Requests
    // ========================================================================

    /// Request fundamental data.
    pub async fn req_fundamental_data(
        &mut self,
        req_id: i32,
        contract: &Contract,
        report_type: &str,
        fundamental_data_options: &[TagValue],
    ) -> Result<()> {
        self.check_server_version(server_version::FUNDAMENTAL_DATA, "req_fundamental_data")?;
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_FUNDAMENTAL_DATA);
        enc.encode_field_i32(2); // version
        enc.encode_field_i32(req_id);
        if sv >= server_version::TRADING_CLASS {
            enc.encode_field_i64(contract.con_id);
        }
        enc.encode_field_str(&contract.symbol);
        enc.encode_field_opt_display(contract.sec_type.as_ref());
        enc.encode_field_str(&contract.exchange);
        enc.encode_field_str(&contract.primary_exchange);
        enc.encode_field_str(&contract.currency);
        enc.encode_field_str(&contract.local_symbol);
        enc.encode_field_str(report_type);
        if sv >= server_version::LINKING {
            enc.encode_tag_value_list(fundamental_data_options);
        }
        self.send_encoded(enc).await
    }

    /// Cancel fundamental data.
    pub async fn cancel_fundamental_data(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::FUNDAMENTAL_DATA, "cancel_fundamental_data")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_FUNDAMENTAL_DATA);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // News Requests
    // ========================================================================

    /// Request news bulletins.
    pub async fn req_news_bulletins(&mut self, all_msgs: bool) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_NEWS_BULLETINS);
        enc.encode_field_i32(1); // version
        enc.encode_field_bool(all_msgs);
        self.send_encoded(enc).await
    }

    /// Cancel news bulletins.
    pub async fn cancel_news_bulletins(&mut self) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_NEWS_BULLETINS);
        enc.encode_field_i32(1); // version
        self.send_encoded(enc).await
    }

    /// Request news providers.
    pub async fn req_news_providers(&mut self) -> Result<()> {
        self.check_server_version(server_version::REQ_NEWS_PROVIDERS, "req_news_providers")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_NEWS_PROVIDERS);
        self.send_encoded(enc).await
    }

    /// Request a news article.
    pub async fn req_news_article(
        &mut self,
        req_id: i32,
        provider_code: &str,
        article_id: &str,
        news_article_options: &[TagValue],
    ) -> Result<()> {
        self.check_server_version(server_version::REQ_NEWS_ARTICLE, "req_news_article")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_NEWS_ARTICLE);
        enc.encode_field_i32(req_id);
        enc.encode_field_str(provider_code);
        enc.encode_field_str(article_id);
        enc.encode_tag_value_list(news_article_options);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // P&L Requests
    // ========================================================================

    /// Subscribe to P&L updates.
    pub async fn req_pnl(&mut self, req_id: i32, account: &str, model_code: &str) -> Result<()> {
        self.check_server_version(server_version::PNL, "req_pnl")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_PNL);
        enc.encode_field_i32(req_id);
        enc.encode_field_str(account);
        enc.encode_field_str(model_code);
        self.send_encoded(enc).await
    }

    /// Cancel P&L subscription.
    pub async fn cancel_pnl(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::PNL, "cancel_pnl")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_PNL);
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Subscribe to single-position P&L.
    pub async fn req_pnl_single(
        &mut self,
        req_id: i32,
        account: &str,
        model_code: &str,
        con_id: i64,
    ) -> Result<()> {
        self.check_server_version(server_version::PNL, "req_pnl_single")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_PNL_SINGLE);
        enc.encode_field_i32(req_id);
        enc.encode_field_str(account);
        enc.encode_field_str(model_code);
        enc.encode_field_i64(con_id);
        self.send_encoded(enc).await
    }

    /// Cancel single P&L subscription.
    pub async fn cancel_pnl_single(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::PNL, "cancel_pnl_single")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_PNL_SINGLE);
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Market Rules & Data Lookups
    // ========================================================================

    /// Request market rule details.
    pub async fn req_market_rule(&mut self, market_rule_id: i32) -> Result<()> {
        self.check_server_version(server_version::MARKET_RULES, "req_market_rule")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_MARKET_RULE);
        enc.encode_field_i32(market_rule_id);
        self.send_encoded(enc).await
    }

    /// Request market depth exchanges.
    pub async fn req_mkt_depth_exchanges(&mut self) -> Result<()> {
        self.check_server_version(server_version::REQ_MKT_DEPTH_EXCHANGES, "req_mkt_depth_exchanges")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_MKT_DEPTH_EXCHANGES);
        self.send_encoded(enc).await
    }

    /// Request soft dollar tiers.
    pub async fn req_soft_dollar_tiers(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::SOFT_DOLLAR_TIER, "req_soft_dollar_tiers")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_SOFT_DOLLAR_TIERS);
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Request family codes.
    pub async fn req_family_codes(&mut self) -> Result<()> {
        self.check_server_version(server_version::REQ_FAMILY_CODES, "req_family_codes")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_FAMILY_CODES);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Financial Advisor Requests
    // ========================================================================

    /// Request FA configuration data.
    pub async fn request_fa(&mut self, fa_data_type: i32) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_FA);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(fa_data_type);
        self.send_encoded(enc).await
    }

    /// Replace FA configuration data.
    pub async fn replace_fa(&mut self, req_id: i32, fa_data_type: i32, cxml: &str) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REPLACE_FA);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(fa_data_type);
        enc.encode_field_str(cxml);
        if self.server_version >= server_version::REPLACE_FA_END {
            enc.encode_field_i32(req_id);
        }
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Display Group Requests
    // ========================================================================

    /// Query display groups.
    pub async fn query_display_groups(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::LINKING, "query_display_groups")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::QUERY_DISPLAY_GROUPS);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Subscribe to group events.
    pub async fn subscribe_to_group_events(&mut self, req_id: i32, group_id: i32) -> Result<()> {
        self.check_server_version(server_version::LINKING, "subscribe_to_group_events")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::SUBSCRIBE_TO_GROUP_EVENTS);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        enc.encode_field_i32(group_id);
        self.send_encoded(enc).await
    }

    /// Update display group.
    pub async fn update_display_group(&mut self, req_id: i32, contract_info: &str) -> Result<()> {
        self.check_server_version(server_version::LINKING, "update_display_group")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::UPDATE_DISPLAY_GROUP);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        enc.encode_field_str(contract_info);
        self.send_encoded(enc).await
    }

    /// Unsubscribe from group events.
    pub async fn unsubscribe_from_group_events(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::LINKING, "unsubscribe_from_group_events")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::UNSUBSCRIBE_FROM_GROUP_EVENTS);
        enc.encode_field_i32(1); // version
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Verification Requests
    // ========================================================================

    /// Verify API request.
    pub async fn verify_request(&mut self, api_name: &str, api_version: &str) -> Result<()> {
        self.check_server_version(server_version::LINKING, "verify_request")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::VERIFY_REQUEST);
        enc.encode_field_i32(1); // version
        enc.encode_field_str(api_name);
        enc.encode_field_str(api_version);
        self.send_encoded(enc).await
    }

    /// Verify message.
    pub async fn verify_message(&mut self, api_data: &str) -> Result<()> {
        self.check_server_version(server_version::LINKING, "verify_message")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::VERIFY_MESSAGE);
        enc.encode_field_i32(1); // version
        enc.encode_field_str(api_data);
        self.send_encoded(enc).await
    }

    /// Verify and auth request.
    pub async fn verify_and_auth_request(
        &mut self,
        api_name: &str,
        api_version: &str,
        opaque_isv_key: &str,
    ) -> Result<()> {
        self.check_server_version(server_version::LINKING_AUTH, "verify_and_auth_request")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::VERIFY_AND_AUTH_REQUEST);
        enc.encode_field_i32(1); // version
        enc.encode_field_str(api_name);
        enc.encode_field_str(api_version);
        enc.encode_field_str(opaque_isv_key);
        self.send_encoded(enc).await
    }

    /// Verify and auth message.
    pub async fn verify_and_auth_message(
        &mut self,
        api_data: &str,
        xyz_response: &str,
    ) -> Result<()> {
        self.check_server_version(server_version::LINKING_AUTH, "verify_and_auth_message")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::VERIFY_AND_AUTH_MESSAGE);
        enc.encode_field_i32(1); // version
        enc.encode_field_str(api_data);
        enc.encode_field_str(xyz_response);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // WSH Requests
    // ========================================================================

    /// Request WSH meta data.
    pub async fn req_wsh_meta_data(&mut self, req_id: i32) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_WSH_META_DATA);
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Cancel WSH meta data.
    pub async fn cancel_wsh_meta_data(&mut self, req_id: i32) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_WSH_META_DATA);
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    /// Request WSH event data.
    #[allow(clippy::too_many_arguments)]
    pub async fn req_wsh_event_data(
        &mut self,
        req_id: i32,
        con_id: i32,
        filter: &str,
        fill_watchlist: bool,
        fill_portfolio: bool,
        fill_competitors: bool,
        start_date: &str,
        end_date: &str,
        total_limit: Option<i32>,
    ) -> Result<()> {
        let sv = self.server_version;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_WSH_EVENT_DATA);
        enc.encode_field_i32(req_id);
        enc.encode_field_i32(con_id);
        if sv >= server_version::WSH_EVENT_DATA_FILTERS {
            enc.encode_field_str(filter);
        }
        if sv >= server_version::WSH_EVENT_DATA_FILTERS_DATE {
            enc.encode_field_bool(fill_watchlist);
            enc.encode_field_bool(fill_portfolio);
            enc.encode_field_bool(fill_competitors);
            enc.encode_field_str(start_date);
            enc.encode_field_str(end_date);
            enc.encode_field_max_i32(total_limit);
        }
        self.send_encoded(enc).await
    }

    /// Cancel WSH event data.
    pub async fn cancel_wsh_event_data(&mut self, req_id: i32) -> Result<()> {
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::CANCEL_WSH_EVENT_DATA);
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // User Info Requests
    // ========================================================================

    /// Request user info.
    pub async fn req_user_info(&mut self, req_id: i32) -> Result<()> {
        self.check_server_version(server_version::USER_INFO, "req_user_info")?;
        let mut enc = self.encoder();
        enc.encode_msg_id(outgoing::REQ_USER_INFO);
        enc.encode_field_i32(req_id);
        self.send_encoded(enc).await
    }

    // ========================================================================
    // Private: Protobuf Encoding
    // ========================================================================

    async fn place_order_protobuf(
        &mut self,
        id: i64,
        contract: &Contract,
        order: &Order,
    ) -> Result<()> {
        use prost::Message;
        let request = crate::proto_encode::build_place_order_request(id, contract, order);
        let mut enc = self.encoder();
        enc.encode_raw_int(outgoing::PLACE_ORDER + outgoing::PROTOBUF_MSG_ID);
        enc.write_raw(&request.encode_to_vec());
        self.send_encoded(enc).await
    }

    async fn cancel_order_protobuf(
        &mut self,
        id: i64,
        order_cancel: &OrderCancel,
    ) -> Result<()> {
        use prost::Message;
        let request = crate::proto_encode::build_cancel_order_request(id, order_cancel);
        let mut enc = self.encoder();
        enc.encode_raw_int(outgoing::CANCEL_ORDER + outgoing::PROTOBUF_MSG_ID);
        enc.write_raw(&request.encode_to_vec());
        self.send_encoded(enc).await
    }

    async fn req_executions_protobuf(
        &mut self,
        req_id: i32,
        filter: &ExecutionFilter,
    ) -> Result<()> {
        use prost::Message;
        let request = crate::proto_encode::build_execution_request(req_id, filter);
        let mut enc = self.encoder();
        enc.encode_raw_int(outgoing::REQ_EXECUTIONS + outgoing::PROTOBUF_MSG_ID);
        enc.write_raw(&request.encode_to_vec());
        self.send_encoded(enc).await
    }

    async fn req_global_cancel_protobuf(&mut self, order_cancel: &OrderCancel) -> Result<()> {
        use prost::Message;
        let request = crate::proto_encode::build_global_cancel_request(order_cancel);
        let mut enc = self.encoder();
        enc.encode_raw_int(outgoing::REQ_GLOBAL_CANCEL + outgoing::PROTOBUF_MSG_ID);
        enc.write_raw(&request.encode_to_vec());
        self.send_encoded(enc).await
    }
}

/// Encode an order condition to the wire format.
fn encode_condition(enc: &mut MessageEncoder, cond: &OrderCondition) {
    match cond {
        OrderCondition::Price {
            is_conjunction_connection,
            is_more,
            con_id,
            exchange,
            price,
            trigger_method,
        } => {
            enc.encode_field_i32(1); // Price type
            enc.encode_field_str(if *is_conjunction_connection { "a" } else { "o" });
            enc.encode_field_bool(*is_more);
            enc.encode_field_i32(*con_id);
            enc.encode_field_str(exchange);
            enc.encode_field_f64(*price);
            enc.encode_field_i32(*trigger_method as i32);
        }
        OrderCondition::Time {
            is_conjunction_connection,
            is_more,
            time,
        } => {
            enc.encode_field_i32(3); // Time type
            enc.encode_field_str(if *is_conjunction_connection { "a" } else { "o" });
            enc.encode_field_bool(*is_more);
            enc.encode_field_str(time);
        }
        OrderCondition::Margin {
            is_conjunction_connection,
            is_more,
            percent,
        } => {
            enc.encode_field_i32(4); // Margin type
            enc.encode_field_str(if *is_conjunction_connection { "a" } else { "o" });
            enc.encode_field_bool(*is_more);
            enc.encode_field_i32(*percent);
        }
        OrderCondition::Execution {
            is_conjunction_connection,
            exchange,
            sec_type,
            symbol,
        } => {
            enc.encode_field_i32(5); // Execution type
            enc.encode_field_str(if *is_conjunction_connection { "a" } else { "o" });
            enc.encode_field_str(sec_type);
            enc.encode_field_str(exchange);
            enc.encode_field_str(symbol);
        }
        OrderCondition::Volume {
            is_conjunction_connection,
            is_more,
            con_id,
            exchange,
            volume,
        } => {
            enc.encode_field_i32(6); // Volume type
            enc.encode_field_str(if *is_conjunction_connection { "a" } else { "o" });
            enc.encode_field_bool(*is_more);
            enc.encode_field_i32(*con_id);
            enc.encode_field_str(exchange);
            enc.encode_field_i32(*volume);
        }
        OrderCondition::PercentChange {
            is_conjunction_connection,
            is_more,
            con_id,
            exchange,
            change_percent,
        } => {
            enc.encode_field_i32(7); // PercentChange type
            enc.encode_field_str(if *is_conjunction_connection { "a" } else { "o" });
            enc.encode_field_bool(*is_more);
            enc.encode_field_i32(*con_id);
            enc.encode_field_str(exchange);
            enc.encode_field_max_f64(*change_percent);
        }
    }
}

impl Drop for IBClient {
    fn drop(&mut self) {
        if self.connected.load(Ordering::Relaxed) {
            // Can't do async in Drop, but we can abort the reader task
            if let Some(handle) = self.reader_handle.take() {
                handle.abort();
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Build a framed message from null-terminated fields.
    fn build_framed_msg(fields: &[&str]) -> Vec<u8> {
        let mut body = Vec::new();
        for f in fields {
            body.extend_from_slice(f.as_bytes());
            body.push(0);
        }
        let mut frame = Vec::new();
        frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
        frame.extend(body);
        frame
    }

    /// Create a mock TWS server that performs handshake, reads start_api,
    /// and sends the given messages.
    async fn mock_tws(sv: i32, messages: Vec<Vec<u8>>) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();

            // Read connect request
            let mut buf = vec![0u8; 512];
            let _ = stream.read(&mut buf).await.unwrap();

            // Send handshake
            let handshake = build_framed_msg(&[&sv.to_string(), "20260101 12:00:00"]);
            stream.write_all(&handshake).await.unwrap();

            // Read start_api
            let _ = stream.read(&mut buf).await.unwrap();

            // Send test messages
            for msg in messages {
                stream.write_all(&msg).await.unwrap();
            }

            // Keep connection open briefly, then close
            tokio::task::yield_now().await;
            drop(stream);
        });

        tokio::task::yield_now().await;
        port
    }

    #[tokio::test]
    async fn client_connect_and_receive_events() {
        let messages = vec![
            build_framed_msg(&["9", "1", "100"]),   // NEXT_VALID_ID
            build_framed_msg(&["15", "1", "DU123"]), // MANAGED_ACCTS
        ];

        let port = mock_tws(176, messages).await;

        let (mut client, mut rx) =
            IBClient::connect("127.0.0.1", port, 0, None)
                .await
                .unwrap();

        assert!(client.is_connected());
        assert_eq!(client.server_version(), 176);
        assert_eq!(client.client_id(), 0);

        // Receive events
        let event1 = rx.recv().await.unwrap();
        match event1 {
            IBEvent::NextValidId { order_id } => assert_eq!(order_id, 100),
            other => panic!("expected NextValidId, got {other:?}"),
        }

        let event2 = rx.recv().await.unwrap();
        match event2 {
            IBEvent::ManagedAccounts { accounts } => assert_eq!(accounts, "DU123"),
            other => panic!("expected ManagedAccounts, got {other:?}"),
        }

        client.disconnect().await;
        assert!(!client.is_connected());
    }

    #[tokio::test]
    async fn client_next_req_id() {
        let port = mock_tws(176, vec![]).await;

        let (client, _rx) =
            IBClient::connect("127.0.0.1", port, 0, None)
                .await
                .unwrap();

        assert_eq!(client.next_req_id(), 1);
        assert_eq!(client.next_req_id(), 2);
        assert_eq!(client.next_req_id(), 3);

        client.set_next_req_id(100);
        assert_eq!(client.next_req_id(), 100);
        assert_eq!(client.next_req_id(), 101);
    }

    #[tokio::test]
    async fn client_req_current_time() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 512];

            // Read connect request
            let _ = stream.read(&mut buf).await.unwrap();

            // Send handshake
            let handshake = build_framed_msg(&["176", "20260101 12:00:00"]);
            stream.write_all(&handshake).await.unwrap();

            // Read start_api
            let _ = stream.read(&mut buf).await.unwrap();

            // Read req_current_time
            let n = stream.read(&mut buf).await.unwrap();
            let received = buf[..n].to_vec();

            // Send response: CURRENT_TIME
            let response = build_framed_msg(&["49", "1", "1708876800"]);
            stream.write_all(&response).await.unwrap();

            tokio::task::yield_now().await;
            received
        });

        tokio::task::yield_now().await;

        let (mut client, mut rx) =
            IBClient::connect("127.0.0.1", port, 0, None)
                .await
                .unwrap();

        client.req_current_time().await.unwrap();

        // Receive the response
        let event = rx.recv().await.unwrap();
        match event {
            IBEvent::CurrentTime { time } => assert_eq!(time, 1708876800),
            other => panic!("expected CurrentTime, got {other:?}"),
        }

        // Verify the server received a valid message
        let received = server.await.unwrap();
        assert!(!received.is_empty());
    }

    #[tokio::test]
    async fn client_disconnect() {
        let port = mock_tws(176, vec![]).await;

        let (mut client, mut rx) =
            IBClient::connect("127.0.0.1", port, 0, None)
                .await
                .unwrap();

        assert!(client.is_connected());
        client.disconnect().await;
        assert!(!client.is_connected());

        // Double disconnect is safe
        client.disconnect().await;

        // Channel should be closed (reader task exited)
        // Drain any remaining events
        while rx.recv().await.is_some() {}
    }
}
