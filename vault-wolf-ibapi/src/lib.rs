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

pub mod errors;
pub mod models;
pub mod protocol;

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
