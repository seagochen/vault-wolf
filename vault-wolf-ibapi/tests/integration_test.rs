//! Integration tests for vault-wolf-ibapi.
//!
//! These tests require a running IB TWS/Gateway instance (Paper Trading recommended).
//! They are ignored by default and can be run with:
//!
//! ```bash
//! cargo test -p vault-wolf-ibapi --test integration_test -- --ignored
//! ```
//!
//! Configuration via environment variables:
//!   IB_HOST    (default: 127.0.0.1)
//!   IB_PORT    (default: 4002)
//!   IB_CLIENT  (default: 100)

use std::time::Duration;
use vault_wolf_ibapi::{
    Contract, IBClient, IBEvent, OrderType, SecType, Action, Order, OrderCancel,
};

fn ib_host() -> String {
    std::env::var("IB_HOST").unwrap_or_else(|_| "127.0.0.1".into())
}

fn ib_port() -> u16 {
    std::env::var("IB_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4002)
}

fn ib_client_id() -> i32 {
    std::env::var("IB_CLIENT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100)
}

fn spy_contract() -> Contract {
    Contract {
        symbol: "SPY".into(),
        sec_type: Some(SecType::Stock),
        exchange: "SMART".into(),
        currency: "USD".into(),
        ..Contract::default()
    }
}

// ============================================================================
// Connection Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_connect_and_disconnect() {
    let (mut client, mut rx) = IBClient::connect(&ib_host(), ib_port(), ib_client_id(), None)
        .await
        .expect("Failed to connect to IB");

    // Should receive NextValidId and ManagedAccounts events
    let mut got_next_id = false;
    let mut got_managed_accounts = false;

    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(IBEvent::NextValidId { order_id }) => {
                        assert!(order_id > 0, "Next valid order ID should be positive");
                        got_next_id = true;
                        println!("NextValidId: {order_id}");
                    }
                    Some(IBEvent::ManagedAccounts { accounts }) => {
                        assert!(!accounts.is_empty(), "Should have at least one managed account");
                        got_managed_accounts = true;
                        println!("ManagedAccounts: {accounts}");
                    }
                    Some(other) => {
                        println!("Other event: {other:?}");
                    }
                    None => break,
                }
                if got_next_id && got_managed_accounts {
                    break;
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    assert!(got_next_id, "Should have received NextValidId");
    assert!(got_managed_accounts, "Should have received ManagedAccounts");

    client.disconnect().await;
}

#[tokio::test]
#[ignore]
async fn test_connect_wrong_port() {
    let result = IBClient::connect(&ib_host(), 19999, ib_client_id(), None).await;
    assert!(result.is_err(), "Connection to wrong port should fail");
}

// ============================================================================
// Market Data Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_request_market_data() {
    let (mut client, mut rx) = IBClient::connect(&ib_host(), ib_port(), ib_client_id() + 1, None)
        .await
        .expect("Failed to connect");

    // Wait for initial events
    tokio::time::sleep(Duration::from_millis(500)).await;
    // Drain initial events
    while rx.try_recv().is_ok() {}

    let contract = spy_contract();
    let req_id = 9001;

    client
        .req_mkt_data(req_id, &contract, "", false, false, &[])
        .await
        .expect("req_mkt_data failed");

    // Collect tick events for a few seconds
    let mut tick_count = 0;
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(IBEvent::TickPrice { req_id: rid, .. })
                    | Some(IBEvent::TickSize { req_id: rid, .. })
                    | Some(IBEvent::TickString { req_id: rid, .. })
                    | Some(IBEvent::TickGeneric { req_id: rid, .. }) if rid == req_id => {
                        tick_count += 1;
                    }
                    Some(IBEvent::Error { req_id: rid, code, message, .. }) if rid == req_id => {
                        println!("Market data error: code={code}, msg={message}");
                        break;
                    }
                    _ => {}
                }
                if tick_count >= 5 {
                    break;
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    // Cancel subscription
    let _ = client.cancel_mkt_data(req_id).await;

    println!("Received {tick_count} tick events");
    assert!(tick_count > 0, "Should have received at least one tick event");

    client.disconnect().await;
}

// ============================================================================
// Historical Data Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_request_historical_data() {
    let (mut client, mut rx) = IBClient::connect(&ib_host(), ib_port(), ib_client_id() + 2, None)
        .await
        .expect("Failed to connect");

    // Wait for initial events
    tokio::time::sleep(Duration::from_millis(500)).await;
    while rx.try_recv().is_ok() {}

    let contract = spy_contract();
    let req_id = 9002;

    client
        .req_historical_data(
            req_id,
            &contract,
            "",      // end_date_time (empty = now)
            "1 D",   // duration
            "1 hour", // bar_size
            "TRADES", // what_to_show
            true,     // use_rth
            1,        // format_date
            false,    // keep_up_to_date
            &[],      // chart_options
        )
        .await
        .expect("req_historical_data failed");

    // Wait for HistoricalData + HistoricalDataEnd
    let mut bars = Vec::new();
    let mut got_end = false;

    let timeout = tokio::time::sleep(Duration::from_secs(30));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(IBEvent::HistoricalData { req_id: rid, bars: batch }) if rid == req_id => {
                        bars.extend(batch);
                    }
                    Some(IBEvent::HistoricalDataEnd { req_id: rid, .. }) if rid == req_id => {
                        got_end = true;
                        break;
                    }
                    Some(IBEvent::Error { req_id: rid, code, message, .. }) if rid == req_id => {
                        panic!("Historical data error: code={code}, msg={message}");
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    assert!(got_end, "Should have received HistoricalDataEnd");
    assert!(!bars.is_empty(), "Should have received historical bars");

    println!("Received {} bars", bars.len());
    for bar in &bars {
        println!(
            "  {} O={:.2} H={:.2} L={:.2} C={:.2} V={:?}",
            bar.time, bar.open, bar.high, bar.low, bar.close, bar.volume
        );
    }

    client.disconnect().await;
}

