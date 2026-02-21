//! VaultWolf Manager — high-level wrapper around the IB TWS API.
//!
//! Replaces the C++ `VaultEWrapper` + `VaultWolfManager` classes and
//! completely removes the dependency on `IntelRDFPMathLib` (libbid).
//! The Rust `ibapi` crate handles decimal encoding internally.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicI64, Ordering},
    Mutex,
};

use ibapi::accounts::{AccountSummaryResult, AccountSummaryTags, PositionUpdate};
use ibapi::client::blocking::Client;
use ibapi::contracts::Contract;
use ibapi::market_data::historical;
use ibapi::market_data::TradingHours;
use ibapi::orders;
use ibapi::prelude::*;

use crate::models;

// ============================================================================
// VaultWolfManager
// ============================================================================

/// Thread-safe manager that wraps an IB TWS API client.
pub struct VaultWolfManager {
    client: Option<Client>,
    connected: AtomicBool,

    // Market data storage
    tick_data: Mutex<HashMap<String, models::TickData>>,
    historical_data_cache: Mutex<HashMap<i64, models::HistoricalData>>,
    req_id_to_contract: Mutex<HashMap<i64, models::ContractSpec>>,

    // Account data storage
    account_summary: Mutex<HashMap<String, models::AccountSummary>>,
    positions: Mutex<Vec<models::Position>>,

    // Order data storage
    order_map: Mutex<HashMap<i64, models::OrderInfo>>,

    // Managed accounts
    managed_accounts: Mutex<Vec<String>>,

    // ID management
    next_req_id: AtomicI64,
}

impl VaultWolfManager {
    pub fn new() -> Self {
        Self {
            client: None,
            connected: AtomicBool::new(false),
            tick_data: Mutex::new(HashMap::new()),
            historical_data_cache: Mutex::new(HashMap::new()),
            req_id_to_contract: Mutex::new(HashMap::new()),
            account_summary: Mutex::new(HashMap::new()),
            positions: Mutex::new(Vec::new()),
            order_map: Mutex::new(HashMap::new()),
            managed_accounts: Mutex::new(Vec::new()),
            next_req_id: AtomicI64::new(1000),
        }
    }

    // ========================================================================
    // Connection Management
    // ========================================================================

    pub fn connect_to_ib(&mut self, host: &str, port: u16, client_id: i32) -> Result<(), String> {
        let addr = format!("{}:{}", host, port);
        tracing::info!("Connecting to IB TWS/Gateway at {addr}...");

        match Client::connect(&addr, client_id) {
            Ok(c) => {
                // Fetch managed accounts
                if let Ok(accounts) = c.managed_accounts() {
                    *self.managed_accounts.lock().unwrap() = accounts;
                }
                self.client = Some(c);
                self.connected.store(true, Ordering::SeqCst);
                tracing::info!("Successfully connected to IB");
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to connect to IB: {e}");
                Err(format!("Connection failed: {e}"))
            }
        }
    }

