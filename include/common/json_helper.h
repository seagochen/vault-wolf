//
// VaultWolf JSON Helper
// Simple JSON serialization without external dependencies
// Author: VaultWolf Team
// Date: 2025-11-21
//

#ifndef VAULTWOLF_JSON_HELPER_H
#define VAULTWOLF_JSON_HELPER_H

#include "data_types.h"
#include <sstream>
#include <iomanip>
#include <algorithm>

namespace VaultWolf {
namespace JSON {

/**
 * @brief Escape JSON string
 */
inline std::string escape(const std::string& str) {
    std::ostringstream oss;
    for (char c : str) {
        switch (c) {
            case '"': oss << "\\\""; break;
            case '\\': oss << "\\\\"; break;
            case '\b': oss << "\\b"; break;
            case '\f': oss << "\\f"; break;
            case '\n': oss << "\\n"; break;
            case '\r': oss << "\\r"; break;
            case '\t': oss << "\\t"; break;
            default:
                if ('\x00' <= c && c <= '\x1f') {
                    oss << "\\u" << std::hex << std::setw(4)
                        << std::setfill('0') << static_cast<int>(c);
                } else {
                    oss << c;
                }
        }
    }
    return oss.str();
}

/**
 * @brief Convert double to string with proper formatting
 */
inline std::string doubleToStr(double val) {
    if (val == 0.0) return "0.0";
    std::ostringstream oss;
    oss << std::fixed << std::setprecision(6) << val;
    std::string str = oss.str();
    // Remove trailing zeros
    str.erase(str.find_last_not_of('0') + 1, std::string::npos);
    if (str.back() == '.') str += '0';
    return str;
}

/**
 * @brief Serialize TickData to JSON
 */
inline std::string toJSON(const TickData& tick) {
    std::ostringstream oss;
    oss << "{\n";
    oss << "  \"symbol\": \"" << escape(tick.symbol) << "\",\n";
    oss << "  \"secType\": \"" << escape(tick.secType) << "\",\n";
    oss << "  \"reqId\": " << tick.reqId << ",\n";
    oss << "  \"bid\": " << doubleToStr(tick.bid) << ",\n";
    oss << "  \"ask\": " << doubleToStr(tick.ask) << ",\n";
    oss << "  \"last\": " << doubleToStr(tick.last) << ",\n";
    oss << "  \"close\": " << doubleToStr(tick.close) << ",\n";
    oss << "  \"open\": " << doubleToStr(tick.open) << ",\n";
    oss << "  \"high\": " << doubleToStr(tick.high) << ",\n";
    oss << "  \"low\": " << doubleToStr(tick.low) << ",\n";
    oss << "  \"bidSize\": " << tick.bidSize << ",\n";
    oss << "  \"askSize\": " << tick.askSize << ",\n";
    oss << "  \"lastSize\": " << tick.lastSize << ",\n";
    oss << "  \"volume\": " << tick.volume << ",\n";
    oss << "  \"timestamp\": \"" << escape(tick.timestamp) << "\"";

    // Add option-specific data if available
    if (tick.secType == "OPT") {
        oss << ",\n";
        oss << "  \"impliedVol\": " << doubleToStr(tick.impliedVol) << ",\n";
        oss << "  \"delta\": " << doubleToStr(tick.delta) << ",\n";
        oss << "  \"gamma\": " << doubleToStr(tick.gamma) << ",\n";
        oss << "  \"vega\": " << doubleToStr(tick.vega) << ",\n";
        oss << "  \"theta\": " << doubleToStr(tick.theta) << ",\n";
        oss << "  \"optPrice\": " << doubleToStr(tick.optPrice) << ",\n";
        oss << "  \"undPrice\": " << doubleToStr(tick.undPrice) << "\n";
    } else {
        oss << "\n";
    }

    oss << "}";
    return oss.str();
}

/**
 * @brief Serialize HistoricalBar to JSON
 */
inline std::string toJSON(const HistoricalBar& bar) {
    std::ostringstream oss;
    oss << "{\n";
    oss << "  \"date\": \"" << escape(bar.date) << "\",\n";
    oss << "  \"open\": " << doubleToStr(bar.open) << ",\n";
    oss << "  \"high\": " << doubleToStr(bar.high) << ",\n";
    oss << "  \"low\": " << doubleToStr(bar.low) << ",\n";
    oss << "  \"close\": " << doubleToStr(bar.close) << ",\n";
    oss << "  \"volume\": " << bar.volume << ",\n";
    oss << "  \"barCount\": " << bar.barCount << ",\n";
    oss << "  \"wap\": " << doubleToStr(bar.wap) << "\n";
    oss << "}";
    return oss.str();
}

/**
 * @brief Serialize HistoricalData to JSON
 */
inline std::string toJSON(const HistoricalData& hist) {
    std::ostringstream oss;
    oss << "{\n";
    oss << "  \"symbol\": \"" << escape(hist.symbol) << "\",\n";
    oss << "  \"secType\": \"" << escape(hist.secType) << "\",\n";
    oss << "  \"reqId\": " << hist.reqId << ",\n";
    oss << "  \"startDate\": \"" << escape(hist.startDate) << "\",\n";
    oss << "  \"endDate\": \"" << escape(hist.endDate) << "\",\n";
    oss << "  \"bars\": [\n";

    for (size_t i = 0; i < hist.bars.size(); ++i) {
        oss << "    " << toJSON(hist.bars[i]);
        if (i < hist.bars.size() - 1) oss << ",";
        oss << "\n";
    }

    oss << "  ]\n";
    oss << "}";
    return oss.str();
}

/**
 * @brief Serialize Position to JSON
 */
inline std::string toJSON(const Position& pos) {
    std::ostringstream oss;
    oss << "{\n";
    oss << "  \"account\": \"" << escape(pos.account) << "\",\n";
    oss << "  \"symbol\": \"" << escape(pos.symbol) << "\",\n";
    oss << "  \"secType\": \"" << escape(pos.secType) << "\",\n";
    oss << "  \"currency\": \"" << escape(pos.currency) << "\",\n";
    oss << "  \"exchange\": \"" << escape(pos.exchange) << "\",\n";
    oss << "  \"position\": " << doubleToStr(pos.position) << ",\n";
    oss << "  \"avgCost\": " << doubleToStr(pos.avgCost) << ",\n";
    oss << "  \"marketPrice\": " << doubleToStr(pos.marketPrice) << ",\n";
    oss << "  \"marketValue\": " << doubleToStr(pos.marketValue) << ",\n";
    oss << "  \"unrealizedPNL\": " << doubleToStr(pos.unrealizedPNL) << ",\n";
    oss << "  \"realizedPNL\": " << doubleToStr(pos.realizedPNL) << "\n";
    oss << "}";
    return oss.str();
}

/**
 * @brief Serialize AccountSummary to JSON
 */
inline std::string toJSON(const AccountSummary& acc) {
    std::ostringstream oss;
    oss << "{\n";
    oss << "  \"account\": \"" << escape(acc.account) << "\",\n";
    oss << "  \"values\": {\n";

    size_t count = 0;
    for (const auto& kv : acc.values) {
        oss << "    \"" << escape(kv.first) << "\": \"" << escape(kv.second) << "\"";
        if (++count < acc.values.size()) oss << ",";
        oss << "\n";
    }

    oss << "  }\n";
    oss << "}";
    return oss.str();
}

/**
 * @brief Serialize OrderInfo to JSON
 */
inline std::string toJSON(const OrderInfo& order) {
    std::ostringstream oss;
    oss << "{\n";
    oss << "  \"orderId\": " << order.orderId << ",\n";
    oss << "  \"account\": \"" << escape(order.account) << "\",\n";
    oss << "  \"symbol\": \"" << escape(order.symbol) << "\",\n";
    oss << "  \"secType\": \"" << escape(order.secType) << "\",\n";
    oss << "  \"exchange\": \"" << escape(order.exchange) << "\",\n";
    oss << "  \"currency\": \"" << escape(order.currency) << "\",\n";
    oss << "  \"action\": \"" << escape(order.action) << "\",\n";
    oss << "  \"orderType\": \"" << escape(order.orderType) << "\",\n";
    oss << "  \"totalQuantity\": " << doubleToStr(order.totalQuantity) << ",\n";
    oss << "  \"lmtPrice\": " << doubleToStr(order.lmtPrice) << ",\n";
    oss << "  \"auxPrice\": " << doubleToStr(order.auxPrice) << ",\n";
    oss << "  \"status\": \"" << escape(order.status) << "\",\n";
    oss << "  \"filled\": " << doubleToStr(order.filled) << ",\n";
    oss << "  \"remaining\": " << doubleToStr(order.remaining) << ",\n";
    oss << "  \"avgFillPrice\": " << doubleToStr(order.avgFillPrice) << ",\n";
    oss << "  \"permId\": " << order.permId << ",\n";
    oss << "  \"parentId\": " << order.parentId << ",\n";
    oss << "  \"lastFillPrice\": " << doubleToStr(order.lastFillPrice) << ",\n";

    // Option-specific fields
    if (order.secType == "OPT") {
        oss << "  \"right\": \"" << escape(order.right) << "\",\n";
        oss << "  \"strike\": " << doubleToStr(order.strike) << ",\n";
        oss << "  \"expiry\": \"" << escape(order.expiry) << "\",\n";
    }

    oss << "  \"submitTime\": \"" << escape(order.submitTime) << "\",\n";
    oss << "  \"lastUpdateTime\": \"" << escape(order.lastUpdateTime) << "\"\n";
    oss << "}";
    return oss.str();
}

/**
 * @brief Serialize vector of items to JSON array
 */
template<typename T>
inline std::string toJSONArray(const std::vector<T>& items) {
    std::ostringstream oss;
    oss << "[\n";
    for (size_t i = 0; i < items.size(); ++i) {
        oss << toJSON(items[i]);
        if (i < items.size() - 1) oss << ",";
        oss << "\n";
    }
    oss << "]";
    return oss.str();
}

/**
 * @brief Create success response JSON
 */
inline std::string successResponse(const std::string& message, const std::string& data = "") {
    std::ostringstream oss;
    oss << "{\n";
    oss << "  \"success\": true,\n";
    oss << "  \"message\": \"" << escape(message) << "\"";
    if (!data.empty()) {
        oss << ",\n";
        oss << "  \"data\": " << data << "\n";
    } else {
        oss << "\n";
    }
    oss << "}";
    return oss.str();
}

/**
 * @brief Create error response JSON
 */
inline std::string errorResponse(const std::string& message, int errorCode = 0) {
    std::ostringstream oss;
    oss << "{\n";
    oss << "  \"success\": false,\n";
    oss << "  \"message\": \"" << escape(message) << "\",\n";
    oss << "  \"errorCode\": " << errorCode << "\n";
    oss << "}";
    return oss.str();
}

} // namespace JSON
} // namespace VaultWolf

#endif // VAULTWOLF_JSON_HELPER_H
