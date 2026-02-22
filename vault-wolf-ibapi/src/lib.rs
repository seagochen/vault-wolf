//! vault-wolf-ibapi -- Rust native Interactive Brokers TWS API client library.
//!
//! This crate provides a complete port of the C++ IB TWS API, including all
//! data structures, protocol constants, and error types. It is designed as the
//! foundation for building a full IB API client in Rust.
//!
//! ## Modules
//!
//! - [`models`] -- All IB API data structures (Contract, Order, Execution, etc.)
//! - [`protocol`] -- Protocol constants, message IDs, server version requirements
//! - [`errors`] -- Error types for the library
//! - [`encoder`] -- Wire-format message encoding
//! - [`decoder`] -- Wire-format message decoding + server message dispatch
//! - [`transport`] -- Async TCP transport with V100+ framing
//! - [`wrapper`] -- IBEvent enum (all server callback events)
//! - [`reader`] -- Async message reader (spawned tokio task)
//! - [`client`] -- IBClient (main API entry point)

pub mod client;
pub mod decoder;
pub mod encoder;
pub mod errors;
mod generated;
pub mod models;
pub mod proto_decode;
pub mod proto_encode;
pub mod protocol;
pub mod reader;
pub mod transport;
pub mod wrapper;

// ============================================================================
// Re-exports for convenience
// ============================================================================

pub use errors::IBApiError;

// Contract types
pub use models::contract::{
    ComboLeg, Contract, ContractDescription, ContractDetails, DeltaNeutralContract,
};

// Order types
pub use models::order::{
    Order, OrderAllocation, OrderCancel, OrderComboLeg, OrderCondition, OrderState,
};

// Execution types
pub use models::execution::{CommissionAndFeesReport, Execution, ExecutionFilter};

// Bar / historical data types
pub use models::bar::{Bar, HistoricalSession, HistoricalTick, HistoricalTickBidAsk, HistoricalTickLast};

// Market data types
pub use models::market_data::{DepthMktDataDescription, TickAttrib, TickAttribBidAsk, TickAttribLast};

// Scanner
pub use models::scanner::ScannerSubscription;

// Common types
pub use models::common::{
    FamilyCode, HistogramEntry, NewsProvider, PriceIncrement, SmartComponent, SoftDollarTier,
    TagValue,
};

// Enums
pub use models::enums::*;

// Protocol
pub use protocol::TickType;

// Encoder / Decoder / Transport
pub use decoder::MessageDecoder;
pub use encoder::MessageEncoder;
pub use transport::Transport;

// Client / Reader / Events
pub use client::IBClient;
pub use reader::MessageReader;
pub use wrapper::{IBEvent, ScannerDataItem};
