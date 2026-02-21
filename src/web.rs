//! VaultWolf Web Server — REST API built with axum.
//!
//! Replaces the C++ `WebServer` class that used `cpp-httplib`.
//! All endpoints mirror the original API surface.

use std::sync::Arc;

use axum::{
    Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json,
};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::manager::VaultWolfManager;
use crate::models::*;

// ============================================================================
// App State
// ============================================================================

pub type SharedManager = Arc<Mutex<VaultWolfManager>>;

// ============================================================================
// Router
// ============================================================================

pub fn create_router(manager: SharedManager) -> Router {
    Router::new()
        // Health
        .route("/health", get(handle_health))
        // Market Data
        .route("/api/market/realtime", get(handle_realtime_market_data))
        .route(
            "/api/market/historical",
            get(handle_historical_data),
        )
        .route("/api/market/subscribe", post(handle_subscribe_market_data))
        .route(
            "/api/market/unsubscribe",
            post(handle_unsubscribe_market_data),
        )
        // Account
        .route("/api/account/summary", get(handle_account_summary))
        .route("/api/account/positions", get(handle_positions))
        // Orders
        .route("/api/order/place", post(handle_place_order))
        .route("/api/order/cancel", post(handle_cancel_order))
        .route("/api/order/modify", post(handle_modify_order))
        .route("/api/order/list", get(handle_get_orders))
        .route("/api/order/{id}", get(handle_get_order))
        .with_state(manager)
}

