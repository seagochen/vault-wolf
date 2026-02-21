//! IB TWS API protocol constants.
//!
//! Ported from `cppclient/client/EDecoder.h`, `EClient.h`, `EWrapper.h`,
//! and `CommonDefs.h`. These constants define the binary wire protocol between
//! the client and TWS/Gateway.

use serde::{Deserialize, Serialize};

// ============================================================================
// Client / Protocol Constants
// ============================================================================

/// Client protocol version sent during handshake.
pub const CLIENT_VERSION: i32 = 66;

/// Minimum supported client version in the version range.
pub const MIN_CLIENT_VER: i32 = 100;

/// Maximum supported client version (= `MIN_SERVER_VER_PROTOBUF_PLACE_ORDER`).
pub const MAX_CLIENT_VER: i32 = 203;

/// Message header length (4-byte big-endian message length prefix).
pub const HEADER_LEN: usize = 4;

/// Raw integer encoding length (4-byte big-endian).
pub const RAW_INT_LEN: usize = 4;

/// Maximum message length: 16 MB - 1 byte.
pub const MAX_MSG_LEN: usize = 0xFFFFFF;

/// API protocol signature sent at connection start.
pub const API_SIGN: &[u8; 4] = b"API\0";

/// Maximum number of connection redirects allowed.
pub const REDIRECT_COUNT_MAX: i32 = 2;

// ============================================================================
// Incoming Message IDs (server -> client)
// ============================================================================

/// Incoming message type identifiers.
///
/// These are the first field in every message received from TWS/Gateway.
/// Ported from `EDecoder.h`.
#[allow(non_upper_case_globals)]
pub mod incoming {
    pub const TICK_PRICE: i32 = 1;
    pub const TICK_SIZE: i32 = 2;
    pub const ORDER_STATUS: i32 = 3;
    pub const ERR_MSG: i32 = 4;
    pub const OPEN_ORDER: i32 = 5;
    pub const ACCT_VALUE: i32 = 6;
    pub const PORTFOLIO_VALUE: i32 = 7;
    pub const ACCT_UPDATE_TIME: i32 = 8;
    pub const NEXT_VALID_ID: i32 = 9;
    pub const CONTRACT_DATA: i32 = 10;
    pub const EXECUTION_DATA: i32 = 11;
    pub const MARKET_DEPTH: i32 = 12;
    pub const MARKET_DEPTH_L2: i32 = 13;
    pub const NEWS_BULLETINS: i32 = 14;
    pub const MANAGED_ACCTS: i32 = 15;
    pub const RECEIVE_FA: i32 = 16;
    pub const HISTORICAL_DATA: i32 = 17;
    pub const BOND_CONTRACT_DATA: i32 = 18;
    pub const SCANNER_PARAMETERS: i32 = 19;
    pub const SCANNER_DATA: i32 = 20;
    pub const TICK_OPTION_COMPUTATION: i32 = 21;
    pub const TICK_GENERIC: i32 = 45;
    pub const TICK_STRING: i32 = 46;
    pub const TICK_EFP: i32 = 47;
    pub const CURRENT_TIME: i32 = 49;
    pub const REAL_TIME_BARS: i32 = 50;
    pub const FUNDAMENTAL_DATA: i32 = 51;
    pub const CONTRACT_DATA_END: i32 = 52;
    pub const OPEN_ORDER_END: i32 = 53;
    pub const ACCT_DOWNLOAD_END: i32 = 54;
    pub const EXECUTION_DATA_END: i32 = 55;
    pub const DELTA_NEUTRAL_VALIDATION: i32 = 56;
    pub const TICK_SNAPSHOT_END: i32 = 57;
    pub const MARKET_DATA_TYPE: i32 = 58;
    pub const COMMISSION_AND_FEES_REPORT: i32 = 59;
    pub const POSITION_DATA: i32 = 61;
    pub const POSITION_END: i32 = 62;
    pub const ACCOUNT_SUMMARY: i32 = 63;
    pub const ACCOUNT_SUMMARY_END: i32 = 64;
    pub const VERIFY_MESSAGE_API: i32 = 65;
    pub const VERIFY_COMPLETED: i32 = 66;
    pub const DISPLAY_GROUP_LIST: i32 = 67;
    pub const DISPLAY_GROUP_UPDATED: i32 = 68;
    pub const VERIFY_AND_AUTH_MESSAGE_API: i32 = 69;
    pub const VERIFY_AND_AUTH_COMPLETED: i32 = 70;
    pub const POSITION_MULTI: i32 = 71;
    pub const POSITION_MULTI_END: i32 = 72;
    pub const ACCOUNT_UPDATE_MULTI: i32 = 73;
    pub const ACCOUNT_UPDATE_MULTI_END: i32 = 74;
    pub const SECURITY_DEFINITION_OPTION_PARAMETER: i32 = 75;
    pub const SECURITY_DEFINITION_OPTION_PARAMETER_END: i32 = 76;
    pub const SOFT_DOLLAR_TIERS: i32 = 77;
    pub const FAMILY_CODES: i32 = 78;
    pub const SYMBOL_SAMPLES: i32 = 79;
    pub const MKT_DEPTH_EXCHANGES: i32 = 80;
    pub const TICK_REQ_PARAMS: i32 = 81;
    pub const SMART_COMPONENTS: i32 = 82;
    pub const NEWS_ARTICLE: i32 = 83;
    pub const TICK_NEWS: i32 = 84;
    pub const NEWS_PROVIDERS: i32 = 85;
    pub const HISTORICAL_NEWS: i32 = 86;
    pub const HISTORICAL_NEWS_END: i32 = 87;
    pub const HEAD_TIMESTAMP: i32 = 88;
    pub const HISTOGRAM_DATA: i32 = 89;
    pub const HISTORICAL_DATA_UPDATE: i32 = 90;
    pub const REROUTE_MKT_DATA_REQ: i32 = 91;
    pub const REROUTE_MKT_DEPTH_REQ: i32 = 92;
    pub const MARKET_RULE: i32 = 93;
    pub const PNL: i32 = 94;
    pub const PNL_SINGLE: i32 = 95;
    pub const HISTORICAL_TICKS: i32 = 96;
    pub const HISTORICAL_TICKS_BID_ASK: i32 = 97;
    pub const HISTORICAL_TICKS_LAST: i32 = 98;
    pub const TICK_BY_TICK: i32 = 99;
    pub const ORDER_BOUND: i32 = 100;
    pub const COMPLETED_ORDER: i32 = 101;
    pub const COMPLETED_ORDERS_END: i32 = 102;
    pub const REPLACE_FA_END: i32 = 103;
    pub const WSH_META_DATA: i32 = 104;
    pub const WSH_EVENT_DATA: i32 = 105;
    pub const HISTORICAL_SCHEDULE: i32 = 106;
    pub const USER_INFO: i32 = 107;
    pub const HISTORICAL_DATA_END: i32 = 108;
    pub const CURRENT_TIME_IN_MILLIS: i32 = 109;
}

