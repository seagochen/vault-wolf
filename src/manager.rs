//! VaultWolf Manager — high-level wrapper around the IB TWS API.
//!
//! Uses `ibtws-rust` (our Rust-native IB API client) with an async
//! event-driven architecture. A background event processor task reads
//! `IBEvent`s from the client channel and updates shared state, while
//! request methods send commands and await responses via oneshot channels.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rust_decimal::prelude::ToPrimitive;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use ibtws_rust::{
    Action, Contract, IBClient, IBEvent, Order, OrderCancel, OrderType, Right, SecType, TickType,
};

use crate::models;

// ============================================================================
// Pending request types
// ============================================================================

/// A pending request waiting for server response.
enum PendingRequest {
    /// Waiting for HistoricalData + HistoricalDataEnd events.
    HistoricalData {
        tx: oneshot::Sender<Result<models::HistoricalData, String>>,
        symbol: String,
        sec_type: String,
        bars: Vec<models::HistoricalBar>,
    },
    /// Waiting for AccountSummary events + AccountSummaryEnd.
    AccountSummary {
        tx: oneshot::Sender<Result<HashMap<String, models::AccountSummary>, String>>,
        data: HashMap<String, models::AccountSummary>,
    },
    /// Waiting for Position events + PositionEnd.
    Positions {
        tx: oneshot::Sender<Result<Vec<models::Position>, String>>,
        data: Vec<models::Position>,
    },
}

// ============================================================================
// Shared state updated by the event processor
// ============================================================================

/// State shared between the event processor and the manager.
pub struct SharedState {
    pub tick_data: Mutex<HashMap<String, models::TickData>>,
    pub order_map: Mutex<HashMap<i64, models::OrderInfo>>,
    pub managed_accounts: Mutex<Vec<String>>,
    pub next_order_id: AtomicI64,
}

impl SharedState {
    fn new() -> Self {
        Self {
            tick_data: Mutex::new(HashMap::new()),
            order_map: Mutex::new(HashMap::new()),
            managed_accounts: Mutex::new(Vec::new()),
            next_order_id: AtomicI64::new(0),
        }
    }
}

// ============================================================================
// VaultWolfManager
// ============================================================================

/// Async manager that wraps an IB TWS API client.
pub struct VaultWolfManager {
    client: Option<IBClient>,
    connected: AtomicBool,

    // Shared state (updated by event processor)
    state: Arc<SharedState>,

    // Cached data
    historical_data_cache: Mutex<HashMap<i64, models::HistoricalData>>,
    account_summary: Mutex<HashMap<String, models::AccountSummary>>,
    positions: Mutex<Vec<models::Position>>,
    req_id_to_contract: Mutex<HashMap<i64, models::ContractSpec>>,

    // ID management
    next_req_id: AtomicI64,

    // Pending requests for event processor to fulfill
    pending: Arc<Mutex<HashMap<i32, PendingRequest>>>,

    // Event processor handle
    event_handle: Option<JoinHandle<()>>,
}

impl VaultWolfManager {
    pub fn new() -> Self {
        Self {
            client: None,
            connected: AtomicBool::new(false),
            state: Arc::new(SharedState::new()),
            historical_data_cache: Mutex::new(HashMap::new()),
            account_summary: Mutex::new(HashMap::new()),
            positions: Mutex::new(Vec::new()),
            req_id_to_contract: Mutex::new(HashMap::new()),
            next_req_id: AtomicI64::new(1000),
            pending: Arc::new(Mutex::new(HashMap::new())),
            event_handle: None,
        }
    }

    // ========================================================================
    // Connection Management
    // ========================================================================

