//! IB TWS API event types.
//!
//! Defines `IBEvent`, an enum representing all possible events (callbacks) from
//! the IB TWS/Gateway server. This replaces the C++ `EWrapper` virtual callback
//! interface with a more Rust-idiomatic channel-based event model.
//!
//! Users receive events through a `tokio::sync::mpsc::UnboundedReceiver<IBEvent>`
//! returned from `IBClient::connect()`.
//!
//! Ported from: `EWrapper.h`, `EWrapper_prototypes.h` (113 virtual methods).

use rust_decimal::Decimal;

use crate::models::bar::{Bar, HistoricalSession, HistoricalTick, HistoricalTickBidAsk, HistoricalTickLast};
use crate::models::common::{
    FamilyCode, HistogramEntry, NewsProvider, PriceIncrement, SmartComponent, SoftDollarTier,
};
use crate::models::contract::{Contract, ContractDescription, ContractDetails, DeltaNeutralContract};
use crate::models::execution::{CommissionAndFeesReport, Execution};
use crate::models::market_data::{DepthMktDataDescription, TickAttrib, TickAttribBidAsk, TickAttribLast};
use crate::models::order::{Order, OrderState};
use crate::protocol::TickType;

// ============================================================================
// IBEvent
// ============================================================================

/// A single scanner result entry within a `ScannerData` event.
#[derive(Debug)]
pub struct ScannerDataItem {
    pub rank: i32,
    pub contract_details: ContractDetails,
    pub distance: String,
    pub benchmark: String,
    pub projection: String,
    pub legs_str: String,
}