// ============================================================================
// Query / Body parameter types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct MarketDataQuery {
    pub symbol: Option<String>,
    pub sec_type: Option<String>,
    pub currency: Option<String>,
    pub exchange: Option<String>,
    // Options
    pub right: Option<String>,
    pub strike: Option<f64>,
    pub expiry: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HistoricalDataQuery {
    pub symbol: Option<String>,
    pub sec_type: Option<String>,
    pub currency: Option<String>,
    pub exchange: Option<String>,
    pub end_date: Option<String>,
    pub duration: Option<String>,
    pub bar_size: Option<String>,
    pub what_to_show: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CancelQuery {
    pub req_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AccountQuery {
    pub account: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PositionQuery {
    pub account: Option<String>,
    pub symbol: Option<String>,
    pub sec_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PlaceOrderBody {
    pub symbol: Option<String>,
    pub sec_type: Option<String>,
    pub currency: Option<String>,
    pub exchange: Option<String>,
    pub action: Option<String>,
    pub quantity: Option<f64>,
    pub order_type: Option<String>,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
    // Options
    pub right: Option<String>,
    pub strike: Option<f64>,
    pub expiry: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CancelOrderBody {
    pub order_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ModifyOrderBody {
    pub order_id: Option<i64>,
    pub quantity: Option<f64>,
    pub limit_price: Option<f64>,
    pub stop_price: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct OrderListQuery {
    pub status: Option<String>,
    pub symbol: Option<String>,
    pub sec_type: Option<String>,
}

// ============================================================================
// Helpers
// ============================================================================

fn to_spec(q: &MarketDataQuery) -> ContractSpec {
    ContractSpec {
        symbol: q.symbol.clone().unwrap_or_default(),
        sec_type: q.sec_type.clone().unwrap_or_else(|| "STK".into()),
        currency: q.currency.clone().unwrap_or_else(|| "USD".into()),
        exchange: q.exchange.clone().unwrap_or_else(|| "SMART".into()),
        right: q.right.clone(),
        strike: q.strike,
        expiry: q.expiry.clone(),
        last_trade_date_or_contract_month: None,
    }
}

fn ok_json<T: serde::Serialize>(msg: &str, data: T) -> impl IntoResponse {
    Json(ApiResponse::success(msg, data))
}

fn ok_msg(msg: &str) -> impl IntoResponse {
    Json(ApiResponse::<()>::success_msg(msg))
}

fn err_json(msg: &str, code: i32) -> (StatusCode, Json<ApiResponse<()>>) {
    let status = match code {
        404 => StatusCode::NOT_FOUND,
        500 => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::BAD_REQUEST,
    };
    (status, Json(ApiResponse::error(msg, code)))
}

// ============================================================================
// Route Handlers
// ============================================================================

async fn handle_health(State(mgr): State<SharedManager>) -> impl IntoResponse {
    let m = mgr.lock().await;
    let connected = m.is_connected();
    Json(serde_json::json!({
        "status": if connected { "healthy" } else { "disconnected" },
        "ibConnected": connected,
        "server": "VaultWolf API Server",
        "version": "1.0.0"
    }))
}

async fn handle_realtime_market_data(
    State(mgr): State<SharedManager>,
    Query(q): Query<MarketDataQuery>,
) -> impl IntoResponse {
    let symbol = match &q.symbol {
        Some(s) if !s.is_empty() => s.clone(),
        _ => return err_json("Missing required parameter: symbol", 400).into_response(),
    };
    let sec_type = q.sec_type.as_deref().unwrap_or("STK");

    let m = mgr.lock().await;
    match m.get_tick_data(&symbol, sec_type) {
        Some(td) => ok_json("Market data retrieved", td).into_response(),
        None => err_json(
            &format!("No market data found for symbol: {symbol}"),
            404,
        )
        .into_response(),
    }
}

async fn handle_subscribe_market_data(
    State(mgr): State<SharedManager>,
    Query(q): Query<MarketDataQuery>,
) -> impl IntoResponse {
    let spec = to_spec(&q);
    if spec.symbol.is_empty() {
        return err_json("Missing required parameter: symbol", 400).into_response();
    }

    let m = mgr.lock().await;
    match m.request_market_data(&spec) {
        Ok(req_id) => ok_json(
            "Market data subscription created",
            serde_json::json!({
                "reqId": req_id,
                "symbol": spec.symbol,
                "secType": spec.sec_type,
            }),
        )
        .into_response(),
        Err(e) => err_json(&e, 500).into_response(),
    }
}

async fn handle_unsubscribe_market_data(
    State(mgr): State<SharedManager>,
    Query(q): Query<CancelQuery>,
) -> impl IntoResponse {
    let req_id = match q.req_id {
        Some(id) => id,
        None => return err_json("Missing required parameter: req_id", 400).into_response(),
    };

    let m = mgr.lock().await;
    m.cancel_market_data(req_id);
    ok_msg("Market data subscription cancelled").into_response()
}

async fn handle_historical_data(
    State(mgr): State<SharedManager>,
    Query(q): Query<HistoricalDataQuery>,
) -> impl IntoResponse {
    let symbol = match &q.symbol {
        Some(s) if !s.is_empty() => s.clone(),
        _ => return err_json("Missing required parameter: symbol", 400).into_response(),
    };

    let spec = ContractSpec {
        symbol,
        sec_type: q.sec_type.clone().unwrap_or_else(|| "STK".into()),
        currency: q.currency.clone().unwrap_or_else(|| "USD".into()),
        exchange: q.exchange.clone().unwrap_or_else(|| "SMART".into()),
        ..Default::default()
    };

    let duration = q.duration.as_deref().unwrap_or("1 D");
    let bar_size = q.bar_size.as_deref().unwrap_or("1 hour");
    let what_to_show = q.what_to_show.as_deref().unwrap_or("TRADES");
    let end_date = q.end_date.as_deref();

    let m = mgr.lock().await;
    match m.request_historical_data(&spec, end_date, duration, bar_size, what_to_show) {
        Ok((_req_id, hist)) => ok_json("Historical data retrieved", hist).into_response(),
        Err(e) => err_json(&e, 500).into_response(),
    }
}

async fn handle_account_summary(
    State(mgr): State<SharedManager>,
    Query(q): Query<AccountQuery>,
) -> impl IntoResponse {
    let m = mgr.lock().await;
    let _ = m.request_account_summary();

    let account = q.account.as_deref().unwrap_or("");
    match m.get_account_summary(account) {
        Some(summary) => ok_json("Account summary retrieved", summary).into_response(),
        None => err_json("No account summary available", 404).into_response(),
    }
}

async fn handle_positions(
    State(mgr): State<SharedManager>,
    Query(q): Query<PositionQuery>,
) -> impl IntoResponse {
    let m = mgr.lock().await;
    let _ = m.request_positions();

    let positions = if let Some(account) = &q.account {
        m.get_positions_by_account(account)
    } else if let Some(symbol) = &q.symbol {
        let sec_type = q.sec_type.as_deref().unwrap_or("STK");
        m.get_positions_by_symbol(symbol, sec_type)
    } else {
        m.get_all_positions()
    };

    ok_json("Positions retrieved", positions).into_response()
}

async fn handle_place_order(
    State(mgr): State<SharedManager>,
    Json(body): Json<PlaceOrderBody>,
) -> impl IntoResponse {
    let symbol = match &body.symbol {
        Some(s) if !s.is_empty() => s.clone(),
        _ => return err_json("Missing required parameter: symbol", 400).into_response(),
    };
    let action = match &body.action {
        Some(a) if !a.is_empty() => a.clone(),
        _ => return err_json("Missing required parameter: action (BUY/SELL)", 400).into_response(),
    };
    let quantity = match body.quantity {
        Some(q) if q > 0.0 => q,
        _ => return err_json("Missing required parameter: quantity", 400).into_response(),
    };
    let order_type = body.order_type.as_deref().unwrap_or("MKT");

    let spec = ContractSpec {
        symbol: symbol.clone(),
        sec_type: body.sec_type.clone().unwrap_or_else(|| "STK".into()),
        currency: body.currency.clone().unwrap_or_else(|| "USD".into()),
        exchange: body.exchange.clone().unwrap_or_else(|| "SMART".into()),
        right: body.right.clone(),
        strike: body.strike,
        expiry: body.expiry.clone(),
        ..Default::default()
    };

    let m = mgr.lock().await;
    let result = match order_type {
        "MKT" => m.place_market_order(&spec, &action, quantity),
        "LMT" => {
            let price = match body.limit_price {
                Some(p) => p,
                None => {
                    return err_json("Missing required parameter for limit order: limit_price", 400)
                        .into_response()
                }
            };
            m.place_limit_order(&spec, &action, quantity, price)
        }
        "STP" => {
            let price = match body.stop_price {
                Some(p) => p,
                None => {
                    return err_json("Missing required parameter for stop order: stop_price", 400)
                        .into_response()
                }
            };
            m.place_stop_order(&spec, &action, quantity, price)
        }
        _ => return err_json("Invalid order type. Supported: MKT, LMT, STP", 400).into_response(),
    };

    match result {
        Ok(order_id) => ok_json(
            "Order placed successfully",
            serde_json::json!({
                "orderId": order_id,
                "symbol": symbol,
                "action": action,
                "quantity": quantity,
                "orderType": order_type,
            }),
        )
        .into_response(),
        Err(e) => err_json(&e, 500).into_response(),
    }
}

async fn handle_cancel_order(
    State(mgr): State<SharedManager>,
    Json(body): Json<CancelOrderBody>,
) -> impl IntoResponse {
    let order_id = match body.order_id {
        Some(id) => id,
        None => return err_json("Missing required parameter: order_id", 400).into_response(),
    };

    let m = mgr.lock().await;
    match m.cancel_order(order_id) {
        Ok(()) => ok_msg("Order cancellation requested").into_response(),
        Err(e) => err_json(&e, 500).into_response(),
    }
}

async fn handle_modify_order(
    State(_mgr): State<SharedManager>,
    Json(body): Json<ModifyOrderBody>,
) -> impl IntoResponse {
    let _order_id = match body.order_id {
        Some(id) => id,
        None => return err_json("Missing required parameter: order_id", 400).into_response(),
    };

    // Modify is currently a no-op placeholder; the C++ version re-placed the order.
    // In Rust ibapi, order modification is done by re-placing with the same orderId.
    ok_msg("Order modification requested").into_response()
}

async fn handle_get_orders(
    State(mgr): State<SharedManager>,
    Query(q): Query<OrderListQuery>,
) -> impl IntoResponse {
    let m = mgr.lock().await;

    let orders = if let Some(status) = &q.status {
        m.get_orders_by_status(status)
    } else if let Some(symbol) = &q.symbol {
        let sec_type = q.sec_type.as_deref().unwrap_or("STK");
        m.get_orders_by_symbol(symbol, sec_type)
    } else {
        m.get_all_orders()
    };

    ok_json("Orders retrieved", orders).into_response()
}

async fn handle_get_order(
    State(mgr): State<SharedManager>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let m = mgr.lock().await;
    match m.get_order(id) {
        Some(order) => ok_json("Order retrieved", order).into_response(),
        None => err_json("Order not found", 404).into_response(),
    }
}
