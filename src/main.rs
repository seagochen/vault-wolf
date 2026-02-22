//! VaultWolf Trading API Server
//!
//! A REST API server that connects to Interactive Brokers TWS/Gateway,
//! providing endpoints for market data, account info, and order management.
//!
//! Migrated from C++ (IBTwsApi + cpp-httplib + IntelRDFPMathLib) to pure Rust.

mod manager;
mod models;
mod web;

use std::sync::Arc;

use clap::Parser;
use tokio::sync::Mutex;

/// VaultWolf Trading API Server — Interactive Brokers integration.
#[derive(Parser, Debug)]
#[command(name = "vault-wolf", version = "1.0.0")]
struct Args {
    /// IB TWS/Gateway host
    #[arg(long = "ib-host", default_value = "127.0.0.1")]
    ib_host: String,

    /// IB TWS/Gateway port
    #[arg(long = "ib-port", default_value_t = 4002)]
    ib_port: u16,

    /// IB client ID
    #[arg(long = "ib-client-id", default_value_t = 0)]
    ib_client_id: i32,

    /// Web server port
    #[arg(long = "web-port", default_value_t = 5000)]
    web_port: u16,
}

fn print_banner() {
    println!("========================================");
    println!("   VaultWolf Trading API Server");
    println!("   Interactive Brokers Integration");
    println!("   Version 1.0.0  (Rust)");
    println!("========================================");
    println!();
}

fn print_endpoints(port: u16) {
    println!();
    println!("========================================");
    println!("   VaultWolf Server is READY!");
    println!("========================================");
    println!();
    println!("API Endpoints:");
    println!("  Health Check:     GET  http://localhost:{port}/health");
    println!();
    println!("  Market Data:");
    println!("    Real-time:      GET  http://localhost:{port}/api/market/realtime?symbol=SPY");
    println!("    Historical:     GET  http://localhost:{port}/api/market/historical?symbol=SPY&duration=1%20D");
    println!("    Subscribe:      POST http://localhost:{port}/api/market/subscribe");
    println!("    Unsubscribe:    POST http://localhost:{port}/api/market/unsubscribe");
    println!();
    println!("  Account:");
    println!("    Summary:        GET  http://localhost:{port}/api/account/summary");
    println!("    Positions:      GET  http://localhost:{port}/api/account/positions");
    println!();
    println!("  Orders:");
    println!("    Place Order:    POST http://localhost:{port}/api/order/place");
    println!("    Cancel Order:   POST http://localhost:{port}/api/order/cancel");
    println!("    Modify Order:   POST http://localhost:{port}/api/order/modify");
    println!("    List Orders:    GET  http://localhost:{port}/api/order/list");
    println!("    Get Order:      GET  http://localhost:{port}/api/order/{{id}}");
    println!();
    println!("Press Ctrl+C to stop the server...");
    println!();
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args = Args::parse();
    print_banner();

    // Create manager and connect to IB
    let mut manager = manager::VaultWolfManager::new();

    println!(
        "Connecting to IB TWS/Gateway at {}:{} (client ID: {})...",
        args.ib_host, args.ib_port, args.ib_client_id
    );

    if let Err(e) = manager.connect_to_ib(&args.ib_host, args.ib_port, args.ib_client_id).await {
        eprintln!("Failed to connect to IB TWS/Gateway!");
        eprintln!("  Error: {e}");
        eprintln!("Please ensure:");
        eprintln!("  1. TWS or IB Gateway is running");
        eprintln!("  2. API connections are enabled in TWS/Gateway settings");
        eprintln!("  3. The host and port are correct");
        std::process::exit(1);
    }

    println!("Successfully connected to IB!");

    // Show managed accounts
    let accounts = manager.get_managed_accounts().await;
    if !accounts.is_empty() {
        println!("Managed accounts: {}", accounts.join(", "));
    }

    // Wrap in Arc<Mutex> for shared state
    let shared_manager = Arc::new(Mutex::new(manager));

    // Set up graceful shutdown
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let shutdown_tx = Arc::new(std::sync::Mutex::new(Some(shutdown_tx)));

    ctrlc::set_handler(move || {
        println!("\nReceived Ctrl+C, shutting down gracefully...");
        if let Some(tx) = shutdown_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
    })
    .expect("Failed to set Ctrl+C handler");

    // Build router
    let app = web::create_router(shared_manager.clone());

    print_endpoints(args.web_port);

    // Start server
    let addr = format!("0.0.0.0:{}", args.web_port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind to address");

    tracing::info!("Listening on {addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        })
        .await
        .expect("Server error");

    // Cleanup
    println!("Shutting down...");
    {
        let mut m = shared_manager.lock().await;
        m.disconnect_from_ib().await;
    }
    println!("Shutdown complete. Goodbye!");
}