// ============================================================================
// Outgoing Message IDs (client -> server)
// ============================================================================

/// Outgoing request type identifiers.
///
/// These are sent as the first field in every request to TWS/Gateway.
/// Ported from `EClient.h`.
#[allow(non_upper_case_globals)]
pub mod outgoing {
    pub const REQ_MKT_DATA: i32 = 1;
    pub const CANCEL_MKT_DATA: i32 = 2;
    pub const PLACE_ORDER: i32 = 3;
    pub const CANCEL_ORDER: i32 = 4;
    pub const REQ_OPEN_ORDERS: i32 = 5;
    pub const REQ_ACCT_DATA: i32 = 6;
    pub const REQ_EXECUTIONS: i32 = 7;
    pub const REQ_IDS: i32 = 8;
    pub const REQ_CONTRACT_DATA: i32 = 9;
    pub const REQ_MKT_DEPTH: i32 = 10;
    pub const CANCEL_MKT_DEPTH: i32 = 11;
    pub const REQ_NEWS_BULLETINS: i32 = 12;
    pub const CANCEL_NEWS_BULLETINS: i32 = 13;
    pub const SET_SERVER_LOGLEVEL: i32 = 14;
    pub const REQ_AUTO_OPEN_ORDERS: i32 = 15;
    pub const REQ_ALL_OPEN_ORDERS: i32 = 16;
    pub const REQ_MANAGED_ACCTS: i32 = 17;
    pub const REQ_FA: i32 = 18;
    pub const REPLACE_FA: i32 = 19;
    pub const REQ_HISTORICAL_DATA: i32 = 20;
    pub const EXERCISE_OPTIONS: i32 = 21;
    pub const REQ_SCANNER_SUBSCRIPTION: i32 = 22;
    pub const CANCEL_SCANNER_SUBSCRIPTION: i32 = 23;
    pub const REQ_SCANNER_PARAMETERS: i32 = 24;
    pub const CANCEL_HISTORICAL_DATA: i32 = 25;
    pub const REQ_CURRENT_TIME: i32 = 49;
    pub const REQ_REAL_TIME_BARS: i32 = 50;
    pub const CANCEL_REAL_TIME_BARS: i32 = 51;
    pub const REQ_FUNDAMENTAL_DATA: i32 = 52;
    pub const CANCEL_FUNDAMENTAL_DATA: i32 = 53;
    pub const REQ_CALC_IMPLIED_VOLAT: i32 = 54;
    pub const REQ_CALC_OPTION_PRICE: i32 = 55;
    pub const CANCEL_CALC_IMPLIED_VOLAT: i32 = 56;
    pub const CANCEL_CALC_OPTION_PRICE: i32 = 57;
    pub const REQ_GLOBAL_CANCEL: i32 = 58;
    pub const REQ_MARKET_DATA_TYPE: i32 = 59;
    pub const REQ_POSITIONS: i32 = 61;
    pub const REQ_ACCOUNT_SUMMARY: i32 = 62;
    pub const CANCEL_ACCOUNT_SUMMARY: i32 = 63;
    pub const CANCEL_POSITIONS: i32 = 64;
    pub const VERIFY_REQUEST: i32 = 65;
    pub const VERIFY_MESSAGE: i32 = 66;
    pub const QUERY_DISPLAY_GROUPS: i32 = 67;
    pub const SUBSCRIBE_TO_GROUP_EVENTS: i32 = 68;
    pub const UPDATE_DISPLAY_GROUP: i32 = 69;
    pub const UNSUBSCRIBE_FROM_GROUP_EVENTS: i32 = 70;
    pub const START_API: i32 = 71;
    pub const VERIFY_AND_AUTH_REQUEST: i32 = 72;
    pub const VERIFY_AND_AUTH_MESSAGE: i32 = 73;
    pub const REQ_POSITIONS_MULTI: i32 = 74;
    pub const CANCEL_POSITIONS_MULTI: i32 = 75;
    pub const REQ_ACCOUNT_UPDATES_MULTI: i32 = 76;
    pub const CANCEL_ACCOUNT_UPDATES_MULTI: i32 = 77;
    pub const REQ_SEC_DEF_OPT_PARAMS: i32 = 78;
    pub const REQ_SOFT_DOLLAR_TIERS: i32 = 79;
    pub const REQ_FAMILY_CODES: i32 = 80;
    pub const REQ_MATCHING_SYMBOLS: i32 = 81;
    pub const REQ_MKT_DEPTH_EXCHANGES: i32 = 82;
    pub const REQ_SMART_COMPONENTS: i32 = 83;
    pub const REQ_NEWS_ARTICLE: i32 = 84;
    pub const REQ_NEWS_PROVIDERS: i32 = 85;
    pub const REQ_HISTORICAL_NEWS: i32 = 86;
    pub const REQ_HEAD_TIMESTAMP: i32 = 87;
    pub const REQ_HISTOGRAM_DATA: i32 = 88;
    pub const CANCEL_HISTOGRAM_DATA: i32 = 89;
    pub const CANCEL_HEAD_TIMESTAMP: i32 = 90;
    pub const REQ_MARKET_RULE: i32 = 91;
    pub const REQ_PNL: i32 = 92;
    pub const CANCEL_PNL: i32 = 93;
    pub const REQ_PNL_SINGLE: i32 = 94;
    pub const CANCEL_PNL_SINGLE: i32 = 95;
    pub const REQ_HISTORICAL_TICKS: i32 = 96;
    pub const REQ_TICK_BY_TICK_DATA: i32 = 97;
    pub const CANCEL_TICK_BY_TICK_DATA: i32 = 98;
    pub const REQ_COMPLETED_ORDERS: i32 = 99;
    pub const REQ_WSH_META_DATA: i32 = 100;
    pub const CANCEL_WSH_META_DATA: i32 = 101;
    pub const REQ_WSH_EVENT_DATA: i32 = 102;
    pub const CANCEL_WSH_EVENT_DATA: i32 = 103;
    pub const REQ_USER_INFO: i32 = 104;
    pub const REQ_CURRENT_TIME_IN_MILLIS: i32 = 105;

