//
// VaultWolf Web Server Implementation
// Author: VaultWolf Team
// Date: 2025-11-21
//

#include "web/web_server.h"
#include "common/json_helper.h"
#include <iostream>
#include <chrono>
#include <iomanip>

using namespace VaultWolf;

// ============================================================================
// Constructor / Destructor
// ============================================================================

WebServer::WebServer(std::shared_ptr<VaultWolfManager> manager, int port)
    : m_manager(manager)
    , m_server(std::make_unique<httplib::Server>())
    , m_port(port)
    , m_isRunning(false)
{
    setupRoutes();
}

WebServer::~WebServer() {
    stop();
}

// ============================================================================
// Server Control
// ============================================================================

void WebServer::start() {
    std::cout << "Starting VaultWolf Web Server on port " << m_port << "..." << std::endl;
    m_isRunning.store(true);
    m_server->listen("0.0.0.0", m_port);
}

void WebServer::startAsync() {
    if (m_isRunning.load()) {
        return;  // Already running
    }

    m_serverThread = std::make_unique<std::thread>([this]() {
        this->start();
    });
}

void WebServer::stop() {
    if (!m_isRunning.load()) {
        return;
    }

    std::cout << "Stopping VaultWolf Web Server..." << std::endl;
    m_isRunning.store(false);
    m_server->stop();

    if (m_serverThread && m_serverThread->joinable()) {
        m_serverThread->join();
    }
}

bool WebServer::isRunning() const {
    return m_isRunning.load();
}

// ============================================================================
// Route Setup
// ============================================================================

void WebServer::setupRoutes() {
    // Health check
    m_server->Get("/health", [this](const httplib::Request& req, httplib::Response& res) {
        handleHealthCheck(req, res);
    });

    // Market Data Routes
    m_server->Get("/api/market/realtime", [this](const httplib::Request& req, httplib::Response& res) {
        handleMarketDataRequest(req, res);
    });

    m_server->Get("/api/market/historical", [this](const httplib::Request& req, httplib::Response& res) {
        handleHistoricalDataRequest(req, res);
    });

    m_server->Post("/api/market/subscribe", [this](const httplib::Request& req, httplib::Response& res) {
        handleMarketDataRequest(req, res);
    });

    m_server->Post("/api/market/unsubscribe", [this](const httplib::Request& req, httplib::Response& res) {
        handleCancelMarketData(req, res);
    });

    // Account Routes
    m_server->Get("/api/account/summary", [this](const httplib::Request& req, httplib::Response& res) {
        handleAccountSummary(req, res);
    });

    m_server->Get("/api/account/positions", [this](const httplib::Request& req, httplib::Response& res) {
        handlePositions(req, res);
    });

    // Order Routes
    m_server->Post("/api/order/place", [this](const httplib::Request& req, httplib::Response& res) {
        handlePlaceOrder(req, res);
    });

    m_server->Post("/api/order/cancel", [this](const httplib::Request& req, httplib::Response& res) {
        handleCancelOrder(req, res);
    });

    m_server->Post("/api/order/modify", [this](const httplib::Request& req, httplib::Response& res) {
        handleModifyOrder(req, res);
    });

    m_server->Get("/api/order/list", [this](const httplib::Request& req, httplib::Response& res) {
        handleGetOrders(req, res);
    });

    m_server->Get("/api/order/:id", [this](const httplib::Request& req, httplib::Response& res) {
        handleGetOrder(req, res);
    });

    std::cout << "API routes configured successfully" << std::endl;
}

// ============================================================================
// Route Handlers
// ============================================================================

void WebServer::handleHealthCheck(const httplib::Request& req, httplib::Response& res) {
    bool connected = m_manager->isConnected();
    std::ostringstream oss;
    oss << "{\n";
    oss << "  \"status\": \"" << (connected ? "healthy" : "disconnected") << "\",\n";
    oss << "  \"ibConnected\": " << (connected ? "true" : "false") << ",\n";
    oss << "  \"server\": \"VaultWolf API Server\",\n";
    oss << "  \"version\": \"1.0.0\"\n";
    oss << "}";
    sendJSON(res, oss.str());
}

