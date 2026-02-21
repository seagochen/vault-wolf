//
// VaultWolf Common Data Types
// Author: VaultWolf Team
// Date: 2025-11-21
//

#ifndef VAULTWOLF_DATA_TYPES_H
#define VAULTWOLF_DATA_TYPES_H

#include <string>
#include <vector>
#include <map>
#include <memory>
#include <chrono>

namespace VaultWolf {

// ============================================================================
// Market Data Structures
// ============================================================================

/**
 * @brief Real-time tick data for stocks/options
 */
struct TickData {
    std::string symbol;
    std::string secType;      // STK, OPT, etc.
    long reqId;

    // Price data
    double bid = 0.0;
    double ask = 0.0;
    double last = 0.0;
    double close = 0.0;
    double open = 0.0;
    double high = 0.0;
    double low = 0.0;

    // Volume data
    long bidSize = 0;
    long askSize = 0;
    long lastSize = 0;
    long volume = 0;

    // Option-specific data
    double impliedVol = 0.0;
    double delta = 0.0;
    double gamma = 0.0;
    double vega = 0.0;
    double theta = 0.0;
    double optPrice = 0.0;
    double pvDividend = 0.0;
    double undPrice = 0.0;

    // Timestamp
    std::string timestamp;

    TickData() = default;
};

/**
 * @brief Historical bar data
 */
struct HistoricalBar {
    std::string date;
    double open = 0.0;
    double high = 0.0;
    double low = 0.0;
    double close = 0.0;
    long volume = 0;
    int barCount = 0;
    double wap = 0.0;  // Weighted average price

    HistoricalBar() = default;
};

/**
 * @brief Historical data response
 */
struct HistoricalData {
    std::string symbol;
    std::string secType;
    long reqId;
    std::string startDate;
    std::string endDate;
    std::vector<HistoricalBar> bars;

    HistoricalData() = default;
};

// ============================================================================
// Account Data Structures
// ============================================================================

/**
 * @brief Account summary information
 */
struct AccountSummary {
    std::string account;
    std::map<std::string, std::string> values;  // tag -> value

    AccountSummary() = default;
};

/**
 * @brief Position information
 */
struct Position {
    std::string account;
    std::string symbol;
    std::string secType;
    std::string currency;
    std::string exchange;

    double position = 0.0;
    double avgCost = 0.0;
    double marketPrice = 0.0;
    double marketValue = 0.0;
    double unrealizedPNL = 0.0;
    double realizedPNL = 0.0;

    Position() = default;
};

/**
 * @brief Portfolio information (similar to Position but with more details)
 */
struct PortfolioItem {
    std::string account;
    std::string symbol;
    std::string secType;
    std::string currency;

    double position = 0.0;
    double marketPrice = 0.0;
    double marketValue = 0.0;
    double averageCost = 0.0;
    double unrealizedPNL = 0.0;
    double realizedPNL = 0.0;

    PortfolioItem() = default;
};

// ============================================================================
// Order Data Structures
// ============================================================================

/**
 * @brief Order status
 */
enum class OrderStatus {
    PendingSubmit,
    PendingCancel,
    PreSubmitted,
    Submitted,
    ApiCancelled,
    Cancelled,
    Filled,
    Inactive,
    Unknown
};

/**
 * @brief Order information
 */
struct OrderInfo {
    long orderId = 0;
    std::string account;
    std::string symbol;
    std::string secType;
    std::string exchange;
    std::string currency;

    // Order details
    std::string action;           // BUY, SELL
    std::string orderType;        // MKT, LMT, STP, etc.
    double totalQuantity = 0.0;
    double lmtPrice = 0.0;
    double auxPrice = 0.0;        // Stop price for stop orders

    // Status
    std::string status;
    double filled = 0.0;
    double remaining = 0.0;
    double avgFillPrice = 0.0;
    long permId = 0;
    long parentId = 0;
    double lastFillPrice = 0.0;

    // Option-specific
    std::string right;            // C (Call) or P (Put)
    double strike = 0.0;
    std::string expiry;

    // Timestamps
    std::string submitTime;
    std::string lastUpdateTime;

    OrderInfo() = default;
};

/**
 * @brief Order execution details
 */
struct Execution {
    long orderId = 0;
    std::string execId;
    std::string time;
    std::string account;
    std::string exchange;
    std::string side;
    double shares = 0.0;
    double price = 0.0;
    long permId = 0;
    long clientId = 0;
    double avgPrice = 0.0;

    Execution() = default;
};

// ============================================================================
// Request/Response Wrappers
// ============================================================================

/**
 * @brief Generic API request
 */
struct ApiRequest {
    std::string endpoint;
    std::map<std::string, std::string> params;

    ApiRequest() = default;
};

/**
 * @brief Generic API response
 */
struct ApiResponse {
    bool success = false;
    std::string message;
    std::string data;  // JSON string
    int errorCode = 0;

    ApiResponse() = default;

    ApiResponse(bool s, const std::string& msg, const std::string& d = "")
        : success(s), message(msg), data(d) {}
};

// ============================================================================
// Contract Specification
// ============================================================================

/**
 * @brief Simple contract specification
 */
struct ContractSpec {
    std::string symbol;
    std::string secType;      // STK, OPT, FUT, etc.
    std::string currency;     // USD, EUR, etc.
    std::string exchange;     // SMART, ISLAND, etc.

    // For options
    std::string right;        // C (Call) or P (Put)
    double strike = 0.0;
    std::string expiry;       // YYYYMMDD format

    // For futures
    std::string lastTradeDateOrContractMonth;

    ContractSpec() : currency("USD"), exchange("SMART") {}
};

} // namespace VaultWolf

#endif // VAULTWOLF_DATA_TYPES_H