    /// Protobuf message wrapper ID.
    pub const PROTOBUF_MSG_ID: i32 = 200;
}

// ============================================================================
// Minimum Server Version Constants
// ============================================================================

/// Server version gates that control which features/fields are sent.
///
/// These constants determine protocol behavior based on the negotiated server
/// version during the handshake. Ported from `EClient.h`.
#[allow(non_upper_case_globals)]
pub mod server_version {
    pub const PTA_ORDERS: i32 = 39;
    pub const FUNDAMENTAL_DATA: i32 = 40;
    pub const DELTA_NEUTRAL: i32 = 40;
    pub const CONTRACT_DATA_CHAIN: i32 = 40;
    pub const SCALE_ORDERS2: i32 = 40;
    pub const ALGO_ORDERS: i32 = 41;
    pub const EXECUTION_DATA_CHAIN: i32 = 42;
    pub const NOT_HELD: i32 = 44;
    pub const SEC_ID_TYPE: i32 = 45;
    pub const PLACE_ORDER_CONID: i32 = 46;
    pub const REQ_MKT_DATA_CONID: i32 = 47;
    pub const REQ_CALC_IMPLIED_VOLAT: i32 = 49;
    pub const REQ_CALC_OPTION_PRICE: i32 = 50;
    pub const CANCEL_CALC_IMPLIED_VOLAT: i32 = 50;
    pub const CANCEL_CALC_OPTION_PRICE: i32 = 50;
    pub const SSHORTX_OLD: i32 = 51;
    pub const SSHORTX: i32 = 52;
    pub const REQ_GLOBAL_CANCEL: i32 = 53;
    pub const HEDGE_ORDERS: i32 = 54;
    pub const REQ_MARKET_DATA_TYPE: i32 = 55;
    pub const OPT_OUT_SMART_ROUTING: i32 = 56;
    pub const SMART_COMBO_ROUTING_PARAMS: i32 = 57;
    pub const DELTA_NEUTRAL_CONID: i32 = 58;
    pub const SCALE_ORDERS3: i32 = 60;
    pub const ORDER_COMBO_LEGS_PRICE: i32 = 61;
    pub const TRAILING_PERCENT: i32 = 62;
    pub const DELTA_NEUTRAL_OPEN_CLOSE: i32 = 66;
    pub const POSITIONS: i32 = 67;
    pub const ACCOUNT_SUMMARY: i32 = 67;
    pub const TRADING_CLASS: i32 = 68;
    pub const SCALE_TABLE: i32 = 69;
    pub const LINKING: i32 = 70;
    pub const ALGO_ID: i32 = 71;
    pub const OPTIONAL_CAPABILITIES: i32 = 72;
    pub const ORDER_SOLICITED: i32 = 73;
    pub const LINKING_AUTH: i32 = 74;
    pub const PRIMARYEXCH: i32 = 75;
    pub const RANDOMIZE_SIZE_AND_PRICE: i32 = 76;
    pub const FRACTIONAL_POSITIONS: i32 = 101;
    pub const PEGGED_TO_BENCHMARK: i32 = 102;
    pub const MODELS_SUPPORT: i32 = 103;
    pub const SEC_DEF_OPT_PARAMS_REQ: i32 = 104;
    pub const EXT_OPERATOR: i32 = 105;
    pub const SOFT_DOLLAR_TIER: i32 = 106;
    pub const REQ_FAMILY_CODES: i32 = 107;
    pub const REQ_MATCHING_SYMBOLS: i32 = 108;
    pub const PAST_LIMIT: i32 = 109;
    pub const MD_SIZE_MULTIPLIER: i32 = 110;
    pub const CASH_QTY: i32 = 111;
    pub const REQ_MKT_DEPTH_EXCHANGES: i32 = 112;
    pub const TICK_NEWS: i32 = 113;
    pub const REQ_SMART_COMPONENTS: i32 = 114;
    pub const REQ_NEWS_PROVIDERS: i32 = 115;
    pub const REQ_NEWS_ARTICLE: i32 = 116;
    pub const REQ_HISTORICAL_NEWS: i32 = 117;
    pub const REQ_HEAD_TIMESTAMP: i32 = 118;
    pub const REQ_HISTOGRAM: i32 = 119;
    pub const SERVICE_DATA_TYPE: i32 = 120;
    pub const AGG_GROUP: i32 = 121;
    pub const UNDERLYING_INFO: i32 = 122;
    pub const CANCEL_HEADTIMESTAMP: i32 = 123;
    pub const SYNT_REALTIME_BARS: i32 = 124;
    pub const CFD_REROUTE: i32 = 125;
    pub const MARKET_RULES: i32 = 126;
    pub const PNL: i32 = 127;
    pub const NEWS_QUERY_ORIGINS: i32 = 128;
    pub const UNREALIZED_PNL: i32 = 129;
    pub const HISTORICAL_TICKS: i32 = 130;
    pub const MARKET_CAP_PRICE: i32 = 131;
    pub const PRE_OPEN_BID_ASK: i32 = 132;
    pub const REAL_EXPIRATION_DATE: i32 = 134;
    pub const REALIZED_PNL: i32 = 135;
    pub const LAST_LIQUIDITY: i32 = 136;
    pub const TICK_BY_TICK: i32 = 137;
    pub const DECISION_MAKER: i32 = 138;
    pub const MIFID_EXECUTION: i32 = 139;
    pub const TICK_BY_TICK_IGNORE_SIZE: i32 = 140;
    pub const AUTO_PRICE_FOR_HEDGE: i32 = 141;
    pub const WHAT_IF_EXT_FIELDS: i32 = 142;
    pub const SCANNER_GENERIC_OPTS: i32 = 143;
    pub const API_BIND_ORDER: i32 = 144;
    pub const ORDER_CONTAINER: i32 = 145;
    pub const SMART_DEPTH: i32 = 146;
    pub const REMOVE_NULL_ALL_CASTING: i32 = 147;
    pub const D_PEG_ORDERS: i32 = 148;
    pub const MKT_DEPTH_PRIM_EXCHANGE: i32 = 149;
    pub const COMPLETED_ORDERS: i32 = 150;
    pub const PRICE_MGMT_ALGO: i32 = 151;
    pub const STOCK_TYPE: i32 = 152;
    pub const ENCODE_MSG_ASCII7: i32 = 153;
    pub const SEND_ALL_FAMILY_CODES: i32 = 154;
    pub const NO_DEFAULT_OPEN_CLOSE: i32 = 155;
    pub const PRICE_BASED_VOLATILITY: i32 = 156;
    pub const REPLACE_FA_END: i32 = 157;
    pub const DURATION: i32 = 158;
    pub const MARKET_DATA_IN_SHARES: i32 = 159;
    pub const POST_TO_ATS: i32 = 160;
    pub const WSHE_CALENDAR: i32 = 161;
    pub const AUTO_CANCEL_PARENT: i32 = 162;
    pub const FRACTIONAL_SIZE_SUPPORT: i32 = 163;
    pub const SIZE_RULES: i32 = 164;
    pub const HISTORICAL_SCHEDULE: i32 = 165;
    pub const ADVANCED_ORDER_REJECT: i32 = 166;
    pub const USER_INFO: i32 = 167;
    pub const CRYPTO_AGGREGATED_TRADES: i32 = 168;
    pub const MANUAL_ORDER_TIME: i32 = 169;
    pub const PEGBEST_PEGMID_OFFSETS: i32 = 170;
    pub const WSH_EVENT_DATA_FILTERS: i32 = 171;
    pub const IPO_PRICES: i32 = 172;
    pub const WSH_EVENT_DATA_FILTERS_DATE: i32 = 173;
    pub const INSTRUMENT_TIMEZONE: i32 = 174;
    pub const HMDS_MARKET_DATA_IN_SHARES: i32 = 175;
    pub const BOND_ISSUERID: i32 = 176;
    pub const FA_PROFILE_DESUPPORT: i32 = 177;
    pub const PENDING_PRICE_REVISION: i32 = 178;
    pub const FUND_DATA_FIELDS: i32 = 179;
    pub const MANUAL_ORDER_TIME_EXERCISE_OPTIONS: i32 = 180;
    pub const OPEN_ORDER_AD_STRATEGY: i32 = 181;
    pub const LAST_TRADE_DATE: i32 = 182;
    pub const CUSTOMER_ACCOUNT: i32 = 183;
    pub const PROFESSIONAL_CUSTOMER: i32 = 184;
    pub const BOND_ACCRUED_INTEREST: i32 = 185;
    pub const INELIGIBILITY_REASONS: i32 = 186;
    pub const RFQ_FIELDS: i32 = 187;
    pub const BOND_TRADING_HOURS: i32 = 188;
    pub const INCLUDE_OVERNIGHT: i32 = 189;
    pub const UNDO_RFQ_FIELDS: i32 = 190;
    pub const PERM_ID_AS_LONG: i32 = 191;
    pub const CME_TAGGING_FIELDS: i32 = 192;
    pub const CME_TAGGING_FIELDS_IN_OPEN_ORDER: i32 = 193;
    pub const ERROR_TIME: i32 = 194;
    pub const FULL_ORDER_PREVIEW_FIELDS: i32 = 195;
    pub const HISTORICAL_DATA_END: i32 = 196;
    pub const CURRENT_TIME_IN_MILLIS: i32 = 197;
    pub const SUBMITTER: i32 = 198;
    pub const IMBALANCE_ONLY: i32 = 199;
    pub const PARAMETRIZED_DAYS_OF_EXECUTIONS: i32 = 200;
    pub const PROTOBUF: i32 = 201;
    pub const ZERO_STRIKE: i32 = 202;
    pub const PROTOBUF_PLACE_ORDER: i32 = 203;
}