    pub async fn connect_to_ib(
        &mut self,
        host: &str,
        port: u16,
        client_id: i32,
    ) -> Result<(), String> {
        tracing::info!("Connecting to IB TWS/Gateway at {host}:{port}...");

        let (client, rx) = IBClient::connect(host, port, client_id, None)
            .await
            .map_err(|e| format!("Connection failed: {e}"))?;

        self.client = Some(client);
        self.connected.store(true, Ordering::SeqCst);

        // Spawn event processor
        let handle = spawn_event_processor(
            rx,
            Arc::clone(&self.state),
            Arc::clone(&self.pending),
        );
        self.event_handle = Some(handle);

        // Wait briefly for ManagedAccounts event
        tokio::time::sleep(Duration::from_millis(500)).await;

        let accounts = self.state.managed_accounts.lock().await;
        if !accounts.is_empty() {
            tracing::info!("Managed accounts: {}", accounts.join(", "));
        }

        tracing::info!("Successfully connected to IB");
        Ok(())
    }

    pub async fn disconnect_from_ib(&mut self) {
        self.connected.store(false, Ordering::SeqCst);
        if let Some(client) = &mut self.client {
            client.disconnect().await;
        }
        self.client = None;
        if let Some(handle) = self.event_handle.take() {
            let _ = handle.await;
        }
        tracing::info!("Disconnected from IB");
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst) && self.client.is_some()
    }

    fn client_mut(&mut self) -> Result<&mut IBClient, String> {
        self.client
            .as_mut()
            .ok_or_else(|| "Not connected to IB".to_string())
    }

    // ========================================================================
    // Market Data
    // ========================================================================

    /// Subscribe to market data (ticks arrive via the event processor).
    pub async fn request_market_data(
        &mut self,
        spec: &models::ContractSpec,
    ) -> Result<i64, String> {
        let req_id = self.next_req_id.fetch_add(1, Ordering::SeqCst);
        let contract = build_contract(spec);

        let client = self.client_mut()?;
        client
            .req_mkt_data(req_id as i32, &contract, "", false, false, &[])
            .await
            .map_err(|e| format!("req_mkt_data failed: {e}"))?;

        let key = contract_key(&spec.symbol, &spec.sec_type);
        self.req_id_to_contract
            .lock()
            .await
            .insert(req_id, spec.clone());
        self.state.tick_data.lock().await.insert(
            key,
            models::TickData {
                symbol: spec.symbol.clone(),
                sec_type: spec.sec_type.clone(),
                req_id,
                ..Default::default()
            },
        );

        tracing::info!(
            "Market data subscription created: req_id={req_id}, symbol={}",
            spec.symbol
        );
        Ok(req_id)
    }

    pub async fn cancel_market_data(&mut self, req_id: i64) {
        if let Some(client) = &mut self.client {
            let _ = client.cancel_mkt_data(req_id as i32).await;
        }
        if let Some(spec) = self.req_id_to_contract.lock().await.remove(&req_id) {
            let key = contract_key(&spec.symbol, &spec.sec_type);
            self.state.tick_data.lock().await.remove(&key);
        }
        tracing::info!("Market data subscription cancelled: req_id={req_id}");
    }

    pub async fn get_tick_data(&self, symbol: &str, sec_type: &str) -> Option<models::TickData> {
        let key = contract_key(symbol, sec_type);
        self.state.tick_data.lock().await.get(&key).cloned()
    }

    // ========================================================================
    // Historical Data
    // ========================================================================

    /// Request historical data asynchronously.
    pub async fn request_historical_data(
        &mut self,
        spec: &models::ContractSpec,
        end_date_time: Option<&str>,
        duration: &str,
        bar_size: &str,
        what_to_show: &str,
    ) -> Result<(i64, models::HistoricalData), String> {
        let contract = build_contract(spec);
        let req_id = self.next_req_id.fetch_add(1, Ordering::SeqCst);
        let req_id_i32 = req_id as i32;

        // Register pending request
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(
                req_id_i32,
                PendingRequest::HistoricalData {
                    tx,
                    symbol: spec.symbol.clone(),
                    sec_type: spec.sec_type.clone(),
                    bars: Vec::new(),
                },
            );
        }

        let end_dt = end_date_time.unwrap_or("");

        let client = self.client_mut()?;
        client
            .req_historical_data(
                req_id_i32,
                &contract,
                end_dt,
                duration,
                bar_size,
                what_to_show,
                true, // use_rth = Regular Trading Hours
                1,    // format_date = 1 (yyyyMMdd HH:mm:ss)
                false,
                &[],
            )
            .await
            .map_err(|e| {
                // Clean up pending on failure — can't await here, so use try_lock
                if let Ok(mut p) = self.pending.try_lock() {
                    p.remove(&req_id_i32);
                }
                format!("Historical data request failed: {e}")
            })?;

        // Wait for response with timeout
        let result = tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| "Historical data request timed out".to_string())?
            .map_err(|_| "Event processor dropped".to_string())?;

        match result {
            Ok(hist) => {
                self.historical_data_cache
                    .lock()
                    .await
                    .insert(req_id, hist.clone());
                Ok((req_id, hist))
            }
            Err(e) => Err(e),
        }
    }

    pub async fn get_historical_data(&self, req_id: i64) -> Option<models::HistoricalData> {
        self.historical_data_cache
            .lock()
            .await
            .get(&req_id)
            .cloned()
    }

    // ========================================================================
    // Account APIs
    // ========================================================================

    pub async fn request_account_summary(&mut self) -> Result<(), String> {
        let req_id = self.next_req_id.fetch_add(1, Ordering::SeqCst) as i32;

        // Register pending request
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(
                req_id,
                PendingRequest::AccountSummary {
                    tx,
                    data: HashMap::new(),
                },
            );
        }

        // ALL tags for account summary
        let tags = "AccountType,NetLiquidation,TotalCashValue,SettledCash,\
                    AccruedCash,BuyingPower,EquityWithLoanValue,PreviousEquityWithLoanValue,\
                    GrossPositionValue,ReqTEquity,ReqTMargin,SMA,InitMarginReq,MaintMarginReq,\
                    AvailableFunds,ExcessLiquidity,Cushion,FullInitMarginReq,FullMaintMarginReq,\
                    FullAvailableFunds,FullExcessLiquidity,LookAheadNextChange,LookAheadInitMarginReq,\
                    LookAheadMaintMarginReq,LookAheadAvailableFunds,LookAheadExcessLiquidity,\
                    HighestSeverity,DayTradesRemaining,Leverage";

        let client = self.client_mut()?;
        client
            .req_account_summary(req_id, "All", tags)
            .await
            .map_err(|e| format!("Account summary request failed: {e}"))?;

        // Wait for response
        let result = tokio::time::timeout(Duration::from_secs(10), rx)
            .await
            .map_err(|_| "Account summary request timed out".to_string())?
            .map_err(|_| "Event processor dropped".to_string())?;

        match result {
            Ok(data) => {
                *self.account_summary.lock().await = data;

                // Cancel the subscription after receiving data
                if let Some(client) = &mut self.client {
                    let _ = client.cancel_account_summary(req_id).await;
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn get_account_summary(&self, account: &str) -> Option<models::AccountSummary> {
        let map = self.account_summary.lock().await;
        if account.is_empty() {
            map.values().next().cloned()
        } else {
            map.get(account).cloned()
        }
    }

    pub async fn request_positions(&mut self) -> Result<(), String> {
        // Register pending request with sentinel req_id = -1 (positions have no req_id)
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(
                -1,
                PendingRequest::Positions {
                    tx,
                    data: Vec::new(),
                },
            );
        }

        let client = self.client_mut()?;
        client
            .req_positions()
            .await
            .map_err(|e| format!("Positions request failed: {e}"))?;

        // Wait for response
        let result = tokio::time::timeout(Duration::from_secs(10), rx)
            .await
            .map_err(|_| "Positions request timed out".to_string())?
            .map_err(|_| "Event processor dropped".to_string())?;

        match result {
            Ok(data) => {
                *self.positions.lock().await = data;

                // Cancel positions subscription
                if let Some(client) = &mut self.client {
                    let _ = client.cancel_positions().await;
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn get_all_positions(&self) -> Vec<models::Position> {
        self.positions.lock().await.clone()
    }

    pub async fn get_positions_by_account(&self, account: &str) -> Vec<models::Position> {
        self.positions
            .lock()
            .await
            .iter()
            .filter(|p| p.account == account)
            .cloned()
            .collect()
    }

    pub async fn get_positions_by_symbol(
        &self,
        symbol: &str,
        sec_type: &str,
    ) -> Vec<models::Position> {
        self.positions
            .lock()
            .await
            .iter()
            .filter(|p| p.symbol == symbol && p.sec_type == sec_type)
            .cloned()
            .collect()
    }

    // ========================================================================
    // Order APIs
    // ========================================================================

    pub async fn place_market_order(
        &mut self,
        spec: &models::ContractSpec,
        action: &str,
        quantity: f64,
    ) -> Result<i64, String> {
        let contract = build_contract(spec);
        let order_id = self.state.next_order_id.fetch_add(1, Ordering::SeqCst);

        let order = Order {
            action: Some(parse_action(action)),
            total_quantity: Some(rust_decimal::Decimal::from_f64_retain(quantity).unwrap_or_default()),
            order_type: Some(OrderType::Market),
            ..Order::default()
        };

        let client = self.client_mut()?;
        client
            .place_order(order_id, &contract, &order)
            .await
            .map_err(|e| format!("Place order failed: {e}"))?;

        self.store_order(order_id, spec, action, "MKT", quantity, 0.0, 0.0)
            .await;
        tracing::info!(
            "Market order placed: id={order_id}, symbol={}, action={action}, qty={quantity}",
            spec.symbol
        );
        Ok(order_id)
    }

    pub async fn place_limit_order(
        &mut self,
        spec: &models::ContractSpec,
        action: &str,
        quantity: f64,
        limit_price: f64,
    ) -> Result<i64, String> {
        let contract = build_contract(spec);
        let order_id = self.state.next_order_id.fetch_add(1, Ordering::SeqCst);

        let order = Order {
            action: Some(parse_action(action)),
            total_quantity: Some(rust_decimal::Decimal::from_f64_retain(quantity).unwrap_or_default()),
            order_type: Some(OrderType::Limit),
            lmt_price: Some(limit_price),
            ..Order::default()
        };

        let client = self.client_mut()?;
        client
            .place_order(order_id, &contract, &order)
            .await
            .map_err(|e| format!("Place order failed: {e}"))?;

        self.store_order(order_id, spec, action, "LMT", quantity, limit_price, 0.0)
            .await;
        tracing::info!(
            "Limit order placed: id={order_id}, symbol={}, price={limit_price}",
            spec.symbol
        );
        Ok(order_id)
    }

    pub async fn place_stop_order(
        &mut self,
        spec: &models::ContractSpec,
        action: &str,
        quantity: f64,
        stop_price: f64,
    ) -> Result<i64, String> {
        let contract = build_contract(spec);
        let order_id = self.state.next_order_id.fetch_add(1, Ordering::SeqCst);

        let order = Order {
            action: Some(parse_action(action)),
            total_quantity: Some(rust_decimal::Decimal::from_f64_retain(quantity).unwrap_or_default()),
            order_type: Some(OrderType::Stop),
            aux_price: Some(stop_price),
            ..Order::default()
        };

        let client = self.client_mut()?;
        client
            .place_order(order_id, &contract, &order)
            .await
            .map_err(|e| format!("Place order failed: {e}"))?;

        self.store_order(order_id, spec, action, "STP", quantity, 0.0, stop_price)
            .await;
        tracing::info!(
            "Stop order placed: id={order_id}, symbol={}, stop={stop_price}",
            spec.symbol
        );
        Ok(order_id)
    }

    pub async fn cancel_order(&mut self, order_id: i64) -> Result<(), String> {
        let client = self.client_mut()?;
        client
            .cancel_order(order_id, &OrderCancel::default())
            .await
            .map_err(|e| format!("Cancel order failed: {e}"))?;
        tracing::info!("Order cancellation requested: id={order_id}");
        Ok(())
    }

    pub async fn get_order(&self, order_id: i64) -> Option<models::OrderInfo> {
        self.state.order_map.lock().await.get(&order_id).cloned()
    }

    pub async fn get_all_orders(&self) -> Vec<models::OrderInfo> {
        self.state
            .order_map
            .lock()
            .await
            .values()
            .cloned()
            .collect()
    }

    pub async fn get_orders_by_status(&self, status: &str) -> Vec<models::OrderInfo> {
        self.state
            .order_map
            .lock()
            .await
            .values()
            .filter(|o| o.status == status)
            .cloned()
            .collect()
    }

    pub async fn get_orders_by_symbol(
        &self,
        symbol: &str,
        sec_type: &str,
    ) -> Vec<models::OrderInfo> {
        self.state
            .order_map
            .lock()
            .await
            .values()
            .filter(|o| o.symbol == symbol && o.sec_type == sec_type)
            .cloned()
            .collect()
    }

    // ========================================================================
    // Utility
    // ========================================================================

    pub async fn get_managed_accounts(&self) -> Vec<String> {
        self.state.managed_accounts.lock().await.clone()
    }

    async fn store_order(
        &self,
        order_id: i64,
        spec: &models::ContractSpec,
        action: &str,
        order_type: &str,
        quantity: f64,
        lmt_price: f64,
        aux_price: f64,
    ) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let is_opt = spec.sec_type == "OPT";
        let info = models::OrderInfo {
            order_id,
            symbol: spec.symbol.clone(),
            sec_type: spec.sec_type.clone(),
            exchange: spec.exchange.clone(),
            currency: spec.currency.clone(),
            action: action.to_string(),
            order_type: order_type.to_string(),
            total_quantity: quantity,
            lmt_price,
            aux_price,
            status: "PendingSubmit".to_string(),
            right: if is_opt { spec.right.clone() } else { None },
            strike: if is_opt { spec.strike } else { None },
            expiry: if is_opt { spec.expiry.clone() } else { None },
            submit_time: now.clone(),
            last_update_time: now,
            ..Default::default()
        };
        self.state.order_map.lock().await.insert(order_id, info);
    }
}

// ============================================================================
// Event Processor
// ============================================================================

/// Spawns a background task that processes IBEvents.
fn spawn_event_processor(
    mut rx: mpsc::UnboundedReceiver<IBEvent>,
    state: Arc<SharedState>,
    pending: Arc<Mutex<HashMap<i32, PendingRequest>>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            process_event(event, &state, &pending).await;
        }
        tracing::debug!("Event processor exiting (channel closed)");
    })
}

async fn process_event(
    event: IBEvent,
    state: &SharedState,
    pending: &Mutex<HashMap<i32, PendingRequest>>,
) {
    match event {
        // -- Connection --
        IBEvent::NextValidId { order_id } => {
            state.next_order_id.store(order_id, Ordering::SeqCst);
            tracing::info!("Next valid order ID: {order_id}");
        }

        IBEvent::ManagedAccounts { accounts } => {
            let list: Vec<String> = accounts
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            *state.managed_accounts.lock().await = list;
        }

        IBEvent::Error {
            req_id,
            code,
            message,
            ..
        } => {
            tracing::warn!("IB Error: req_id={req_id}, code={code}, msg={message}");

            // If this error relates to a pending request, fail it
            if req_id >= 0 {
                let mut pending_map = pending.lock().await;
                if let Some(req) = pending_map.remove(&req_id) {
                    let err_msg = format!("IB Error {code}: {message}");
                    match req {
                        PendingRequest::HistoricalData { tx, .. } => {
                            let _ = tx.send(Err(err_msg));
                        }
                        PendingRequest::AccountSummary { tx, .. } => {
                            let _ = tx.send(Err(err_msg));
                        }
                        PendingRequest::Positions { tx, .. } => {
                            let _ = tx.send(Err(err_msg));
                        }
                    }
                }
            }
        }

        // -- Tick Data (market data updates) --
        IBEvent::TickPrice {
            req_id,
            tick_type,
            price,
            ..
        } => {
            let mut ticks = state.tick_data.lock().await;
            for td in ticks.values_mut() {
                if td.req_id == req_id as i64 {
                    match tick_type {
                        TickType::Bid => td.bid = price,
                        TickType::Ask => td.ask = price,
                        TickType::Last => td.last = price,
                        TickType::High => td.high = price,
                        TickType::Low => td.low = price,
                        TickType::Close => td.close = price,
                        TickType::Open => td.open = price,
                        _ => {}
                    }
                    break;
                }
            }
        }

        IBEvent::TickSize {
            req_id,
            tick_type,
            size,
            ..
        } => {
            let size_val = size.to_i64().unwrap_or(0);
            let mut ticks = state.tick_data.lock().await;
            for td in ticks.values_mut() {
                if td.req_id == req_id as i64 {
                    match tick_type {
                        TickType::BidSize => td.bid_size = size_val,
                        TickType::AskSize => td.ask_size = size_val,
                        TickType::LastSize => td.last_size = size_val,
                        TickType::Volume => td.volume = size_val,
                        _ => {}
                    }
                    break;
                }
            }
        }

        IBEvent::TickOptionComputation {
            req_id,
            implied_vol,
            delta,
            gamma,
            vega,
            theta,
            opt_price,
            pv_dividend,
            und_price,
            ..
        } => {
            let mut ticks = state.tick_data.lock().await;
            for td in ticks.values_mut() {
                if td.req_id == req_id as i64 {
                    td.implied_vol = implied_vol;
                    td.delta = delta;
                    td.gamma = gamma;
                    td.vega = vega;
                    td.theta = theta;
                    td.opt_price = opt_price;
                    td.pv_dividend = pv_dividend;
                    td.und_price = und_price;
                    break;
                }
            }
        }

        // -- Historical Data --
        IBEvent::HistoricalData { req_id, bars } => {
            let mut pending_map = pending.lock().await;
            if let Some(PendingRequest::HistoricalData {
                bars: ref mut pending_bars,
                ..
            }) = pending_map.get_mut(&req_id)
            {
                for b in &bars {
                    pending_bars.push(models::HistoricalBar {
                        date: b.time.clone(),
                        open: b.open,
                        high: b.high,
                        low: b.low,
                        close: b.close,
                        volume: b
                            .volume
                            .as_ref()
                            .and_then(|v| v.to_i64())
                            .unwrap_or(0),
                        bar_count: b.count,
                        wap: b
                            .wap
                            .as_ref()
                            .and_then(|v| v.to_f64())
                            .unwrap_or(0.0),
                    });
                }
            }
        }

        IBEvent::HistoricalDataEnd {
            req_id, start, end, ..
        } => {
            let mut pending_map = pending.lock().await;
            if let Some(PendingRequest::HistoricalData {
                tx,
                symbol,
                sec_type,
                bars,
            }) = pending_map.remove(&req_id)
            {
                let hist = models::HistoricalData {
                    symbol,
                    sec_type,
                    req_id: req_id as i64,
                    start_date: start,
                    end_date: end,
                    bars,
                };
                let _ = tx.send(Ok(hist));
            }
        }

        // -- Account Summary --
        IBEvent::AccountSummary {
            req_id,
            account,
            tag,
            value,
            ..
        } => {
            let mut pending_map = pending.lock().await;
            if let Some(PendingRequest::AccountSummary { data, .. }) =
                pending_map.get_mut(&req_id)
            {
                let entry =
                    data.entry(account.clone())
                        .or_insert_with(|| models::AccountSummary {
                            account,
                            values: HashMap::new(),
                        });
                entry.values.insert(tag, value);
            }
        }

        IBEvent::AccountSummaryEnd { req_id } => {
            let mut pending_map = pending.lock().await;
            if let Some(PendingRequest::AccountSummary { tx, data }) =
                pending_map.remove(&req_id)
            {
                let _ = tx.send(Ok(data));
            }
        }

        // -- Positions --
        IBEvent::Position {
            account,
            contract,
            position,
            avg_cost,
        } => {
            let mut pending_map = pending.lock().await;
            if let Some(PendingRequest::Positions { data, .. }) = pending_map.get_mut(&-1) {
                data.push(models::Position {
                    account,
                    symbol: contract.symbol.clone(),
                    sec_type: contract
                        .sec_type
                        .as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    currency: contract.currency.clone(),
                    exchange: contract.exchange.clone(),
                    position: position.to_f64().unwrap_or(0.0),
                    avg_cost,
                    ..Default::default()
                });
            }
        }

        IBEvent::PositionEnd => {
            let mut pending_map = pending.lock().await;
            if let Some(PendingRequest::Positions { tx, data }) = pending_map.remove(&-1) {
                let _ = tx.send(Ok(data));
            }
        }

        // -- Order Status --
        IBEvent::OrderStatus {
            order_id,
            status,
            filled,
            remaining,
            avg_fill_price,
            perm_id,
            last_fill_price,
            ..
        } => {
            let mut orders = state.order_map.lock().await;
            if let Some(info) = orders.get_mut(&order_id) {
                info.status = status;
                info.filled = filled.to_f64().unwrap_or(0.0);
                info.remaining = remaining.to_f64().unwrap_or(0.0);
                info.avg_fill_price = avg_fill_price;
                info.perm_id = perm_id;
                info.last_fill_price = last_fill_price;
                info.last_update_time =
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            }
        }

        // Ignore other events
        _ => {}
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn build_contract(spec: &models::ContractSpec) -> Contract {
    let sec_type = match spec.sec_type.as_str() {
        "STK" => Some(SecType::Stock),
        "OPT" => Some(SecType::Option),
        "FUT" => Some(SecType::Future),
        "CASH" => Some(SecType::Forex),
        "IND" => Some(SecType::Index),
        "FOP" => Some(SecType::FutureOption),
        "BOND" => Some(SecType::Bond),
        "CRYPTO" => Some(SecType::Crypto),
        other if !other.is_empty() => Some(SecType::Other(other.to_string())),
        _ => Some(SecType::Stock),
    };

    let (right, strike) = if spec.sec_type == "OPT" {
        let right = match spec.right.as_deref() {
            Some("P") => Some(Right::Put),
            Some("C") => Some(Right::Call),
            _ => Some(Right::Call),
        };
        (right, spec.strike)
    } else {
        (None, None)
    };

    let last_trade_date = if spec.sec_type == "OPT" || spec.sec_type == "FUT" {
        spec.expiry
            .as_deref()
            .or(spec.last_trade_date_or_contract_month.as_deref())
            .unwrap_or("")
            .to_string()
    } else {
        String::new()
    };

    Contract {
        symbol: spec.symbol.clone(),
        sec_type,
        exchange: if spec.exchange.is_empty() {
            "SMART".to_string()
        } else {
            spec.exchange.clone()
        },
        currency: if spec.currency.is_empty() {
            "USD".to_string()
        } else {
            spec.currency.clone()
        },
        right,
        strike,
        last_trade_date_or_contract_month: last_trade_date,
        ..Contract::default()
    }
}

fn contract_key(symbol: &str, sec_type: &str) -> String {
    format!("{symbol}:{sec_type}")
}

fn parse_action(action: &str) -> Action {
    match action.to_uppercase().as_str() {
        "SELL" => Action::Sell,
        _ => Action::Buy,
    }
}