void WebServer::handleMarketDataRequest(const httplib::Request& req, httplib::Response& res) {
    try {
        // Check if this is a GET request (query existing data) or POST (subscribe to new data)
        bool isSubscribe = (req.method == "POST");

        ContractSpec contract = parseContractSpec(req);

        if (contract.symbol.empty()) {
            sendError(res, "Missing required parameter: symbol");
            return;
        }

        if (isSubscribe) {
            // Subscribe to market data
            long reqId = m_manager->requestMarketData(contract);
            std::ostringstream oss;
            oss << "{\n";
            oss << "  \"reqId\": " << reqId << ",\n";
            oss << "  \"symbol\": \"" << contract.symbol << "\",\n";
            oss << "  \"secType\": \"" << contract.secType << "\"\n";
            oss << "}";
            sendSuccess(res, "Market data subscription created", oss.str());
        } else {
            // Get existing tick data
            auto tickData = m_manager->getTickData(contract.symbol, contract.secType);
            if (tickData) {
                std::string json = JSON::toJSON(*tickData);
                sendSuccess(res, "Market data retrieved", json);
            } else {
                sendError(res, "No market data found for symbol: " + contract.symbol, 404);
            }
        }
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

void WebServer::handleHistoricalDataRequest(const httplib::Request& req, httplib::Response& res) {
    try {
        ContractSpec contract = parseContractSpec(req);

        if (contract.symbol.empty()) {
            sendError(res, "Missing required parameter: symbol");
            return;
        }

        // Get parameters with defaults
        std::string endDateTime = req.has_param("end_date") ?
            req.get_param_value("end_date") : "";
        std::string duration = req.has_param("duration") ?
            req.get_param_value("duration") : "1 D";
        std::string barSize = req.has_param("bar_size") ?
            req.get_param_value("bar_size") : "1 hour";
        std::string whatToShow = req.has_param("what_to_show") ?
            req.get_param_value("what_to_show") : "TRADES";

        // If no end date specified, use current time
        if (endDateTime.empty()) {
            auto now = std::chrono::system_clock::now();
            auto time_t = std::chrono::system_clock::to_time_t(now);
            std::ostringstream oss;
            oss << std::put_time(std::localtime(&time_t), "%Y%m%d %H:%M:%S");
            endDateTime = oss.str();
        }

        // Request historical data
        long reqId = m_manager->requestHistoricalData(
            contract, endDateTime, duration, barSize, whatToShow
        );

        // Wait a moment for data to arrive (simple approach)
        std::this_thread::sleep_for(std::chrono::seconds(2));

        // Get historical data
        auto histData = m_manager->getHistoricalData(reqId);
        if (histData && !histData->bars.empty()) {
            std::string json = JSON::toJSON(*histData);
            sendSuccess(res, "Historical data retrieved", json);
        } else {
            // Return request ID for later querying
            std::ostringstream oss;
            oss << "{\n";
            oss << "  \"reqId\": " << reqId << ",\n";
            oss << "  \"status\": \"pending\",\n";
            oss << "  \"message\": \"Historical data request submitted. Data may not be available yet.\"\n";
            oss << "}";
            sendSuccess(res, "Historical data request submitted", oss.str());
        }
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

void WebServer::handleCancelMarketData(const httplib::Request& req, httplib::Response& res) {
    try {
        if (!req.has_param("req_id")) {
            sendError(res, "Missing required parameter: req_id");
            return;
        }

        long reqId = std::stol(req.get_param_value("req_id"));
        m_manager->cancelMarketData(reqId);

        sendSuccess(res, "Market data subscription cancelled");
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

void WebServer::handleAccountSummary(const httplib::Request& req, httplib::Response& res) {
    try {
        std::string account = req.has_param("account") ?
            req.get_param_value("account") : "";

        // Request account summary if needed
        m_manager->requestAccountSummary();
        std::this_thread::sleep_for(std::chrono::seconds(1));

        auto summary = m_manager->getAccountSummary(account);
        if (summary) {
            std::string json = JSON::toJSON(*summary);
            sendSuccess(res, "Account summary retrieved", json);
        } else {
            sendError(res, "No account summary available", 404);
        }
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

void WebServer::handlePositions(const httplib::Request& req, httplib::Response& res) {
    try {
        // Request positions
        m_manager->requestPositions();
        std::this_thread::sleep_for(std::chrono::seconds(1));

        std::vector<Position> positions;

        // Filter by parameters
        if (req.has_param("account")) {
            std::string account = req.get_param_value("account");
            positions = m_manager->getPositionsByAccount(account);
        }
        else if (req.has_param("symbol")) {
            std::string symbol = req.get_param_value("symbol");
            std::string secType = req.has_param("sec_type") ?
                req.get_param_value("sec_type") : "STK";
            positions = m_manager->getPositionsBySymbol(symbol, secType);
        }
        else {
            positions = m_manager->getAllPositions();
        }

        std::string json = JSON::toJSONArray(positions);
        sendSuccess(res, "Positions retrieved", json);
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

void WebServer::handlePlaceOrder(const httplib::Request& req, httplib::Response& res) {
    try {
        ContractSpec contract = parseContractSpec(req);

        if (contract.symbol.empty()) {
            sendError(res, "Missing required parameter: symbol");
            return;
        }

        if (!req.has_param("action")) {
            sendError(res, "Missing required parameter: action (BUY/SELL)");
            return;
        }

        if (!req.has_param("quantity")) {
            sendError(res, "Missing required parameter: quantity");
            return;
        }

        std::string action = req.get_param_value("action");
        double quantity = std::stod(req.get_param_value("quantity"));
        std::string orderType = req.has_param("order_type") ?
            req.get_param_value("order_type") : "MKT";

        long orderId = 0;

        if (orderType == "MKT") {
            orderId = m_manager->placeMarketOrder(contract, action, quantity);
        }
        else if (orderType == "LMT") {
            if (!req.has_param("limit_price")) {
                sendError(res, "Missing required parameter for limit order: limit_price");
                return;
            }
            double limitPrice = std::stod(req.get_param_value("limit_price"));
            orderId = m_manager->placeLimitOrder(contract, action, quantity, limitPrice);
        }
        else if (orderType == "STP") {
            if (!req.has_param("stop_price")) {
                sendError(res, "Missing required parameter for stop order: stop_price");
                return;
            }
            double stopPrice = std::stod(req.get_param_value("stop_price"));
            orderId = m_manager->placeStopOrder(contract, action, quantity, stopPrice);
        }
        else {
            sendError(res, "Invalid order type. Supported: MKT, LMT, STP");
            return;
        }

        std::ostringstream oss;
        oss << "{\n";
        oss << "  \"orderId\": " << orderId << ",\n";
        oss << "  \"symbol\": \"" << contract.symbol << "\",\n";
        oss << "  \"action\": \"" << action << "\",\n";
        oss << "  \"quantity\": " << quantity << ",\n";
        oss << "  \"orderType\": \"" << orderType << "\"\n";
        oss << "}";

        sendSuccess(res, "Order placed successfully", oss.str());
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

void WebServer::handleCancelOrder(const httplib::Request& req, httplib::Response& res) {
    try {
        if (!req.has_param("order_id")) {
            sendError(res, "Missing required parameter: order_id");
            return;
        }

        long orderId = std::stol(req.get_param_value("order_id"));
        m_manager->cancelOrder(orderId);

        sendSuccess(res, "Order cancellation requested");
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

void WebServer::handleModifyOrder(const httplib::Request& req, httplib::Response& res) {
    try {
        if (!req.has_param("order_id")) {
            sendError(res, "Missing required parameter: order_id");
            return;
        }

        long orderId = std::stol(req.get_param_value("order_id"));
        double quantity = req.has_param("quantity") ?
            std::stod(req.get_param_value("quantity")) : 0.0;
        double limitPrice = req.has_param("limit_price") ?
            std::stod(req.get_param_value("limit_price")) : 0.0;
        double stopPrice = req.has_param("stop_price") ?
            std::stod(req.get_param_value("stop_price")) : 0.0;

        m_manager->modifyOrder(orderId, quantity, limitPrice, stopPrice);

        sendSuccess(res, "Order modification requested");
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

void WebServer::handleGetOrders(const httplib::Request& req, httplib::Response& res) {
    try {
        std::vector<OrderInfo> orders;

        if (req.has_param("status")) {
            std::string status = req.get_param_value("status");
            orders = m_manager->getOrdersByStatus(status);
        }
        else if (req.has_param("symbol")) {
            std::string symbol = req.get_param_value("symbol");
            std::string secType = req.has_param("sec_type") ?
                req.get_param_value("sec_type") : "STK";
            orders = m_manager->getOrdersBySymbol(symbol, secType);
        }
        else {
            // Request open and completed orders first
            m_manager->requestOpenOrders();
            std::this_thread::sleep_for(std::chrono::milliseconds(500));

            orders = m_manager->getAllOrders();
        }

        std::string json = JSON::toJSONArray(orders);
        sendSuccess(res, "Orders retrieved", json);
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

void WebServer::handleGetOrder(const httplib::Request& req, httplib::Response& res) {
    try {
        if (req.path_params.count("id") == 0) {
            sendError(res, "Missing order ID in path");
            return;
        }

        long orderId = std::stol(req.path_params.at("id"));
        auto order = m_manager->getOrder(orderId);

        if (order) {
            std::string json = JSON::toJSON(*order);
            sendSuccess(res, "Order retrieved", json);
        } else {
            sendError(res, "Order not found", 404);
        }
    } catch (const std::exception& e) {
        sendError(res, std::string("Error: ") + e.what(), 500);
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

ContractSpec WebServer::parseContractSpec(const httplib::Request& req) {
    ContractSpec contract;

    contract.symbol = req.has_param("symbol") ? req.get_param_value("symbol") : "";
    contract.secType = req.has_param("sec_type") ? req.get_param_value("sec_type") : "STK";
    contract.currency = req.has_param("currency") ? req.get_param_value("currency") : "USD";
    contract.exchange = req.has_param("exchange") ? req.get_param_value("exchange") : "SMART";

    // For options
    if (contract.secType == "OPT") {
        contract.right = req.has_param("right") ? req.get_param_value("right") : "";
        contract.strike = req.has_param("strike") ? std::stod(req.get_param_value("strike")) : 0.0;
        contract.expiry = req.has_param("expiry") ? req.get_param_value("expiry") : "";
    }

    // For futures
    if (contract.secType == "FUT") {
        contract.lastTradeDateOrContractMonth = req.has_param("expiry") ?
            req.get_param_value("expiry") : "";
    }

    return contract;
}

void WebServer::sendJSON(httplib::Response& res, const std::string& json, int statusCode) {
    res.status = statusCode;
    res.set_content(json, "application/json");
}

void WebServer::sendError(httplib::Response& res, const std::string& message, int statusCode) {
    std::string json = JSON::errorResponse(message, statusCode);
    sendJSON(res, json, statusCode);
}

void WebServer::sendSuccess(httplib::Response& res, const std::string& message, const std::string& data) {
    std::string json = JSON::successResponse(message, data);
    sendJSON(res, json, 200);
}