// ============================================================================
// TickType Enum
// ============================================================================

/// Tick type identifiers for market data callbacks.
///
/// Maps to C++ `enum TickType` (106 values, `BID_SIZE`=0 through `NOT_SET`=105).
/// Ported from `EWrapper.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum TickType {
    BidSize = 0,
    Bid = 1,
    Ask = 2,
    AskSize = 3,
    Last = 4,
    LastSize = 5,
    High = 6,
    Low = 7,
    Volume = 8,
    Close = 9,
    BidOptionComputation = 10,
    AskOptionComputation = 11,
    LastOptionComputation = 12,
    ModelOption = 13,
    Open = 14,
    Low13Week = 15,
    High13Week = 16,
    Low26Week = 17,
    High26Week = 18,
    Low52Week = 19,
    High52Week = 20,
    AvgVolume = 21,
    OpenInterest = 22,
    OptionHistoricalVol = 23,
    OptionImpliedVol = 24,
    OptionBidExch = 25,
    OptionAskExch = 26,
    OptionCallOpenInterest = 27,
    OptionPutOpenInterest = 28,
    OptionCallVolume = 29,
    OptionPutVolume = 30,
    IndexFuturePremium = 31,
    BidExch = 32,
    AskExch = 33,
    AuctionVolume = 34,
    AuctionPrice = 35,
    AuctionImbalance = 36,
    MarkPrice = 37,
    BidEfpComputation = 38,
    AskEfpComputation = 39,
    LastEfpComputation = 40,
    OpenEfpComputation = 41,
    HighEfpComputation = 42,
    LowEfpComputation = 43,
    CloseEfpComputation = 44,
    LastTimestamp = 45,
    Shortable = 46,
    FundamentalRatios = 47,
    RtVolume = 48,
    Halted = 49,
    BidYield = 50,
    AskYield = 51,
    LastYield = 52,
    CustOptionComputation = 53,
    TradeCount = 54,
    TradeRate = 55,
    VolumeRate = 56,
    LastRthTrade = 57,
    RtHistoricalVol = 58,
    IbDividends = 59,
    BondFactorMultiplier = 60,
    RegulatoryImbalance = 61,
    NewsTick = 62,
    ShortTermVolume3Min = 63,
    ShortTermVolume5Min = 64,
    ShortTermVolume10Min = 65,
    DelayedBid = 66,
    DelayedAsk = 67,
    DelayedLast = 68,
    DelayedBidSize = 69,
    DelayedAskSize = 70,
    DelayedLastSize = 71,
    DelayedHigh = 72,
    DelayedLow = 73,
    DelayedVolume = 74,
    DelayedClose = 75,
    DelayedOpen = 76,
    RtTrdVolume = 77,
    CreditmanMarkPrice = 78,
    CreditmanSlowMarkPrice = 79,
    DelayedBidOptionComputation = 80,
    DelayedAskOptionComputation = 81,
    DelayedLastOptionComputation = 82,
    DelayedModelOptionComputation = 83,
    LastExch = 84,
    LastRegTime = 85,
    FuturesOpenInterest = 86,
    AvgOptVolume = 87,
    DelayedLastTimestamp = 88,
    ShortableShares = 89,
    DelayedHalted = 90,
    Reuters2MutualFunds = 91,
    EtfNavClose = 92,
    EtfNavPriorClose = 93,
    EtfNavBid = 94,
    EtfNavAsk = 95,
    EtfNavLast = 96,
    EtfFrozenNavLast = 97,
    EtfNavHigh = 98,
    EtfNavLow = 99,
    SocialMarketAnalytics = 100,
    EstimatedIpoMidpoint = 101,
    FinalIpoLast = 102,
    DelayedYieldBid = 103,
    DelayedYieldAsk = 104,
    NotSet = 105,
}