/// All possible events from the IB TWS/Gateway server.
///
/// Each variant corresponds to one C++ `EWrapper` callback method.
/// Large variants use `Box` to keep the overall enum size manageable.
///
/// ## Usage
///
/// ```rust,ignore
/// let (mut client, mut rx) = IBClient::connect("127.0.0.1", 4002, 0, None).await?;
///
/// while let Some(event) = rx.recv().await {
///     match event {
///         IBEvent::NextValidId { order_id } => { /* ... */ },
///         IBEvent::TickPrice { req_id, tick_type, price, .. } => { /* ... */ },
///         IBEvent::Error { code, message, .. } => { /* ... */ },
///         _ => {}
///     }
/// }
/// ```
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum IBEvent {
    // ========================================================================
    // Connection & Error
    // ========================================================================

    /// Next valid order ID. Sent after successful connection.
    /// C++: `nextValidId(OrderId orderId)`
    NextValidId {
        order_id: i64,
    },

    /// List of managed accounts (comma-separated). Sent after connection.
    /// C++: `managedAccounts(const std::string& accountsList)`
    ManagedAccounts {
        accounts: String,
    },

    /// Server error or warning message.
    /// C++: `error(int id, time_t errorTime, int errorCode, ...)`
    Error {
        req_id: i32,
        error_time: i64,
        code: i32,
        message: String,
        advanced_order_reject_json: String,
    },

    /// Connection to TWS/Gateway has been closed.
    /// C++: `connectionClosed()`
    ConnectionClosed,

    // ========================================================================
    // Market Data (Ticks)
    // ========================================================================

    /// Real-time price tick.
    /// C++: `tickPrice(TickerId, TickType, double, const TickAttrib&)`
    TickPrice {
        req_id: i32,
        tick_type: TickType,
        price: f64,
        size: Decimal,
        attrib: TickAttrib,
    },

    /// Real-time size tick.
    /// C++: `tickSize(TickerId, TickType, Decimal)`
    TickSize {
        req_id: i32,
        tick_type: TickType,
        size: Decimal,
    },

    /// Option computation tick (Greeks).
    /// C++: `tickOptionComputation(TickerId, TickType, int, double, ...)`
    TickOptionComputation {
        req_id: i32,
        tick_type: TickType,
        tick_attrib: i32,
        implied_vol: Option<f64>,
        delta: Option<f64>,
        opt_price: Option<f64>,
        pv_dividend: Option<f64>,
        gamma: Option<f64>,
        vega: Option<f64>,
        theta: Option<f64>,
        und_price: Option<f64>,
    },

    /// Generic numeric tick value.
    /// C++: `tickGeneric(TickerId, TickType, double)`
    TickGeneric {
        req_id: i32,
        tick_type: TickType,
        value: f64,
    },

    /// String tick value.
    /// C++: `tickString(TickerId, TickType, const std::string&)`
    TickString {
        req_id: i32,
        tick_type: TickType,
        value: String,
    },

    /// Exchange for Physical tick.
    /// C++: `tickEFP(TickerId, TickType, double, ...)`
    TickEfp {
        req_id: i32,
        tick_type: TickType,
        basis_points: f64,
        formatted_basis_points: String,
        total_dividends: f64,
        hold_days: i32,
        future_last_trade_date: String,
        dividend_impact: f64,
        dividends_to_last_trade_date: f64,
    },

    /// Snapshot data complete for a request.
    /// C++: `tickSnapshotEnd(int reqId)`
    TickSnapshotEnd {
        req_id: i32,
    },

    /// Tick request parameters.
    /// C++: `tickReqParams(int, double, const std::string&, int)`
    TickReqParams {
        req_id: i32,
        min_tick: f64,
        bbo_exchange: String,
        snapshot_permissions: i32,
    },

    /// News tick.
    /// C++: `tickNews(int, time_t, ...)`
    TickNews {
        req_id: i32,
        timestamp: i64,
        provider_code: String,
        article_id: String,
        headline: String,
        extra_data: String,
    },

    /// Market data type change (real-time, frozen, delayed, etc.).
    /// C++: `marketDataType(TickerId, int)`
    MarketDataType {
        req_id: i32,
        market_data_type: i32,
    },

    // ========================================================================
    // Tick-by-Tick Data
    // ========================================================================

    /// Tick-by-tick last trade or all-last data.
    /// C++: `tickByTickAllLast(int, int, time_t, double, Decimal, ...)`
    TickByTickAllLast {
        req_id: i32,
        tick_type: i32,
        time: i64,
        price: f64,
        size: Decimal,
        attrib: TickAttribLast,
        exchange: String,
        special_conditions: String,
    },

    /// Tick-by-tick bid/ask data.
    /// C++: `tickByTickBidAsk(int, time_t, double, double, ...)`
    TickByTickBidAsk {
        req_id: i32,
        time: i64,
        bid_price: f64,
        ask_price: f64,
        bid_size: Decimal,
        ask_size: Decimal,
        attrib: TickAttribBidAsk,
    },

    /// Tick-by-tick midpoint data.
    /// C++: `tickByTickMidPoint(int, time_t, double)`
    TickByTickMidPoint {
        req_id: i32,
        time: i64,
        mid_point: f64,
    },

    // ========================================================================
    // Orders
    // ========================================================================

    /// Order status update.
    /// C++: `orderStatus(OrderId, const std::string&, Decimal, Decimal, ...)`
    OrderStatus {
        order_id: i64,
        status: String,
        filled: Decimal,
        remaining: Decimal,
        avg_fill_price: f64,
        perm_id: i64,
        parent_id: i32,
        last_fill_price: f64,
        client_id: i32,
        why_held: String,
        mkt_cap_price: f64,
    },

    /// Open order details.
    /// C++: `openOrder(OrderId, const Contract&, const Order&, const OrderState&)`
    OpenOrder {
        order_id: i64,
        contract: Box<Contract>,
        order: Box<Order>,
        order_state: Box<OrderState>,
    },

    /// End of open orders list.
    /// C++: `openOrderEnd()`
    OpenOrderEnd,

    /// Completed order details.
    /// C++: `completedOrder(const Contract&, const Order&, const OrderState&)`
    CompletedOrder {
        contract: Box<Contract>,
        order: Box<Order>,
        order_state: Box<OrderState>,
    },

    /// End of completed orders list.
    /// C++: `completedOrdersEnd()`
    CompletedOrdersEnd,

    /// Order bound notification (links perm ID to client/order ID).
    /// C++: `orderBound(long long, int, int)`
    OrderBound {
        perm_id: i64,
        client_id: i32,
        order_id: i32,
    },

    // ========================================================================
    // Execution
    // ========================================================================

    /// Execution (fill) details.
    /// C++: `execDetails(int, const Contract&, const Execution&)`
    ExecDetails {
        req_id: i32,
        contract: Box<Contract>,
        execution: Box<Execution>,
    },

    /// End of execution details for a request.
    /// C++: `execDetailsEnd(int reqId)`
    ExecDetailsEnd {
        req_id: i32,
    },

    /// Commission and fees report for an execution.
    /// C++: `commissionAndFeesReport(const CommissionAndFeesReport&)`
    CommissionReport {
        report: CommissionAndFeesReport,
    },

    // ========================================================================
    // Account Data
    // ========================================================================

    /// Account value update (key-value pair).
    /// C++: `updateAccountValue(const std::string&, ...)`
    UpdateAccountValue {
        key: String,
        value: String,
        currency: String,
        account_name: String,
    },

    /// Portfolio position update.
    /// C++: `updatePortfolio(const Contract&, Decimal, double, ...)`
    UpdatePortfolio {
        contract: Box<Contract>,
        position: Decimal,
        market_price: f64,
        market_value: f64,
        average_cost: f64,
        unrealized_pnl: f64,
        realized_pnl: f64,
        account_name: String,
    },

    /// Account update timestamp.
    /// C++: `updateAccountTime(const std::string&)`
    UpdateAccountTime {
        timestamp: String,
    },

    /// Account data download complete.
    /// C++: `accountDownloadEnd(const std::string&)`
    AccountDownloadEnd {
        account: String,
    },

    /// Account summary data row.
    /// C++: `accountSummary(int, const std::string&, ...)`
    AccountSummary {
        req_id: i32,
        account: String,
        tag: String,
        value: String,
        currency: String,
    },

    /// End of account summary.
    /// C++: `accountSummaryEnd(int)`
    AccountSummaryEnd {
        req_id: i32,
    },

    /// Position data.
    /// C++: `position(const std::string&, const Contract&, Decimal, double)`
    Position {
        account: String,
        contract: Box<Contract>,
        position: Decimal,
        avg_cost: f64,
    },

    /// End of positions list.
    /// C++: `positionEnd()`
    PositionEnd,

    /// Position data for multi-account/model.
    /// C++: `positionMulti(int, const std::string&, ...)`
    PositionMulti {
        req_id: i32,
        account: String,
        model_code: String,
        contract: Box<Contract>,
        pos: Decimal,
        avg_cost: f64,
    },

    /// End of multi-position list.
    /// C++: `positionMultiEnd(int)`
    PositionMultiEnd {
        req_id: i32,
    },

    /// Account update for multi-account/model.
    /// C++: `accountUpdateMulti(int, const std::string&, ...)`
    AccountUpdateMulti {
        req_id: i32,
        account: String,
        model_code: String,
        key: String,
        value: String,
        currency: String,
    },

    /// End of multi-account update.
    /// C++: `accountUpdateMultiEnd(int)`
    AccountUpdateMultiEnd {
        req_id: i32,
    },

    // ========================================================================
    // Contract Information
    // ========================================================================

    /// Contract details for a request.
    /// C++: `contractDetails(int, const ContractDetails&)`
    ContractDetails {
        req_id: i32,
        details: Box<ContractDetails>,
    },

    /// Bond contract details.
    /// C++: `bondContractDetails(int, const ContractDetails&)`
    BondContractDetails {
        req_id: i32,
        details: Box<ContractDetails>,
    },

    /// End of contract details for a request.
    /// C++: `contractDetailsEnd(int)`
    ContractDetailsEnd {
        req_id: i32,
    },

    /// Symbol search results.
    /// C++: `symbolSamples(int, const std::vector<ContractDescription>&)`
    SymbolSamples {
        req_id: i32,
        descriptions: Vec<ContractDescription>,
    },

    /// Delta neutral contract validation.
    /// C++: `deltaNeutralValidation(int, const DeltaNeutralContract&)`
    DeltaNeutralValidation {
        req_id: i32,
        delta_neutral_contract: DeltaNeutralContract,
    },

    /// Security definition optional parameters (option chains).
    /// C++: `securityDefinitionOptionalParameter(int, ...)`
    SecurityDefinitionOptionalParameter {
        req_id: i32,
        exchange: String,
        underlying_con_id: i32,
        trading_class: String,
        multiplier: String,
        expirations: Vec<String>,
        strikes: Vec<f64>,
    },

    /// End of security definition optional parameters.
    /// C++: `securityDefinitionOptionalParameterEnd(int)`
    SecurityDefinitionOptionalParameterEnd {
        req_id: i32,
    },

    // ========================================================================
    // Market Depth
    // ========================================================================

    /// Level I market depth update.
    /// C++: `updateMktDepth(TickerId, int, int, int, double, Decimal)`
    UpdateMktDepth {
        req_id: i32,
        position: i32,
        operation: i32,
        side: i32,
        price: f64,
        size: Decimal,
    },

    /// Level II market depth update.
    /// C++: `updateMktDepthL2(TickerId, int, const std::string&, ...)`
    UpdateMktDepthL2 {
        req_id: i32,
        position: i32,
        market_maker: String,
        operation: i32,
        side: i32,
        price: f64,
        size: Decimal,
        is_smart_depth: bool,
    },

    /// Available market depth exchanges.
    /// C++: `mktDepthExchanges(const std::vector<DepthMktDataDescription>&)`
    MktDepthExchanges {
        descriptions: Vec<DepthMktDataDescription>,
    },

    // ========================================================================
    // Historical Data
    // ========================================================================

    /// Historical data bars (complete batch from one server message).
    /// C++: `historicalData(TickerId, const Bar&)` — called once per bar in C++.
    HistoricalData {
        req_id: i32,
        bars: Vec<Bar>,
    },

    /// End of historical data.
    /// C++: `historicalDataEnd(int, const std::string&, const std::string&)`
    HistoricalDataEnd {
        req_id: i32,
        start: String,
        end: String,
    },

    /// Historical data update (streaming).
    /// C++: `historicalDataUpdate(TickerId, const Bar&)`
    HistoricalDataUpdate {
        req_id: i32,
        bar: Bar,
    },

    /// Head timestamp for historical data.
    /// C++: `headTimestamp(int, const std::string&)`
    HeadTimestamp {
        req_id: i32,
        head_timestamp: String,
    },

    /// Historical tick data (trades/midpoint).
    /// C++: `historicalTicks(int, const std::vector<HistoricalTick>&, bool)`
    HistoricalTicks {
        req_id: i32,
        ticks: Vec<HistoricalTick>,
        done: bool,
    },

    /// Historical bid/ask tick data.
    /// C++: `historicalTicksBidAsk(int, const std::vector<HistoricalTickBidAsk>&, bool)`
    HistoricalTicksBidAsk {
        req_id: i32,
        ticks: Vec<HistoricalTickBidAsk>,
        done: bool,
    },

    /// Historical last-trade tick data.
    /// C++: `historicalTicksLast(int, const std::vector<HistoricalTickLast>&, bool)`
    HistoricalTicksLast {
        req_id: i32,
        ticks: Vec<HistoricalTickLast>,
        done: bool,
    },

    /// Historical trading schedule.
    /// C++: `historicalSchedule(int, ...)`
    HistoricalSchedule {
        req_id: i32,
        start_date_time: String,
        end_date_time: String,
        time_zone: String,
        sessions: Vec<HistoricalSession>,
    },

    // ========================================================================
    // Real-time Bars
    // ========================================================================

    /// Real-time 5-second bar.
    /// C++: `realtimeBar(TickerId, long, double, double, double, double, ...)`
    RealtimeBar {
        req_id: i32,
        time: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: Decimal,
        wap: Decimal,
        count: i32,
    },

    // ========================================================================
    // Scanner
    // ========================================================================

    /// Scanner results (all rows from one server message).
    /// C++: `scannerData(int, int, const ContractDetails&, ...)` — called once per row.
    ScannerData {
        req_id: i32,
        items: Vec<ScannerDataItem>,
    },

    /// End of scanner data.
    /// C++: `scannerDataEnd(int)`
    ScannerDataEnd {
        req_id: i32,
    },

    /// Scanner parameters XML.
    /// C++: `scannerParameters(const std::string&)`
    ScannerParameters {
        xml: String,
    },

    // ========================================================================
    // Fundamentals
    // ========================================================================

    /// Fundamental data XML.
    /// C++: `fundamentalData(TickerId, const std::string&)`
    FundamentalData {
        req_id: i32,
        data: String,
    },

    // ========================================================================
    // P&L
    // ========================================================================

    /// P&L update.
    /// C++: `pnl(int, double, double, double)`
    Pnl {
        req_id: i32,
        daily_pnl: f64,
        unrealized_pnl: f64,
        realized_pnl: f64,
    },

    /// Single-position P&L update.
    /// C++: `pnlSingle(int, Decimal, double, double, double, double)`
    PnlSingle {
        req_id: i32,
        pos: Decimal,
        daily_pnl: f64,
        unrealized_pnl: f64,
        realized_pnl: f64,
        value: f64,
    },

    // ========================================================================
    // News
    // ========================================================================

    /// News bulletin.
    /// C++: `updateNewsBulletin(int, int, const std::string&, const std::string&)`
    UpdateNewsBulletin {
        msg_id: i32,
        msg_type: i32,
        message: String,
        origin_exch: String,
    },

    /// News article content.
    /// C++: `newsArticle(int, int, const std::string&)`
    NewsArticle {
        req_id: i32,
        article_type: i32,
        article_text: String,
    },

    /// Historical news headline.
    /// C++: `historicalNews(int, const std::string&, ...)`
    HistoricalNews {
        req_id: i32,
        time: String,
        provider_code: String,
        article_id: String,
        headline: String,
    },

    /// End of historical news.
    /// C++: `historicalNewsEnd(int, bool)`
    HistoricalNewsEnd {
        req_id: i32,
        has_more: bool,
    },

    /// List of available news providers.
    /// C++: `newsProviders(const std::vector<NewsProvider>&)`
    NewsProviders {
        providers: Vec<NewsProvider>,
    },

    // ========================================================================
    // Financial Advisor
    // ========================================================================

    /// Financial advisor data (groups/aliases XML).
    /// C++: `receiveFA(faDataType, const std::string&)`
    ReceiveFa {
        fa_data_type: i32,
        xml: String,
    },

    /// Replace FA configuration complete.
    /// C++: `replaceFAEnd(int, const std::string&)`
    ReplaceFaEnd {
        req_id: i32,
        text: String,
    },

    // ========================================================================
    // Market Rules & Infrastructure
    // ========================================================================

    /// Market rule (tick size table).
    /// C++: `marketRule(int, const std::vector<PriceIncrement>&)`
    MarketRule {
        market_rule_id: i32,
        price_increments: Vec<PriceIncrement>,
    },

    /// Market data reroute notification.
    /// C++: `rerouteMktDataReq(int, int, const std::string&)`
    RerouteMktDataReq {
        req_id: i32,
        con_id: i32,
        exchange: String,
    },

    /// Market depth reroute notification.
    /// C++: `rerouteMktDepthReq(int, int, const std::string&)`
    RerouteMktDepthReq {
        req_id: i32,
        con_id: i32,
        exchange: String,
    },

    /// Smart routing components.
    /// C++: `smartComponents(int, const SmartComponentsMap&)`
    SmartComponents {
        req_id: i32,
        components: Vec<SmartComponent>,
    },

    /// Family codes for linked accounts.
    /// C++: `familyCodes(const std::vector<FamilyCode>&)`
    FamilyCodes {
        codes: Vec<FamilyCode>,
    },

    /// Soft dollar tiers.
    /// C++: `softDollarTiers(int, const std::vector<SoftDollarTier>&)`
    SoftDollarTiers {
        req_id: i32,
        tiers: Vec<SoftDollarTier>,
    },

    // ========================================================================
    // Histogram
    // ========================================================================

    /// Histogram data.
    /// C++: `histogramData(int, const HistogramDataVector&)`
    HistogramData {
        req_id: i32,
        data: Vec<HistogramEntry>,
    },

    // ========================================================================
    // Time
    // ========================================================================

    /// Current server time (seconds since epoch).
    /// C++: `currentTime(long)`
    CurrentTime {
        time: i64,
    },

    /// Current server time in milliseconds.
    /// C++: `currentTimeInMillis(time_t)`
    CurrentTimeInMillis {
        time_in_millis: i64,
    },

    // ========================================================================
    // WSH (Wall Street Horizon)
    // ========================================================================

    /// WSH metadata.
    /// C++: `wshMetaData(int, const std::string&)`
    WshMetaData {
        req_id: i32,
        data_json: String,
    },

    /// WSH event data.
    /// C++: `wshEventData(int, const std::string&)`
    WshEventData {
        req_id: i32,
        data_json: String,
    },

    // ========================================================================
    // User Info
    // ========================================================================

    /// User info response.
    /// C++: `userInfo(int, const std::string&)`
    UserInfo {
        req_id: i32,
        white_branding_id: String,
    },

    // ========================================================================
    // Display Groups
    // ========================================================================

    /// Display group list.
    /// C++: `displayGroupList(int, const std::string&)`
    DisplayGroupList {
        req_id: i32,
        groups: String,
    },

    /// Display group updated.
    /// C++: `displayGroupUpdated(int, const std::string&)`
    DisplayGroupUpdated {
        req_id: i32,
        contract_info: String,
    },

    // ========================================================================
    // Verification (rarely used)
    // ========================================================================

    /// Verify message API.
    VerifyMessageApi {
        api_data: String,
    },

    /// Verify completed.
    VerifyCompleted {
        is_successful: bool,
        error_text: String,
    },

    /// Verify and auth message API.
    VerifyAndAuthMessageApi {
        api_data: String,
        xyz_challenge: String,
    },

    /// Verify and auth completed.
    VerifyAndAuthCompleted {
        is_successful: bool,
        error_text: String,
    },

    // ========================================================================
    // Unknown / Not Yet Decoded
    // ========================================================================

    /// Message with an unrecognized or not-yet-implemented message ID.
    /// Contains the raw message bytes for debugging.
    Unknown {
        msg_id: i32,
        data: Vec<u8>,
    },
}
