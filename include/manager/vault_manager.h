//
// VaultWolf Manager
// High-level manager that wraps VaultEWrapper and provides data storage
// Author: VaultWolf Team
// Date: 2025-11-21
//

#ifndef VAULTWOLF_MANAGER_H
#define VAULTWOLF_MANAGER_H

#include "ibwrapper/vault_ewrapper.h"
#include "common/data_types.h"
#include "Contract.h"
#include "Order.h"

#include <mutex>
#include <thread>
#include <atomic>
#include <condition_variable>

namespace VaultWolf {

/**
 * @brief VaultWolfManager - High-level API manager
 *
 * This class extends VaultEWrapper to add data storage and retrieval capabilities.
 * It maintains thread-safe storage for market data, orders, and account information.
 */
class VaultWolfManager : public VaultEWrapper {
public:
    VaultWolfManager();
    ~VaultWolfManager() override;

    // ========================================================================
    // Connection Management
    // ========================================================================

    bool connectToIB(const char* host, int port, int clientId = 0);
    void disconnectFromIB();
    void startMessageProcessing();
    void stopMessageProcessing();

    // ========================================================================
    // Market Data APIs
    // ========================================================================

    /**
     * @brief Request real-time market data for a contract
     * @param contract Contract specification
     * @return Request ID for tracking
     */
    long requestMarketData(const ContractSpec& contract);

    /**
     * @brief Cancel market data subscription
     * @param reqId Request ID returned from requestMarketData
     */
    void cancelMarketData(long reqId);

    /**
     * @brief Get latest tick data for a symbol
     * @param symbol Symbol to query
     * @param secType Security type (STK, OPT, etc.)
     * @return TickData or nullptr if not found
     */
    std::shared_ptr<TickData> getTickData(const std::string& symbol, const std::string& secType);

    /**
     * @brief Request historical data
     * @param contract Contract specification
     * @param endDateTime End date/time (format: yyyyMMdd HH:mm:ss)
     * @param duration Duration string (e.g., "1 D", "1 W", "1 M", "1 Y")
     * @param barSize Bar size (e.g., "1 min", "5 mins", "1 hour", "1 day")
     * @param whatToShow What data to show (TRADES, MIDPOINT, BID, ASK, etc.)
     * @return Request ID for tracking
     */
    long requestHistoricalData(
        const ContractSpec& contract,
        const std::string& endDateTime,
        const std::string& duration,
        const std::string& barSize,
        const std::string& whatToShow = "TRADES"
    );

    /**
     * @brief Get historical data for a request
     * @param reqId Request ID
     * @return HistoricalData or nullptr if not found
     */
    std::shared_ptr<HistoricalData> getHistoricalData(long reqId);

    // ========================================================================
    // Account APIs
    // ========================================================================

    /**
     * @brief Request account summary
     * @param tags Tags to request (comma-separated), or "All"
     */
    void requestAccountSummary(const std::string& tags = "All");

    /**
     * @brief Get account summary
     * @param account Account ID (if empty, returns first account)
     * @return AccountSummary or nullptr
     */
    std::shared_ptr<AccountSummary> getAccountSummary(const std::string& account = "");

    /**
     * @brief Request positions
     */
    void requestPositions();

    /**
     * @brief Get all positions
     * @return Vector of positions
     */
    std::vector<Position> getAllPositions();

    /**
     * @brief Get positions for a specific account
     * @param account Account ID
     * @return Vector of positions
     */
    std::vector<Position> getPositionsByAccount(const std::string& account);

    /**
     * @brief Get positions for a specific symbol
     * @param symbol Symbol
     * @param secType Security type
     * @return Vector of positions
     */
    std::vector<Position> getPositionsBySymbol(const std::string& symbol, const std::string& secType);

    // ========================================================================
    // Order APIs
    // ========================================================================

    /**
     * @brief Place a market order
     * @param contract Contract specification
     * @param action BUY or SELL
     * @param quantity Order quantity
     * @return Order ID
     */
    long placeMarketOrder(const ContractSpec& contract, const std::string& action, double quantity);

    /**
     * @brief Place a limit order
     * @param contract Contract specification
     * @param action BUY or SELL
     * @param quantity Order quantity
     * @param limitPrice Limit price
     * @return Order ID
     */
    long placeLimitOrder(const ContractSpec& contract, const std::string& action,
                         double quantity, double limitPrice);

    /**
     * @brief Place a stop order
     * @param contract Contract specification
     * @param action BUY or SELL
     * @param quantity Order quantity
     * @param stopPrice Stop price
     * @return Order ID
     */
    long placeStopOrder(const ContractSpec& contract, const std::string& action,
                        double quantity, double stopPrice);