impl TryFrom<i32> for TickType {
    type Error = i32;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::BidSize),
            1 => Ok(Self::Bid),
            2 => Ok(Self::Ask),
            3 => Ok(Self::AskSize),
            4 => Ok(Self::Last),
            5 => Ok(Self::LastSize),
            6 => Ok(Self::High),
            7 => Ok(Self::Low),
            8 => Ok(Self::Volume),
            9 => Ok(Self::Close),
            10 => Ok(Self::BidOptionComputation),
            11 => Ok(Self::AskOptionComputation),
            12 => Ok(Self::LastOptionComputation),
            13 => Ok(Self::ModelOption),
            14 => Ok(Self::Open),
            15 => Ok(Self::Low13Week),
            16 => Ok(Self::High13Week),
            17 => Ok(Self::Low26Week),
            18 => Ok(Self::High26Week),
            19 => Ok(Self::Low52Week),
            20 => Ok(Self::High52Week),
            21 => Ok(Self::AvgVolume),
            22 => Ok(Self::OpenInterest),
            23 => Ok(Self::OptionHistoricalVol),
            24 => Ok(Self::OptionImpliedVol),
            25 => Ok(Self::OptionBidExch),
            26 => Ok(Self::OptionAskExch),
            27 => Ok(Self::OptionCallOpenInterest),
            28 => Ok(Self::OptionPutOpenInterest),
            29 => Ok(Self::OptionCallVolume),
            30 => Ok(Self::OptionPutVolume),
            31 => Ok(Self::IndexFuturePremium),
            32 => Ok(Self::BidExch),
            33 => Ok(Self::AskExch),
            34 => Ok(Self::AuctionVolume),
            35 => Ok(Self::AuctionPrice),
            36 => Ok(Self::AuctionImbalance),
            37 => Ok(Self::MarkPrice),
            38 => Ok(Self::BidEfpComputation),
            39 => Ok(Self::AskEfpComputation),
            40 => Ok(Self::LastEfpComputation),
            41 => Ok(Self::OpenEfpComputation),
            42 => Ok(Self::HighEfpComputation),
            43 => Ok(Self::LowEfpComputation),
            44 => Ok(Self::CloseEfpComputation),
            45 => Ok(Self::LastTimestamp),
            46 => Ok(Self::Shortable),
            47 => Ok(Self::FundamentalRatios),
            48 => Ok(Self::RtVolume),
            49 => Ok(Self::Halted),
            50 => Ok(Self::BidYield),
            51 => Ok(Self::AskYield),
            52 => Ok(Self::LastYield),
            53 => Ok(Self::CustOptionComputation),
            54 => Ok(Self::TradeCount),
            55 => Ok(Self::TradeRate),
            56 => Ok(Self::VolumeRate),
            57 => Ok(Self::LastRthTrade),
            58 => Ok(Self::RtHistoricalVol),
            59 => Ok(Self::IbDividends),
            60 => Ok(Self::BondFactorMultiplier),
            61 => Ok(Self::RegulatoryImbalance),
            62 => Ok(Self::NewsTick),
            63 => Ok(Self::ShortTermVolume3Min),
            64 => Ok(Self::ShortTermVolume5Min),
            65 => Ok(Self::ShortTermVolume10Min),
            66 => Ok(Self::DelayedBid),
            67 => Ok(Self::DelayedAsk),
            68 => Ok(Self::DelayedLast),
            69 => Ok(Self::DelayedBidSize),
            70 => Ok(Self::DelayedAskSize),
            71 => Ok(Self::DelayedLastSize),
            72 => Ok(Self::DelayedHigh),
            73 => Ok(Self::DelayedLow),
            74 => Ok(Self::DelayedVolume),
            75 => Ok(Self::DelayedClose),
            76 => Ok(Self::DelayedOpen),
            77 => Ok(Self::RtTrdVolume),
            78 => Ok(Self::CreditmanMarkPrice),
            79 => Ok(Self::CreditmanSlowMarkPrice),
            80 => Ok(Self::DelayedBidOptionComputation),
            81 => Ok(Self::DelayedAskOptionComputation),
            82 => Ok(Self::DelayedLastOptionComputation),
            83 => Ok(Self::DelayedModelOptionComputation),
            84 => Ok(Self::LastExch),
            85 => Ok(Self::LastRegTime),
            86 => Ok(Self::FuturesOpenInterest),
            87 => Ok(Self::AvgOptVolume),
            88 => Ok(Self::DelayedLastTimestamp),
            89 => Ok(Self::ShortableShares),
            90 => Ok(Self::DelayedHalted),
            91 => Ok(Self::Reuters2MutualFunds),
            92 => Ok(Self::EtfNavClose),
            93 => Ok(Self::EtfNavPriorClose),
            94 => Ok(Self::EtfNavBid),
            95 => Ok(Self::EtfNavAsk),
            96 => Ok(Self::EtfNavLast),
            97 => Ok(Self::EtfFrozenNavLast),
            98 => Ok(Self::EtfNavHigh),
            99 => Ok(Self::EtfNavLow),
            100 => Ok(Self::SocialMarketAnalytics),
            101 => Ok(Self::EstimatedIpoMidpoint),
            102 => Ok(Self::FinalIpoLast),
            103 => Ok(Self::DelayedYieldBid),
            104 => Ok(Self::DelayedYieldAsk),
            105 => Ok(Self::NotSet),
            other => Err(other),
        }
    }
}