    pub fn disconnect_from_ib(&mut self) {
        self.connected.store(false, Ordering::SeqCst);
        self.client = None;
        tracing::info!("Disconnected from IB");
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst) && self.client.is_some()
    }

    fn client(&self) -> Result<&Client, String> {
        self.client.as_ref().ok_or_else(|| "Not connected to IB".to_string())
    }

    // ========================================================================
    // Market Data
    // ========================================================================

    /// Subscribe to market data (stores a placeholder; real ticks come via polling).
    pub fn request_market_data(&self, spec: &models::ContractSpec) -> Result<i64, String> {
        let _client = self.client()?;
        let req_id = self.next_req_id.fetch_add(1, Ordering::SeqCst);
        let key = contract_key(&spec.symbol, &spec.sec_type);

        self.req_id_to_contract.lock().unwrap().insert(req_id, spec.clone());
        self.tick_data.lock().unwrap().insert(
            key,
            models::TickData {
                symbol: spec.symbol.clone(),
                sec_type: spec.sec_type.clone(),
                req_id,
                ..Default::default()
            },
        );

        tracing::info!("Market data subscription created: req_id={req_id}, symbol={}", spec.symbol);
        Ok(req_id)
    }

    pub fn cancel_market_data(&self, req_id: i64) {
        if let Some(spec) = self.req_id_to_contract.lock().unwrap().remove(&req_id) {
            let key = contract_key(&spec.symbol, &spec.sec_type);
            self.tick_data.lock().unwrap().remove(&key);
        }
        tracing::info!("Market data subscription cancelled: req_id={req_id}");
    }

    pub fn get_tick_data(&self, symbol: &str, sec_type: &str) -> Option<models::TickData> {
        let key = contract_key(symbol, sec_type);
        self.tick_data.lock().unwrap().get(&key).cloned()
    }

    /// Request historical data synchronously.
    pub fn request_historical_data(
        &self,
        spec: &models::ContractSpec,
        end_date_time: Option<&str>,
        duration: &str,
        bar_size: &str,
        what_to_show: &str,
    ) -> Result<(i64, models::HistoricalData), String> {
        let client = self.client()?;
        let contract = build_contract(spec);
        let req_id = self.next_req_id.fetch_add(1, Ordering::SeqCst);

        let ib_duration = parse_duration(duration);
        let ib_bar_size = parse_bar_size(bar_size);
        let ib_what = parse_what_to_show(what_to_show);
        let end_dt = end_date_time.and_then(parse_ib_datetime);

        match client.historical_data(
            &contract,
            end_dt,
            ib_duration,
            ib_bar_size,
            ib_what,
            TradingHours::Regular,
        ) {
            Ok(data) => {
                let hist = models::HistoricalData {
                    symbol: spec.symbol.clone(),
                    sec_type: spec.sec_type.clone(),
                    req_id,
                    start_date: data.start.to_string(),
                    end_date: data.end.to_string(),
                    bars: data
                        .bars
                        .iter()
                        .map(|b| models::HistoricalBar {
                            date: b.date.to_string(),
                            open: b.open,
                            high: b.high,
                            low: b.low,
                            close: b.close,
                            volume: b.volume as i64,
                            bar_count: b.count,
                            wap: b.wap,
                        })
                        .collect(),
                };
                self.historical_data_cache.lock().unwrap().insert(req_id, hist.clone());
                Ok((req_id, hist))
            }
            Err(e) => Err(format!("Historical data request failed: {e}")),
        }
    }

    pub fn get_historical_data(&self, req_id: i64) -> Option<models::HistoricalData> {
        self.historical_data_cache.lock().unwrap().get(&req_id).cloned()
    }

    // ========================================================================
    // Account APIs
    // ========================================================================

    pub fn request_account_summary(&self) -> Result<(), String> {
        let client = self.client()?;
        let group = ibapi::accounts::types::AccountGroup("All".into());

        match client.account_summary(&group, AccountSummaryTags::ALL) {
            Ok(subscription) => {
                let mut summary_map = self.account_summary.lock().unwrap();
                for item in subscription {
                    if let AccountSummaryResult::Summary(s) = item {
                        let entry = summary_map
                            .entry(s.account.clone())
                            .or_insert_with(|| models::AccountSummary {
                                account: s.account.clone(),
                                values: HashMap::new(),
                            });
                        entry.values.insert(s.tag.clone(), s.value.clone());
                    }
                }
                Ok(())
            }
            Err(e) => Err(format!("Account summary request failed: {e}")),
        }
    }

    pub fn get_account_summary(&self, account: &str) -> Option<models::AccountSummary> {
        let map = self.account_summary.lock().unwrap();
        if account.is_empty() {
            map.values().next().cloned()
        } else {
            map.get(account).cloned()
        }
    }

    pub fn request_positions(&self) -> Result<(), String> {
        let client = self.client()?;

        match client.positions() {
            Ok(subscription) => {
                let mut positions = self.positions.lock().unwrap();
                positions.clear();
                for item in subscription {
                    if let PositionUpdate::Position(p) = item {
                        positions.push(models::Position {
                            account: p.account.clone(),
                            symbol: p.contract.symbol.to_string(),
                            sec_type: p.contract.security_type.to_string(),
                            currency: p.contract.currency.to_string(),
                            exchange: p.contract.exchange.to_string(),
                            position: p.position,
                            avg_cost: p.average_cost,
                            ..Default::default()
                        });
                    }
                }
                Ok(())
            }
            Err(e) => Err(format!("Positions request failed: {e}")),
        }
    }

    pub fn get_all_positions(&self) -> Vec<models::Position> {
        self.positions.lock().unwrap().clone()
    }

    pub fn get_positions_by_account(&self, account: &str) -> Vec<models::Position> {
        self.positions
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.account == account)
            .cloned()
            .collect()
    }

    pub fn get_positions_by_symbol(&self, symbol: &str, sec_type: &str) -> Vec<models::Position> {
        self.positions
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.symbol == symbol && p.sec_type == sec_type)
            .cloned()
            .collect()
    }

    // ========================================================================
    // Order APIs
    // ========================================================================

    pub fn place_market_order(
        &self,
        spec: &models::ContractSpec,
        action: &str,
        quantity: f64,
    ) -> Result<i64, String> {
        let client = self.client()?;
        let contract = build_contract(spec);
        let order_id = client.next_order_id();
        let order = orders::order_builder::market_order(parse_action(action), quantity);

        client
            .submit_order(order_id, &contract, &order)
            .map_err(|e| format!("Place order failed: {e}"))?;

        self.store_order(order_id as i64, spec, action, "MKT", quantity, 0.0, 0.0);
        tracing::info!(
            "Market order placed: id={order_id}, symbol={}, action={action}, qty={quantity}",
            spec.symbol
        );
        Ok(order_id as i64)
    }

    pub fn place_limit_order(
        &self,
        spec: &models::ContractSpec,
        action: &str,
        quantity: f64,
        limit_price: f64,
    ) -> Result<i64, String> {
        let client = self.client()?;
        let contract = build_contract(spec);
        let order_id = client.next_order_id();
        let order = orders::order_builder::limit_order(parse_action(action), quantity, limit_price);

        client
            .submit_order(order_id, &contract, &order)
            .map_err(|e| format!("Place order failed: {e}"))?;

        self.store_order(order_id as i64, spec, action, "LMT", quantity, limit_price, 0.0);
        tracing::info!(
            "Limit order placed: id={order_id}, symbol={}, price={limit_price}",
            spec.symbol
        );
        Ok(order_id as i64)
    }

    pub fn place_stop_order(
        &self,
        spec: &models::ContractSpec,
        action: &str,
        quantity: f64,
        stop_price: f64,
    ) -> Result<i64, String> {
        let client = self.client()?;
        let contract = build_contract(spec);
        let order_id = client.next_order_id();
        let order = orders::order_builder::stop(parse_action(action), quantity, stop_price);

        client
            .submit_order(order_id, &contract, &order)
            .map_err(|e| format!("Place order failed: {e}"))?;

        self.store_order(order_id as i64, spec, action, "STP", quantity, 0.0, stop_price);
        tracing::info!(
            "Stop order placed: id={order_id}, symbol={}, stop={stop_price}",
            spec.symbol
        );
        Ok(order_id as i64)
    }

    pub fn cancel_order(&self, order_id: i64) -> Result<(), String> {
        let client = self.client()?;
        let _ = client
            .cancel_order(order_id as i32, "")
            .map_err(|e| format!("Cancel order failed: {e}"))?;
        tracing::info!("Order cancellation requested: id={order_id}");
        Ok(())
    }

    pub fn get_order(&self, order_id: i64) -> Option<models::OrderInfo> {
        self.order_map.lock().unwrap().get(&order_id).cloned()
    }

    pub fn get_all_orders(&self) -> Vec<models::OrderInfo> {
        self.order_map.lock().unwrap().values().cloned().collect()
    }

    pub fn get_orders_by_status(&self, status: &str) -> Vec<models::OrderInfo> {
        self.order_map
            .lock()
            .unwrap()
            .values()
            .filter(|o| o.status == status)
            .cloned()
            .collect()
    }

    pub fn get_orders_by_symbol(&self, symbol: &str, sec_type: &str) -> Vec<models::OrderInfo> {
        self.order_map
            .lock()
            .unwrap()
            .values()
            .filter(|o| o.symbol == symbol && o.sec_type == sec_type)
            .cloned()
            .collect()
    }

    // ========================================================================
    // Utility
    // ========================================================================

    pub fn get_managed_accounts(&self) -> Vec<String> {
        self.managed_accounts.lock().unwrap().clone()
    }

    fn store_order(
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
        self.order_map.lock().unwrap().insert(order_id, info);
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn build_contract(spec: &models::ContractSpec) -> Contract {
    match spec.sec_type.as_str() {
        "OPT" => {
            let strike = spec.strike.unwrap_or(0.0);
            let expiry = spec.expiry.as_deref().unwrap_or("20260101");
            let (y, m, d) = parse_expiry_date(expiry).unwrap_or((2026, 1, 1));

            let base_builder = if spec.right.as_deref() == Some("P") {
                Contract::put(&spec.symbol)
            } else {
                Contract::call(&spec.symbol)
            };

            let mut builder = base_builder.strike(strike).expires_on(y as u16, m, d);
            if !spec.exchange.is_empty() && spec.exchange != "SMART" {
                builder = builder.on_exchange(&spec.exchange);
            }
            if !spec.currency.is_empty() && spec.currency != "USD" {
                builder = builder.in_currency(&spec.currency);
            }
            builder.build()
        }
        "FUT" => {
            let mut builder = Contract::futures(&spec.symbol).front_month();
            if !spec.exchange.is_empty() && spec.exchange != "SMART" {
                builder = builder.on_exchange(&spec.exchange);
            }
            if !spec.currency.is_empty() && spec.currency != "USD" {
                builder = builder.in_currency(&spec.currency);
            }
            builder.build()
        }
        "CASH" => Contract::forex(&spec.symbol, &spec.currency).build(),
        _ => {
            let mut builder = Contract::stock(&spec.symbol);
            if !spec.exchange.is_empty() && spec.exchange != "SMART" {
                builder = builder.on_exchange(&spec.exchange);
            }
            if !spec.currency.is_empty() && spec.currency != "USD" {
                builder = builder.in_currency(&spec.currency);
            }
            builder.build()
        }
    }
}

fn contract_key(symbol: &str, sec_type: &str) -> String {
    format!("{symbol}:{sec_type}")
}

fn parse_action(action: &str) -> orders::Action {
    match action.to_uppercase().as_str() {
        "SELL" => orders::Action::Sell,
        _ => orders::Action::Buy,
    }
}

fn parse_expiry_date(s: &str) -> Option<(i32, u8, u8)> {
    if s.len() < 8 {
        return None;
    }
    let year = s[0..4].parse::<i32>().ok()?;
    let month = s[4..6].parse::<u8>().ok()?;
    let day = s[6..8].parse::<u8>().ok()?;
    Some((year, month, day))
}

fn parse_duration(s: &str) -> historical::Duration {
    let parts: Vec<&str> = s.trim().split_whitespace().collect();
    if parts.len() != 2 {
        return 1_i32.days();
    }
    let n: i32 = parts[0].parse().unwrap_or(1);
    match parts[1].to_uppercase().as_str() {
        "S" => n.seconds(),
        "D" => n.days(),
        "W" => n.weeks(),
        "M" => n.months(),
        "Y" => n.years(),
        _ => n.days(),
    }
}

fn parse_bar_size(s: &str) -> historical::BarSize {
    match s.to_lowercase().as_str() {
        "1 secs" | "1 sec" => HistoricalBarSize::Sec,
        "5 secs" => HistoricalBarSize::Sec5,
        "10 secs" => HistoricalBarSize::Sec10,
        "15 secs" => HistoricalBarSize::Sec15,
        "30 secs" => HistoricalBarSize::Sec30,
        "1 min" => HistoricalBarSize::Min,
        "2 mins" => HistoricalBarSize::Min2,
        "3 mins" => HistoricalBarSize::Min3,
        "5 mins" => HistoricalBarSize::Min5,
        "10 mins" => HistoricalBarSize::Min10,
        "15 mins" => HistoricalBarSize::Min15,
        "20 mins" => HistoricalBarSize::Min20,
        "30 mins" => HistoricalBarSize::Min30,
        "1 hour" => HistoricalBarSize::Hour,
        "2 hours" => HistoricalBarSize::Hour2,
        "3 hours" => HistoricalBarSize::Hour3,
        "4 hours" => HistoricalBarSize::Hour4,
        "8 hours" => HistoricalBarSize::Hour8,
        "1 day" => HistoricalBarSize::Day,
        "1 week" | "1 w" => HistoricalBarSize::Week,
        "1 month" | "1 m" => HistoricalBarSize::Month,
        _ => HistoricalBarSize::Hour,
    }
}

fn parse_what_to_show(s: &str) -> historical::WhatToShow {
    match s.to_uppercase().as_str() {
        "TRADES" => HistoricalWhatToShow::Trades,
        "MIDPOINT" => HistoricalWhatToShow::MidPoint,
        "BID" => HistoricalWhatToShow::Bid,
        "ASK" => HistoricalWhatToShow::Ask,
        "BID_ASK" => HistoricalWhatToShow::BidAsk,
        "HISTORICAL_VOLATILITY" => HistoricalWhatToShow::HistoricalVolatility,
        "OPTION_IMPLIED_VOLATILITY" => HistoricalWhatToShow::OptionImpliedVolatility,
        _ => HistoricalWhatToShow::Trades,
    }
}

fn parse_ib_datetime(s: &str) -> Option<::time::OffsetDateTime> {
    use ::time::format_description;
    let fmt = format_description::parse("[year][month][day] [hour]:[minute]:[second]").ok()?;
    let pdt = ::time::PrimitiveDateTime::parse(s, &fmt).ok()?;
    Some(pdt.assume_utc())
}