    /**
     * @brief Cancel an order
     * @param orderId Order ID to cancel
     */
    void cancelOrder(long orderId);

    /**
     * @brief Modify an order
     * @param orderId Order ID to modify
     * @param quantity New quantity
     * @param limitPrice New limit price (0 for market orders)
     * @param stopPrice New stop price (for stop orders)
     */
    void modifyOrder(long orderId, double quantity, double limitPrice, double stopPrice);

    /**
     * @brief Request all open orders
     */
    void requestOpenOrders();

    /**
     * @brief Request completed orders (today's fills)
     */
    void requestCompletedOrders();

    /**
     * @brief Get order by ID
     * @param orderId Order ID
     * @return OrderInfo or nullptr
     */
    std::shared_ptr<OrderInfo> getOrder(long orderId);

    /**
     * @brief Get all orders
     * @return Vector of orders
     */
    std::vector<OrderInfo> getAllOrders();

    /**
     * @brief Get orders by status
     * @param status Status filter (e.g., "Filled", "Submitted", "Cancelled")
     * @return Vector of orders
     */
    std::vector<OrderInfo> getOrdersByStatus(const std::string& status);

    /**
     * @brief Get orders by symbol
     * @param symbol Symbol
     * @param secType Security type
     * @return Vector of orders
     */
    std::vector<OrderInfo> getOrdersBySymbol(const std::string& symbol, const std::string& secType);

    // ========================================================================
    // Utility Functions
    // ========================================================================

    /**
     * @brief Get next valid order ID
     */
    long getNextOrderId();

    /**
     * @brief Get all managed accounts
     */
    std::vector<std::string> getManagedAccounts();

protected:
    // ========================================================================
    // Override EWrapper callbacks to store data
    // ========================================================================

    void tickPrice(TickerId tickerId, TickType field, double price, const TickAttrib& attrib) override;
    void tickSize(TickerId tickerId, TickType field, Decimal size) override;
    void tickString(TickerId tickerId, TickType tickType, const std::string& value) override;
    void tickGeneric(TickerId tickerId, TickType tickType, double value) override;
    void tickOptionComputation(TickerId tickerId, TickType tickType, int tickAttrib,
        double impliedVol, double delta, double optPrice, double pvDividend,
        double gamma, double vega, double theta, double undPrice) override;

    void historicalData(TickerId reqId, const Bar& bar) override;
    void historicalDataEnd(int reqId, const std::string& startDateStr, const std::string& endDateStr) override;

    void accountSummary(int reqId, const std::string& account, const std::string& tag,
        const std::string& value, const std::string& currency) override;
    void accountSummaryEnd(int reqId) override;

    void position(const std::string& account, const Contract& contract, Decimal position, double avgCost) override;
    void positionEnd() override;

    void orderStatus(OrderId orderId, const std::string& status, Decimal filled,
        Decimal remaining, double avgFillPrice, int permId, int parentId,
        double lastFillPrice, int clientId, const std::string& whyHeld, double mktCapPrice) override;

    void openOrder(OrderId orderId, const Contract& contract, const Order& order, const OrderState& orderState) override;
    void openOrderEnd() override;

    void nextValidId(OrderId orderId) override;
    void managedAccounts(const std::string& accountsList) override;

private:
    // ========================================================================
    // Helper Functions
    // ========================================================================

    Contract createContract(const ContractSpec& spec);
    std::string contractKey(const std::string& symbol, const std::string& secType);

    // ========================================================================
    // Data Storage (Thread-Safe)
    // ========================================================================

    // Market data
    std::map<std::string, std::shared_ptr<TickData>> m_tickDataMap;
    std::map<long, std::shared_ptr<HistoricalData>> m_historicalDataMap;
    std::map<long, ContractSpec> m_reqIdToContractMap;

    // Account data
    std::map<std::string, std::shared_ptr<AccountSummary>> m_accountSummaryMap;
    std::vector<Position> m_positions;

    // Order data
    std::map<long, std::shared_ptr<OrderInfo>> m_orderMap;
    std::map<long, Contract> m_orderIdToContractMap;
    std::map<long, Order> m_orderIdToOrderMap;

    // Managed accounts
    std::vector<std::string> m_managedAccounts;

    // Thread safety
    mutable std::mutex m_tickDataMutex;
    mutable std::mutex m_historicalDataMutex;
    mutable std::mutex m_accountMutex;
    mutable std::mutex m_positionMutex;
    mutable std::mutex m_orderMutex;

    // Message processing thread
    std::unique_ptr<std::thread> m_processingThread;
    std::atomic<bool> m_isProcessing;

    // Order ID management
    std::atomic<long> m_nextOrderId;
};

} // namespace VaultWolf

#endif // VAULTWOLF_MANAGER_H
