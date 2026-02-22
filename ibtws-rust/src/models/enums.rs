//! Strongly-typed enums replacing C++ string and integer constants.
//!
//! Each enum provides type safety and forward compatibility via `Other(String)`
//! variants for extensible types. Serde `rename` attributes match the IB wire
//! protocol strings exactly.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

// ============================================================================
// Security / Contract Enums
// ============================================================================

/// Security type (C++: `string secType` field in `Contract`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SecType {
    #[serde(rename = "STK")]
    Stock,
    #[serde(rename = "OPT")]
    Option,
    #[serde(rename = "FUT")]
    Future,
    #[serde(rename = "CASH")]
    Forex,
    #[serde(rename = "IND")]
    Index,
    #[serde(rename = "FOP")]
    FutureOption,
    #[serde(rename = "BOND")]
    Bond,
    #[serde(rename = "FUND")]
    Fund,
    #[serde(rename = "WAR")]
    Warrant,
    #[serde(rename = "CMDTY")]
    Commodity,
    #[serde(rename = "BAG")]
    Combo,
    #[serde(rename = "NEWS")]
    News,
    #[serde(rename = "CRYPTO")]
    Crypto,
    /// Unrecognized security type from the server.
    #[serde(untagged)]
    Other(String),
}

impl fmt::Display for SecType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stock => write!(f, "STK"),
            Self::Option => write!(f, "OPT"),
            Self::Future => write!(f, "FUT"),
            Self::Forex => write!(f, "CASH"),
            Self::Index => write!(f, "IND"),
            Self::FutureOption => write!(f, "FOP"),
            Self::Bond => write!(f, "BOND"),
            Self::Fund => write!(f, "FUND"),
            Self::Warrant => write!(f, "WAR"),
            Self::Commodity => write!(f, "CMDTY"),
            Self::Combo => write!(f, "BAG"),
            Self::News => write!(f, "NEWS"),
            Self::Crypto => write!(f, "CRYPTO"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for SecType {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "STK" => Self::Stock,
            "OPT" => Self::Option,
            "FUT" => Self::Future,
            "CASH" => Self::Forex,
            "IND" => Self::Index,
            "FOP" => Self::FutureOption,
            "BOND" => Self::Bond,
            "FUND" => Self::Fund,
            "WAR" => Self::Warrant,
            "CMDTY" => Self::Commodity,
            "BAG" => Self::Combo,
            "NEWS" => Self::News,
            "CRYPTO" => Self::Crypto,
            other => Self::Other(other.to_string()),
        })
    }
}

/// Option right (C++: `string right` field in `Contract`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Right {
    #[serde(rename = "C")]
    Call,
    #[serde(rename = "P")]
    Put,
    #[serde(rename = "")]
    Undefined,
}

impl fmt::Display for Right {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call => write!(f, "C"),
            Self::Put => write!(f, "P"),
            Self::Undefined => write!(f, ""),
        }
    }
}

impl FromStr for Right {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "C" | "CALL" => Self::Call,
            "P" | "PUT" => Self::Put,
            _ => Self::Undefined,
        })
    }
}

/// Security ID type (C++: `string secIdType` field in `Contract`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecIdType {
    #[serde(rename = "CUSIP")]
    Cusip,
    #[serde(rename = "SEDOL")]
    Sedol,
    #[serde(rename = "ISIN")]
    Isin,
    #[serde(rename = "RIC")]
    Ric,
    #[serde(untagged)]
    Other(String),
}

impl fmt::Display for SecIdType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cusip => write!(f, "CUSIP"),
            Self::Sedol => write!(f, "SEDOL"),
            Self::Isin => write!(f, "ISIN"),
            Self::Ric => write!(f, "RIC"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for SecIdType {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "CUSIP" => Self::Cusip,
            "SEDOL" => Self::Sedol,
            "ISIN" => Self::Isin,
            "RIC" => Self::Ric,
            other => Self::Other(other.to_string()),
        })
    }
}

// ============================================================================
// Order Enums
// ============================================================================

/// Order action (C++: `string action` field in `Order`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    #[serde(rename = "BUY")]
    Buy,
    #[serde(rename = "SELL")]
    Sell,
    #[serde(rename = "SSHORT")]
    SellShort,
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => write!(f, "BUY"),
            Self::Sell => write!(f, "SELL"),
            Self::SellShort => write!(f, "SSHORT"),
        }
    }
}