impl From<TickType> for i32 {
    fn from(tt: TickType) -> i32 {
        tt as i32
    }
}

// ============================================================================
// Client Error Codes
// ============================================================================

/// Well-known IB client error codes.
///
/// Ported from `TwsSocketClientErrors.h`.
pub mod client_errors {
    pub const ALREADY_CONNECTED: i32 = 501;
    pub const CONNECT_FAIL: i32 = 502;
    pub const UPDATE_TWS: i32 = 503;
    pub const NOT_CONNECTED: i32 = 504;
    pub const UNKNOWN_ID: i32 = 505;
    pub const UNSUPPORTED_VERSION: i32 = 506;
    pub const BAD_LENGTH: i32 = 507;
    pub const BAD_MESSAGE: i32 = 508;
    pub const SOCKET_EXCEPTION: i32 = 509;
    pub const FAIL_CREATE_SOCK: i32 = 520;
    pub const SSL_FAIL: i32 = 530;
    pub const INVALID_SYMBOL: i32 = 579;
    pub const FA_PROFILE_NOT_SUPPORTED: i32 = 585;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_type_try_from() {
        assert_eq!(TickType::try_from(0), Ok(TickType::BidSize));
        assert_eq!(TickType::try_from(4), Ok(TickType::Last));
        assert_eq!(TickType::try_from(105), Ok(TickType::NotSet));
        assert_eq!(TickType::try_from(999), Err(999));
    }