// ============================================================================
// Account Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_request_account_summary() {
    let (mut client, mut rx) = IBClient::connect(&ib_host(), ib_port(), ib_client_id() + 3, None)
        .await
        .expect("Failed to connect");

    tokio::time::sleep(Duration::from_millis(500)).await;
    while rx.try_recv().is_ok() {}

    let req_id = 9003;
    let tags = "NetLiquidation,TotalCashValue,BuyingPower";

    client
        .req_account_summary(req_id, "All", tags)
        .await
        .expect("req_account_summary failed");

    let mut entries = Vec::new();
    let mut got_end = false;

    let timeout = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(IBEvent::AccountSummary { req_id: rid, account, tag, value, currency }) if rid == req_id => {
                        println!("  {account}: {tag} = {value} {currency}");
                        entries.push((account, tag, value));
                    }
                    Some(IBEvent::AccountSummaryEnd { req_id: rid }) if rid == req_id => {
                        got_end = true;
                        break;
                    }
                    Some(IBEvent::Error { req_id: rid, code, message, .. }) if rid == req_id => {
                        panic!("Account summary error: code={code}, msg={message}");
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    // Cancel subscription
    let _ = client.cancel_account_summary(req_id).await;

    assert!(got_end, "Should have received AccountSummaryEnd");
    assert!(!entries.is_empty(), "Should have received account summary entries");

    println!("Received {} account summary entries", entries.len());

    client.disconnect().await;
}

#[tokio::test]
#[ignore]
async fn test_request_positions() {
    let (mut client, mut rx) = IBClient::connect(&ib_host(), ib_port(), ib_client_id() + 4, None)
        .await
        .expect("Failed to connect");

    tokio::time::sleep(Duration::from_millis(500)).await;
    while rx.try_recv().is_ok() {}

    client
        .req_positions()
        .await
        .expect("req_positions failed");

    let mut positions = Vec::new();
    let mut got_end = false;

    let timeout = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(IBEvent::Position { account, contract, position, avg_cost }) => {
                        println!(
                            "  {account}: {} {} pos={position} avg_cost={avg_cost:.2}",
                            contract.symbol,
                            contract.sec_type.as_ref().map(|s| s.to_string()).unwrap_or_default(),
                        );
                        positions.push((account, contract.symbol.clone(), position));
                    }
                    Some(IBEvent::PositionEnd) => {
                        got_end = true;
                        break;
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    let _ = client.cancel_positions().await;

    assert!(got_end, "Should have received PositionEnd");
    // Note: positions may be empty if no positions held
    println!("Received {} positions", positions.len());

    client.disconnect().await;
}

// ============================================================================
// Order Tests (Paper Trading only!)
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_place_and_cancel_limit_order() {
    let (mut client, mut rx) = IBClient::connect(&ib_host(), ib_port(), ib_client_id() + 5, None)
        .await
        .expect("Failed to connect");

    // Wait for NextValidId
    let mut order_id: i64 = -1;
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = rx.recv() => {
                if let Some(IBEvent::NextValidId { order_id: id }) = event {
                    order_id = id;
                    break;
                }
            }
            _ = &mut timeout => {
                panic!("Timed out waiting for NextValidId");
            }
        }
    }

    assert!(order_id > 0, "Should have valid order ID");

    let contract = spy_contract();

    // Place a limit order far from market (won't fill)
    let order = Order {
        action: Some(Action::Buy),
        total_quantity: Some(rust_decimal::Decimal::ONE),
        order_type: Some(OrderType::Limit),
        lmt_price: Some(1.00), // Absurdly low — won't fill
        ..Order::default()
    };

    client
        .place_order(order_id, &contract, &order)
        .await
        .expect("place_order failed");

    // Wait for OrderStatus
    let mut got_status = false;
    let timeout = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(IBEvent::OrderStatus { order_id: oid, status, .. }) if oid == order_id => {
                        println!("Order {oid} status: {status}");
                        got_status = true;
                        break;
                    }
                    Some(IBEvent::Error { code, message, .. }) => {
                        println!("Error: code={code}, msg={message}");
                        // Some errors are expected (e.g., order confirmation)
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }

    assert!(got_status, "Should have received OrderStatus");

    // Cancel the order
    client
        .cancel_order(order_id, &OrderCancel::default())
        .await
        .expect("cancel_order failed");

    // Wait for cancellation status
    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(IBEvent::OrderStatus { order_id: oid, status, .. }) if oid == order_id => {
                        println!("After cancel — Order {oid} status: {status}");
                        if status.contains("Cancel") {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            _ = &mut timeout => {
                println!("Timed out waiting for cancel confirmation (may be normal)");
                break;
            }
        }
    }

    client.disconnect().await;
}