impl FromStr for Action {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "BUY" => Ok(Self::Buy),
            "SELL" => Ok(Self::Sell),
            "SSHORT" => Ok(Self::SellShort),
            other => Err(format!("unknown action: {other}")),
        }
    }
}

/// Order type (C++: `string orderType` field in `Order`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    #[serde(rename = "MKT")]
    Market,
    #[serde(rename = "LMT")]
    Limit,
    #[serde(rename = "STP")]
    Stop,
    #[serde(rename = "STP LMT")]
    StopLimit,
    #[serde(rename = "TRAIL")]
    TrailingStop,
    #[serde(rename = "TRAIL LIMIT")]
    TrailingStopLimit,
    #[serde(rename = "REL")]
    Relative,
    #[serde(rename = "MOC")]
    MarketOnClose,
    #[serde(rename = "LOC")]
    LimitOnClose,
    #[serde(rename = "MOO")]
    MarketOnOpen,
    #[serde(rename = "LOO")]
    LimitOnOpen,
    #[serde(rename = "PEG MKT")]
    PeggedToMarket,
    #[serde(rename = "PEG MID")]
    PeggedToMidpoint,
    #[serde(rename = "PEG BENCH")]
    PeggedToBenchmark,
    #[serde(rename = "VOL")]
    Volatility,
    #[serde(rename = "MIT")]
    MarketIfTouched,
    #[serde(rename = "LIT")]
    LimitIfTouched,
    #[serde(rename = "MKT PRT")]
    MarketWithProtection,
    #[serde(rename = "MIDPRICE")]
    MidPrice,
    #[serde(rename = "SNAP MKT")]
    SnapToMarket,
    #[serde(rename = "SNAP MID")]
    SnapToMidpoint,
    #[serde(rename = "PEG PRIM")]
    PeggedToPrimary,
    /// Unrecognized order type from the server.
    #[serde(untagged)]
    Other(String),
}

impl fmt::Display for OrderType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Market => write!(f, "MKT"),
            Self::Limit => write!(f, "LMT"),
            Self::Stop => write!(f, "STP"),
            Self::StopLimit => write!(f, "STP LMT"),
            Self::TrailingStop => write!(f, "TRAIL"),
            Self::TrailingStopLimit => write!(f, "TRAIL LIMIT"),
            Self::Relative => write!(f, "REL"),
            Self::MarketOnClose => write!(f, "MOC"),
            Self::LimitOnClose => write!(f, "LOC"),
            Self::MarketOnOpen => write!(f, "MOO"),
            Self::LimitOnOpen => write!(f, "LOO"),
            Self::PeggedToMarket => write!(f, "PEG MKT"),
            Self::PeggedToMidpoint => write!(f, "PEG MID"),
            Self::PeggedToBenchmark => write!(f, "PEG BENCH"),
            Self::Volatility => write!(f, "VOL"),
            Self::MarketIfTouched => write!(f, "MIT"),
            Self::LimitIfTouched => write!(f, "LIT"),
            Self::MarketWithProtection => write!(f, "MKT PRT"),
            Self::MidPrice => write!(f, "MIDPRICE"),
            Self::SnapToMarket => write!(f, "SNAP MKT"),
            Self::SnapToMidpoint => write!(f, "SNAP MID"),
            Self::PeggedToPrimary => write!(f, "PEG PRIM"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for OrderType {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "MKT" => Self::Market,
            "LMT" => Self::Limit,
            "STP" => Self::Stop,
            "STP LMT" => Self::StopLimit,
            "TRAIL" => Self::TrailingStop,
            "TRAIL LIMIT" => Self::TrailingStopLimit,
            "REL" => Self::Relative,
            "MOC" => Self::MarketOnClose,
            "LOC" => Self::LimitOnClose,
            "MOO" => Self::MarketOnOpen,
            "LOO" => Self::LimitOnOpen,
            "PEG MKT" => Self::PeggedToMarket,
            "PEG MID" => Self::PeggedToMidpoint,
            "PEG BENCH" => Self::PeggedToBenchmark,
            "VOL" => Self::Volatility,
            "MIT" => Self::MarketIfTouched,
            "LIT" => Self::LimitIfTouched,
            "MKT PRT" => Self::MarketWithProtection,
            "MIDPRICE" => Self::MidPrice,
            "SNAP MKT" => Self::SnapToMarket,
            "SNAP MID" => Self::SnapToMidpoint,
            "PEG PRIM" => Self::PeggedToPrimary,
            other => Self::Other(other.to_string()),
        })
    }
}