    #[test]
    fn tick_type_into_i32() {
        assert_eq!(i32::from(TickType::BidSize), 0);
        assert_eq!(i32::from(TickType::Last), 4);
        assert_eq!(i32::from(TickType::NotSet), 105);
    }

    #[test]
    fn protocol_constants_sanity() {
        assert_eq!(CLIENT_VERSION, 66);
        assert_eq!(MAX_MSG_LEN, 0xFFFFFF);
        assert_eq!(HEADER_LEN, 4);
        assert_eq!(MIN_CLIENT_VER, 100);
        assert_eq!(MAX_CLIENT_VER, 203);
    }

    #[test]
    fn server_version_constants() {
        assert_eq!(server_version::PTA_ORDERS, 39);
        assert_eq!(server_version::PROTOBUF, 201);
        assert_eq!(server_version::PROTOBUF_PLACE_ORDER, 203);
        assert_eq!(server_version::POSITIONS, 67);
        assert_eq!(server_version::ACCOUNT_SUMMARY, 67);
    }

    #[test]
    fn message_id_constants() {
        assert_eq!(incoming::TICK_PRICE, 1);
        assert_eq!(incoming::ORDER_STATUS, 3);
        assert_eq!(incoming::CURRENT_TIME_IN_MILLIS, 109);
        assert_eq!(outgoing::REQ_MKT_DATA, 1);
        assert_eq!(outgoing::PLACE_ORDER, 3);
        assert_eq!(outgoing::START_API, 71);
    }
}
