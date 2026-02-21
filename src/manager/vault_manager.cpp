//
// VaultWolf Manager Implementation
// Author: VaultWolf Team
// Date: 2025-11-21
//

#include "manager/vault_manager.h"
#include "ibwrapper/vault_contracts.h"
#include "ibwrapper/vault_orders.h"
#include "EClientSocket.h"
#include "Order.h"
#include "OrderState.h"
#include "OrderCancel.h"
#include <chrono>
#include <ctime>
#include <iomanip>
#include <sstream>

using namespace VaultWolf;

// ============================================================================
// Constructor / Destructor
// ============================================================================

VaultWolfManager::VaultWolfManager()
    : VaultEWrapper()
    , m_isProcessing(false)
    , m_nextOrderId(1)
{
}

VaultWolfManager::~VaultWolfManager() {
    stopMessageProcessing();
    if (isConnected()) {
        disconnectFromIB();
    }
}

// ============================================================================
// Connection Management
// ============================================================================

bool VaultWolfManager::connectToIB(const char* host, int port, int clientId) {
    return connect(host, port, clientId);
}

void VaultWolfManager::disconnectFromIB() {
    disconnect();
}

void VaultWolfManager::startMessageProcessing() {
    if (m_isProcessing.load()) {
        return;  // Already running
    }

    m_isProcessing.store(true);
    m_processingThread = std::make_unique<std::thread>([this]() {
        while (m_isProcessing.load() && isConnected()) {
            processMessages();
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
    });
}

void VaultWolfManager::stopMessageProcessing() {
    if (!m_isProcessing.load()) {
        return;
    }

    m_isProcessing.store(false);
    if (m_processingThread && m_processingThread->joinable()) {
        m_processingThread->join();
    }
}

// ============================================================================
// Market Data APIs
// ============================================================================

long VaultWolfManager::requestMarketData(const ContractSpec& contract) {
    static long reqId = 1000;
    long currentReqId = reqId++;

    Contract ibContract = createContract(contract);

    // Store the contract mapping
    {
        std::lock_guard<std::mutex> lock(m_tickDataMutex);
        m_reqIdToContractMap[currentReqId] = contract;

        // Initialize tick data structure
        auto tickData = std::make_shared<TickData>();
        tickData->symbol = contract.symbol;
        tickData->secType = contract.secType;
        tickData->reqId = currentReqId;

        std::string key = contractKey(contract.symbol, contract.secType);
        m_tickDataMap[key] = tickData;
    }

    // Request market data from IB
    m_pClient->reqMktData(currentReqId, ibContract, "", false, false, TagValueListSPtr());

    return currentReqId;
}

void VaultWolfManager::cancelMarketData(long reqId) {
    m_pClient->cancelMktData(reqId);

    // Clean up stored data
    std::lock_guard<std::mutex> lock(m_tickDataMutex);
    auto it = m_reqIdToContractMap.find(reqId);
    if (it != m_reqIdToContractMap.end()) {
        std::string key = contractKey(it->second.symbol, it->second.secType);
        m_tickDataMap.erase(key);
        m_reqIdToContractMap.erase(it);
    }
}

std::shared_ptr<TickData> VaultWolfManager::getTickData(const std::string& symbol, const std::string& secType) {
    std::lock_guard<std::mutex> lock(m_tickDataMutex);
    std::string key = contractKey(symbol, secType);
    auto it = m_tickDataMap.find(key);
    if (it != m_tickDataMap.end()) {
        return it->second;
    }
    return nullptr;
}

long VaultWolfManager::requestHistoricalData(
    const ContractSpec& contract,
    const std::string& endDateTime,
    const std::string& duration,
    const std::string& barSize,
    const std::string& whatToShow)
{
    static long reqId = 2000;
    long currentReqId = reqId++;

    Contract ibContract = createContract(contract);

    // Store the contract mapping and initialize historical data structure
    {
        std::lock_guard<std::mutex> lock(m_historicalDataMutex);
        m_reqIdToContractMap[currentReqId] = contract;

        auto histData = std::make_shared<HistoricalData>();
        histData->symbol = contract.symbol;
        histData->secType = contract.secType;
        histData->reqId = currentReqId;
        m_historicalDataMap[currentReqId] = histData;
    }

    // Request historical data from IB
    m_pClient->reqHistoricalData(
        currentReqId,
        ibContract,
        endDateTime,
        duration,
        barSize,
        whatToShow,
        1,  // useRTH (regular trading hours)
        1,  // formatDate (1 = yyyyMMdd HH:mm:ss)
        false,  // keepUpToDate
        TagValueListSPtr()
    );

    return currentReqId;
}

std::shared_ptr<HistoricalData> VaultWolfManager::getHistoricalData(long reqId) {
    std::lock_guard<std::mutex> lock(m_historicalDataMutex);
    auto it = m_historicalDataMap.find(reqId);
    if (it != m_historicalDataMap.end()) {
        return it->second;
    }
    return nullptr;
}

// ============================================================================
// Account APIs
// ============================================================================

void VaultWolfManager::requestAccountSummary(const std::string& tags) {
    m_pClient->reqAccountSummary(9001, "All", tags);
}

std::shared_ptr<AccountSummary> VaultWolfManager::getAccountSummary(const std::string& account) {
    std::lock_guard<std::mutex> lock(m_accountMutex);

    if (account.empty() && !m_accountSummaryMap.empty()) {
        // Return first account
        return m_accountSummaryMap.begin()->second;
    }

    auto it = m_accountSummaryMap.find(account);
    if (it != m_accountSummaryMap.end()) {
        return it->second;
    }
    return nullptr;
}

void VaultWolfManager::requestPositions() {
    m_pClient->reqPositions();
}

std::vector<Position> VaultWolfManager::getAllPositions() {
    std::lock_guard<std::mutex> lock(m_positionMutex);
    return m_positions;
}

std::vector<Position> VaultWolfManager::getPositionsByAccount(const std::string& account) {
    std::lock_guard<std::mutex> lock(m_positionMutex);
    std::vector<Position> result;
    for (const auto& pos : m_positions) {
        if (pos.account == account) {
            result.push_back(pos);
        }
    }
    return result;
}

std::vector<Position> VaultWolfManager::getPositionsBySymbol(const std::string& symbol, const std::string& secType) {
    std::lock_guard<std::mutex> lock(m_positionMutex);
    std::vector<Position> result;
    for (const auto& pos : m_positions) {
        if (pos.symbol == symbol && pos.secType == secType) {
            result.push_back(pos);
        }
    }
    return result;
}

// ============================================================================
// Order APIs
// ============================================================================

long VaultWolfManager::placeMarketOrder(const ContractSpec& contract, const std::string& action, double quantity) {
    long orderId = getNextOrderId();
    Contract ibContract = createContract(contract);
    Order order = OrderSamples::MarketOrder(action, quantity);

    // Store order info
    {
        std::lock_guard<std::mutex> lock(m_orderMutex);
        m_orderIdToContractMap[orderId] = ibContract;
        m_orderIdToOrderMap[orderId] = order;

        auto orderInfo = std::make_shared<OrderInfo>();
        orderInfo->orderId = orderId;
        orderInfo->symbol = contract.symbol;
        orderInfo->secType = contract.secType;
        orderInfo->exchange = contract.exchange;
        orderInfo->currency = contract.currency;
        orderInfo->action = action;
        orderInfo->orderType = "MKT";
        orderInfo->totalQuantity = quantity;
        orderInfo->status = "PendingSubmit";

        // Option-specific
        if (contract.secType == "OPT") {
            orderInfo->right = contract.right;
            orderInfo->strike = contract.strike;
            orderInfo->expiry = contract.expiry;
        }

        m_orderMap[orderId] = orderInfo;
    }

    m_pClient->placeOrder(orderId, ibContract, order);
    return orderId;
}

long VaultWolfManager::placeLimitOrder(const ContractSpec& contract, const std::string& action,
                                       double quantity, double limitPrice) {
    long orderId = getNextOrderId();
    Contract ibContract = createContract(contract);
    Order order = OrderSamples::LimitOrder(action, quantity, limitPrice);

    // Store order info
    {
        std::lock_guard<std::mutex> lock(m_orderMutex);
        m_orderIdToContractMap[orderId] = ibContract;
        m_orderIdToOrderMap[orderId] = order;

        auto orderInfo = std::make_shared<OrderInfo>();
        orderInfo->orderId = orderId;
        orderInfo->symbol = contract.symbol;
        orderInfo->secType = contract.secType;
        orderInfo->exchange = contract.exchange;
        orderInfo->currency = contract.currency;
        orderInfo->action = action;
        orderInfo->orderType = "LMT";
        orderInfo->totalQuantity = quantity;
        orderInfo->lmtPrice = limitPrice;
        orderInfo->status = "PendingSubmit";

        if (contract.secType == "OPT") {
            orderInfo->right = contract.right;
            orderInfo->strike = contract.strike;
            orderInfo->expiry = contract.expiry;
        }

        m_orderMap[orderId] = orderInfo;
    }

    m_pClient->placeOrder(orderId, ibContract, order);
    return orderId;
}

long VaultWolfManager::placeStopOrder(const ContractSpec& contract, const std::string& action,
                                      double quantity, double stopPrice) {
    long orderId = getNextOrderId();
    Contract ibContract = createContract(contract);
    Order order = OrderSamples::Stop(action, quantity, stopPrice);

    // Store order info
    {
        std::lock_guard<std::mutex> lock(m_orderMutex);
        m_orderIdToContractMap[orderId] = ibContract;
        m_orderIdToOrderMap[orderId] = order;

        auto orderInfo = std::make_shared<OrderInfo>();
        orderInfo->orderId = orderId;
        orderInfo->symbol = contract.symbol;
        orderInfo->secType = contract.secType;
        orderInfo->exchange = contract.exchange;
        orderInfo->currency = contract.currency;
        orderInfo->action = action;
        orderInfo->orderType = "STP";
        orderInfo->totalQuantity = quantity;
        orderInfo->auxPrice = stopPrice;
        orderInfo->status = "PendingSubmit";

        if (contract.secType == "OPT") {
            orderInfo->right = contract.right;
            orderInfo->strike = contract.strike;
            orderInfo->expiry = contract.expiry;
        }

        m_orderMap[orderId] = orderInfo;
    }

    m_pClient->placeOrder(orderId, ibContract, order);
    return orderId;
}

void VaultWolfManager::cancelOrder(long orderId) {
    OrderCancel orderCancel;
    m_pClient->cancelOrder(orderId, orderCancel);
}

void VaultWolfManager::modifyOrder(long orderId, double quantity, double limitPrice, double stopPrice) {
    std::lock_guard<std::mutex> lock(m_orderMutex);

    auto contractIt = m_orderIdToContractMap.find(orderId);
    auto orderIt = m_orderIdToOrderMap.find(orderId);

    if (contractIt == m_orderIdToContractMap.end() || orderIt == m_orderIdToOrderMap.end()) {
        return;  // Order not found
    }

    Order order = orderIt->second;
    order.totalQuantity = quantity;
    order.lmtPrice = limitPrice;
    order.auxPrice = stopPrice;

    m_orderIdToOrderMap[orderId] = order;
    m_pClient->placeOrder(orderId, contractIt->second, order);
}

void VaultWolfManager::requestOpenOrders() {
    m_pClient->reqOpenOrders();
}

void VaultWolfManager::requestCompletedOrders() {
    m_pClient->reqCompletedOrders(false);
}

std::shared_ptr<OrderInfo> VaultWolfManager::getOrder(long orderId) {
    std::lock_guard<std::mutex> lock(m_orderMutex);
    auto it = m_orderMap.find(orderId);
    if (it != m_orderMap.end()) {
        return it->second;
    }
    return nullptr;
}

std::vector<OrderInfo> VaultWolfManager::getAllOrders() {
    std::lock_guard<std::mutex> lock(m_orderMutex);
    std::vector<OrderInfo> result;
    for (const auto& kv : m_orderMap) {
        result.push_back(*kv.second);
    }
    return result;
}

std::vector<OrderInfo> VaultWolfManager::getOrdersByStatus(const std::string& status) {
    std::lock_guard<std::mutex> lock(m_orderMutex);
    std::vector<OrderInfo> result;
    for (const auto& kv : m_orderMap) {
        if (kv.second->status == status) {
            result.push_back(*kv.second);
        }
    }
    return result;
}

std::vector<OrderInfo> VaultWolfManager::getOrdersBySymbol(const std::string& symbol, const std::string& secType) {
    std::lock_guard<std::mutex> lock(m_orderMutex);
    std::vector<OrderInfo> result;
    for (const auto& kv : m_orderMap) {
        if (kv.second->symbol == symbol && kv.second->secType == secType) {
            result.push_back(*kv.second);
        }
    }
    return result;
}

// ============================================================================
// Utility Functions
// ============================================================================

long VaultWolfManager::getNextOrderId() {
    return m_nextOrderId.fetch_add(1);
}

std::vector<std::string> VaultWolfManager::getManagedAccounts() {
    return m_managedAccounts;
}

// ============================================================================
// EWrapper Callback Overrides
// ============================================================================

void VaultWolfManager::tickPrice(TickerId tickerId, TickType field, double price, const TickAttrib& attrib) {
    // Call parent implementation
    VaultEWrapper::tickPrice(tickerId, field, price, attrib);

    // Store price data
    std::lock_guard<std::mutex> lock(m_tickDataMutex);
    auto contractIt = m_reqIdToContractMap.find(tickerId);
    if (contractIt == m_reqIdToContractMap.end()) return;

    std::string key = contractKey(contractIt->second.symbol, contractIt->second.secType);
    auto it = m_tickDataMap.find(key);
    if (it != m_tickDataMap.end()) {
        auto& tickData = it->second;

        // Update timestamp
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        std::ostringstream oss;
        oss << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
        tickData->timestamp = oss.str();

        // Update prices based on field type
        switch (field) {
            case BID: tickData->bid = price; break;
            case ASK: tickData->ask = price; break;
            case LAST: tickData->last = price; break;
            case CLOSE: tickData->close = price; break;
            case OPEN: tickData->open = price; break;
            case HIGH: tickData->high = price; break;
            case LOW: tickData->low = price; break;
            default: break;
        }
    }
}

void VaultWolfManager::tickSize(TickerId tickerId, TickType field, Decimal size) {
    VaultEWrapper::tickSize(tickerId, field, size);

    std::lock_guard<std::mutex> lock(m_tickDataMutex);
    auto contractIt = m_reqIdToContractMap.find(tickerId);
    if (contractIt == m_reqIdToContractMap.end()) return;

    std::string key = contractKey(contractIt->second.symbol, contractIt->second.secType);
    auto it = m_tickDataMap.find(key);
    if (it != m_tickDataMap.end()) {
        auto& tickData = it->second;

        switch (field) {
            case BID_SIZE: tickData->bidSize = size; break;
            case ASK_SIZE: tickData->askSize = size; break;
            case LAST_SIZE: tickData->lastSize = size; break;
            case VOLUME: tickData->volume = size; break;
            default: break;
        }
    }
}

void VaultWolfManager::tickString(TickerId tickerId, TickType tickType, const std::string& value) {
    VaultEWrapper::tickString(tickerId, tickType, value);
    // Can store timestamp or other string data if needed
}

void VaultWolfManager::tickGeneric(TickerId tickerId, TickType tickType, double value) {
    VaultEWrapper::tickGeneric(tickerId, tickType, value);
    // Can store generic tick data if needed
}

void VaultWolfManager::tickOptionComputation(TickerId tickerId, TickType tickType, int tickAttrib,
    double impliedVol, double delta, double optPrice, double pvDividend,
    double gamma, double vega, double theta, double undPrice)
{
    VaultEWrapper::tickOptionComputation(tickerId, tickType, tickAttrib, impliedVol,
        delta, optPrice, pvDividend, gamma, vega, theta, undPrice);

    std::lock_guard<std::mutex> lock(m_tickDataMutex);
    auto contractIt = m_reqIdToContractMap.find(tickerId);
    if (contractIt == m_reqIdToContractMap.end()) return;

    std::string key = contractKey(contractIt->second.symbol, contractIt->second.secType);
    auto it = m_tickDataMap.find(key);
    if (it != m_tickDataMap.end()) {
        auto& tickData = it->second;
        tickData->impliedVol = impliedVol;
        tickData->delta = delta;
        tickData->gamma = gamma;
        tickData->vega = vega;
        tickData->theta = theta;
        tickData->optPrice = optPrice;
        tickData->pvDividend = pvDividend;
        tickData->undPrice = undPrice;
    }
}

void VaultWolfManager::historicalData(TickerId reqId, const Bar& bar) {
    VaultEWrapper::historicalData(reqId, bar);

    std::lock_guard<std::mutex> lock(m_historicalDataMutex);
    auto it = m_historicalDataMap.find(reqId);
    if (it != m_historicalDataMap.end()) {
        HistoricalBar hBar;
        hBar.date = bar.time;
        hBar.open = bar.open;
        hBar.high = bar.high;
        hBar.low = bar.low;
        hBar.close = bar.close;
        hBar.volume = bar.volume;
        hBar.barCount = bar.count;
        hBar.wap = bar.wap;

        it->second->bars.push_back(hBar);
    }
}

void VaultWolfManager::historicalDataEnd(int reqId, const std::string& startDateStr, const std::string& endDateStr) {
    VaultEWrapper::historicalDataEnd(reqId, startDateStr, endDateStr);

    std::lock_guard<std::mutex> lock(m_historicalDataMutex);
    auto it = m_historicalDataMap.find(reqId);
    if (it != m_historicalDataMap.end()) {
        it->second->startDate = startDateStr;
        it->second->endDate = endDateStr;
    }
}

void VaultWolfManager::accountSummary(int reqId, const std::string& account, const std::string& tag,
    const std::string& value, const std::string& currency)
{
    VaultEWrapper::accountSummary(reqId, account, tag, value, currency);

    std::lock_guard<std::mutex> lock(m_accountMutex);
    auto it = m_accountSummaryMap.find(account);
    if (it == m_accountSummaryMap.end()) {
        auto summary = std::make_shared<AccountSummary>();
        summary->account = account;
        m_accountSummaryMap[account] = summary;
        it = m_accountSummaryMap.find(account);
    }

    it->second->values[tag] = value;
}

void VaultWolfManager::accountSummaryEnd(int reqId) {
    VaultEWrapper::accountSummaryEnd(reqId);
}

void VaultWolfManager::position(const std::string& account, const Contract& contract, Decimal position, double avgCost) {
    VaultEWrapper::position(account, contract, position, avgCost);

    std::lock_guard<std::mutex> lock(m_positionMutex);

    Position pos;
    pos.account = account;
    pos.symbol = contract.symbol;
    pos.secType = contract.secType;
    pos.currency = contract.currency;
    pos.exchange = contract.exchange;
    pos.position = static_cast<double>(position);
    pos.avgCost = avgCost;

    // Check if position already exists and update, or add new
    bool found = false;
    for (auto& p : m_positions) {
        if (p.account == account && p.symbol == contract.symbol && p.secType == contract.secType) {
            p = pos;
            found = true;
            break;
        }
    }

    if (!found) {
        m_positions.push_back(pos);
    }
}

void VaultWolfManager::positionEnd() {
    VaultEWrapper::positionEnd();
}

void VaultWolfManager::orderStatus(OrderId orderId, const std::string& status, Decimal filled,
    Decimal remaining, double avgFillPrice, int permId, int parentId,
    double lastFillPrice, int clientId, const std::string& whyHeld, double mktCapPrice)
{
    VaultEWrapper::orderStatus(orderId, status, filled, remaining, avgFillPrice,
        permId, parentId, lastFillPrice, clientId, whyHeld, mktCapPrice);

    std::lock_guard<std::mutex> lock(m_orderMutex);
    auto it = m_orderMap.find(orderId);
    if (it != m_orderMap.end()) {
        auto& orderInfo = it->second;
        orderInfo->status = status;
        orderInfo->filled = static_cast<double>(filled);
        orderInfo->remaining = static_cast<double>(remaining);
        orderInfo->avgFillPrice = avgFillPrice;
        orderInfo->permId = permId;
        orderInfo->parentId = parentId;
        orderInfo->lastFillPrice = lastFillPrice;

        // Update timestamp
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        std::ostringstream oss;
        oss << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
        orderInfo->lastUpdateTime = oss.str();
    }
}

void VaultWolfManager::openOrder(OrderId orderId, const Contract& contract, const Order& order, const OrderState& orderState) {
    VaultEWrapper::openOrder(orderId, contract, order, orderState);

    std::lock_guard<std::mutex> lock(m_orderMutex);
    auto it = m_orderMap.find(orderId);
    if (it == m_orderMap.end()) {
        // Create new order info
        auto orderInfo = std::make_shared<OrderInfo>();
        orderInfo->orderId = orderId;
        orderInfo->symbol = contract.symbol;
        orderInfo->secType = contract.secType;
        orderInfo->exchange = contract.exchange;
        orderInfo->currency = contract.currency;
        orderInfo->action = order.action;
        orderInfo->orderType = order.orderType;
        orderInfo->totalQuantity = static_cast<double>(order.totalQuantity);
        orderInfo->lmtPrice = order.lmtPrice;
        orderInfo->auxPrice = order.auxPrice;
        orderInfo->status = orderState.status;

        if (contract.secType == "OPT") {
            orderInfo->right = contract.right;
            orderInfo->strike = contract.strike;
            orderInfo->expiry = contract.lastTradeDateOrContractMonth;
        }

        m_orderMap[orderId] = orderInfo;
    } else {
        // Update existing order info
        auto& orderInfo = it->second;
        orderInfo->status = orderState.status;
    }
}

void VaultWolfManager::openOrderEnd() {
    VaultEWrapper::openOrderEnd();
}

void VaultWolfManager::nextValidId(OrderId orderId) {
    VaultEWrapper::nextValidId(orderId);
    m_nextOrderId.store(orderId);
}

void VaultWolfManager::managedAccounts(const std::string& accountsList) {
    VaultEWrapper::managedAccounts(accountsList);

    m_managedAccounts.clear();
    std::istringstream iss(accountsList);
    std::string account;
    while (std::getline(iss, account, ',')) {
        m_managedAccounts.push_back(account);
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

Contract VaultWolfManager::createContract(const ContractSpec& spec) {
    Contract contract;
    contract.symbol = spec.symbol;
    contract.secType = spec.secType;
    contract.currency = spec.currency;
    contract.exchange = spec.exchange;

    if (spec.secType == "OPT") {
        contract.right = spec.right;
        contract.strike = spec.strike;
        contract.lastTradeDateOrContractMonth = spec.expiry;
    }
    else if (spec.secType == "FUT") {
        contract.lastTradeDateOrContractMonth = spec.lastTradeDateOrContractMonth;
    }

    return contract;
}

std::string VaultWolfManager::contractKey(const std::string& symbol, const std::string& secType) {
    return symbol + "_" + secType;
}