/// Time in force (C++: `string tif` field in `Order`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeInForce {
    #[serde(rename = "DAY")]
    Day,
    #[serde(rename = "GTC")]
    GoodTilCancelled,
    #[serde(rename = "IOC")]
    ImmediateOrCancel,
    #[serde(rename = "GTD")]
    GoodTilDate,
    #[serde(rename = "OPG")]
    AtTheOpening,
    #[serde(rename = "FOK")]
    FillOrKill,
    #[serde(rename = "DTC")]
    DayTilCancelled,
    #[serde(untagged)]
    Other(String),
}

impl fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Day => write!(f, "DAY"),
            Self::GoodTilCancelled => write!(f, "GTC"),
            Self::ImmediateOrCancel => write!(f, "IOC"),
            Self::GoodTilDate => write!(f, "GTD"),
            Self::AtTheOpening => write!(f, "OPG"),
            Self::FillOrKill => write!(f, "FOK"),
            Self::DayTilCancelled => write!(f, "DTC"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

impl FromStr for TimeInForce {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "DAY" => Self::Day,
            "GTC" => Self::GoodTilCancelled,
            "IOC" => Self::ImmediateOrCancel,
            "GTD" => Self::GoodTilDate,
            "OPG" => Self::AtTheOpening,
            "FOK" => Self::FillOrKill,
            "DTC" => Self::DayTilCancelled,
            other => Self::Other(other.to_string()),
        })
    }
}

// ============================================================================
// Integer-Based Enums (from CommonDefs.h, Order.h)
// ============================================================================

/// Order origin (C++: `enum Origin` in `Order.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum Origin {
    #[default]
    Customer = 0,
    Firm = 1,
    Unknown = 2,
}

/// Auction strategy (C++: `enum AuctionStrategy` in `Order.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum AuctionStrategy {
    #[default]
    Unset = 0,
    Match = 1,
    Improvement = 2,
    Transparent = 3,
}

/// Combo leg open/close (C++: `enum LegOpenClose` in `Contract.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum LegOpenClose {
    #[default]
    Same = 0,
    Open = 1,
    Close = 2,
    Unknown = 3,
}

/// Market data type (C++: `enum MarketDataType` in `CommonDefs.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum MarketDataType {
    RealTime = 1,
    Frozen = 2,
    Delayed = 3,
    DelayedFrozen = 4,
}

/// FA data type (C++: `enum faDataType` in `CommonDefs.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum FaDataType {
    Groups = 1,
    Aliases = 3,
}

/// Fund asset type (C++: `enum class FundAssetType` in `CommonDefs.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FundAssetType {
    #[default]
    None,
    Others,
    MoneyMarket,
    FixedIncome,
    MultiAsset,
    Equity,
    Sector,
    Guaranteed,
    Alternative,
}

/// Fund distribution policy indicator (C++: `enum class FundDistributionPolicyIndicator`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FundDistributionPolicyIndicator {
    #[default]
    None,
    AccumulationFund,
    IncomeFund,
}

/// Option exercise type (C++: `enum class OptionExerciseType` in `CommonDefs.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OptionExerciseType {
    #[default]
    None,
    Exercise,
    Lapse,
    DoNothing,
    Assigned,
    AutoexerciseClearing,
    Expired,
    Netting,
    AutoexerciseTrading,
}

/// Use price management algorithm (C++: `enum UsePriceMmgtAlgo` in `Order.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum UsePriceMgmtAlgo {
    DontUse = 0,
    Use = 1,
    #[default]
    Default = 2,
}

/// Trigger method for price conditions and orders (C++: `PriceCondition::Method`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum TriggerMethod {
    #[default]
    Default = 0,
    DoubleBidAsk = 1,
    Last = 2,
    DoubleLast = 3,
    BidAsk = 4,
    LastOrBidAsk = 7,
    MidPoint = 8,
}

/// Order condition type discriminant (C++: `OrderCondition::OrderConditionType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum OrderConditionType {
    Price = 1,
    Time = 3,
    Margin = 4,
    Execution = 5,
    Volume = 6,
    PercentChange = 7,
}

// ============================================================================
// TryFrom<i32> for integer-based enums
// ============================================================================

impl TryFrom<i32> for Origin {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Customer),
            1 => Ok(Self::Firm),
            2 => Ok(Self::Unknown),
            _ => Err(v),
        }
    }
}

impl TryFrom<i32> for AuctionStrategy {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Unset),
            1 => Ok(Self::Match),
            2 => Ok(Self::Improvement),
            3 => Ok(Self::Transparent),
            _ => Err(v),
        }
    }
}

impl TryFrom<i32> for LegOpenClose {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Same),
            1 => Ok(Self::Open),
            2 => Ok(Self::Close),
            3 => Ok(Self::Unknown),
            _ => Err(v),
        }
    }
}

impl TryFrom<i32> for MarketDataType {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(Self::RealTime),
            2 => Ok(Self::Frozen),
            3 => Ok(Self::Delayed),
            4 => Ok(Self::DelayedFrozen),
            _ => Err(v),
        }
    }
}

impl TryFrom<i32> for FaDataType {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(Self::Groups),
            3 => Ok(Self::Aliases),
            _ => Err(v),
        }
    }
}

impl TryFrom<i32> for TriggerMethod {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Default),
            1 => Ok(Self::DoubleBidAsk),
            2 => Ok(Self::Last),
            3 => Ok(Self::DoubleLast),
            4 => Ok(Self::BidAsk),
            7 => Ok(Self::LastOrBidAsk),
            8 => Ok(Self::MidPoint),
            _ => Err(v),
        }
    }
}

impl TryFrom<i32> for OrderConditionType {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(Self::Price),
            3 => Ok(Self::Time),
            4 => Ok(Self::Margin),
            5 => Ok(Self::Execution),
            6 => Ok(Self::Volume),
            7 => Ok(Self::PercentChange),
            _ => Err(v),
        }
    }
}

impl TryFrom<i32> for UsePriceMgmtAlgo {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::DontUse),
            1 => Ok(Self::Use),
            2 => Ok(Self::Default),
            _ => Err(v),
        }
    }
}

impl TryFrom<i32> for OptionExerciseType {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            -1 => Ok(Self::None),
            1 => Ok(Self::Exercise),
            2 => Ok(Self::Lapse),
            3 => Ok(Self::DoNothing),
            100 => Ok(Self::Assigned),
            101 => Ok(Self::AutoexerciseClearing),
            102 => Ok(Self::Expired),
            103 => Ok(Self::Netting),
            200 => Ok(Self::AutoexerciseTrading),
            _ => Err(v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sec_type_display_round_trip() {
        let types = vec![
            (SecType::Stock, "STK"),
            (SecType::Option, "OPT"),
            (SecType::Future, "FUT"),
            (SecType::Forex, "CASH"),
            (SecType::Index, "IND"),
            (SecType::Crypto, "CRYPTO"),
        ];
        for (variant, expected) in types {
            assert_eq!(variant.to_string(), expected);
            assert_eq!(SecType::from_str(expected).unwrap(), variant);
        }
    }

    #[test]
    fn sec_type_other_variant() {
        let parsed: SecType = SecType::from_str("UNKNOWN_TYPE").unwrap();
        assert_eq!(parsed, SecType::Other("UNKNOWN_TYPE".to_string()));
        assert_eq!(parsed.to_string(), "UNKNOWN_TYPE");
    }

    #[test]
    fn order_type_display_round_trip() {
        let types = vec![
            (OrderType::Market, "MKT"),
            (OrderType::Limit, "LMT"),
            (OrderType::StopLimit, "STP LMT"),
            (OrderType::TrailingStop, "TRAIL"),
            (OrderType::MidPrice, "MIDPRICE"),
        ];
        for (variant, expected) in types {
            assert_eq!(variant.to_string(), expected);
            assert_eq!(OrderType::from_str(expected).unwrap(), variant);
        }
    }

    #[test]
    fn action_from_str() {
        assert_eq!(Action::from_str("BUY").unwrap(), Action::Buy);
        assert_eq!(Action::from_str("SELL").unwrap(), Action::Sell);
        assert!(Action::from_str("INVALID").is_err());
    }

    #[test]
    fn origin_default() {
        assert_eq!(Origin::default(), Origin::Customer);
    }

    #[test]
    fn right_display() {
        assert_eq!(Right::Call.to_string(), "C");
        assert_eq!(Right::Put.to_string(), "P");
        assert_eq!(Right::from_str("CALL").unwrap(), Right::Call);
    }
}
