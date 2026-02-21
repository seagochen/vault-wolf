//
// VaultWolf Web Server
// REST API server using cpp-httplib
// Author: VaultWolf Team
// Date: 2025-11-21
//

#ifndef VAULTWOLF_WEB_SERVER_H
#define VAULTWOLF_WEB_SERVER_H

#include "manager/vault_manager.h"
#include "httplib/httplib.h"
#include <memory>
#include <string>

namespace VaultWolf {

/**
 * @brief WebServer - REST API server for VaultWolf
 *
 * Provides HTTP endpoints for:
 * - Market data (real-time and historical)
 * - Account information
 * - Order management
 * - Position queries
 */
class WebServer {
public:
    /**
     * @brief Constructor
     * @param manager Pointer to VaultWolfManager instance
     * @param port HTTP server port (default: 5000)
     */
    WebServer(std::shared_ptr<VaultWolfManager> manager, int port = 5000);

    ~WebServer();

    /**
     * @brief Start the web server (blocking)
     */
    void start();

    /**
     * @brief Start the web server in a background thread (non-blocking)
     */
    void startAsync();

    /**
     * @brief Stop the web server
     */
    void stop();

    /**
     * @brief Check if server is running
     */
    bool isRunning() const;

private:
    // ========================================================================
    // Route Handlers
    // ========================================================================

    // Health check
    void handleHealthCheck(const httplib::Request& req, httplib::Response& res);

    // Market Data
    void handleMarketDataRequest(const httplib::Request& req, httplib::Response& res);
    void handleHistoricalDataRequest(const httplib::Request& req, httplib::Response& res);
    void handleCancelMarketData(const httplib::Request& req, httplib::Response& res);

    // Account
    void handleAccountSummary(const httplib::Request& req, httplib::Response& res);
    void handlePositions(const httplib::Request& req, httplib::Response& res);

    // Orders
    void handlePlaceOrder(const httplib::Request& req, httplib::Response& res);
    void handleCancelOrder(const httplib::Request& req, httplib::Response& res);
    void handleModifyOrder(const httplib::Request& req, httplib::Response& res);
    void handleGetOrders(const httplib::Request& req, httplib::Response& res);
    void handleGetOrder(const httplib::Request& req, httplib::Response& res);

    // ========================================================================
    // Helper Functions
    // ========================================================================

    /**
     * @brief Setup all API routes
     */
    void setupRoutes();

    /**
     * @brief Parse contract specification from request parameters
     */
    ContractSpec parseContractSpec(const httplib::Request& req);

    /**
     * @brief Send JSON response
     */
    void sendJSON(httplib::Response& res, const std::string& json, int statusCode = 200);

    /**
     * @brief Send error response
     */
    void sendError(httplib::Response& res, const std::string& message, int statusCode = 400);

    /**
     * @brief Send success response
     */
    void sendSuccess(httplib::Response& res, const std::string& message, const std::string& data = "");

    // ========================================================================
    // Member Variables
    // ========================================================================

    std::shared_ptr<VaultWolfManager> m_manager;
    std::unique_ptr<httplib::Server> m_server;
    std::unique_ptr<std::thread> m_serverThread;
    int m_port;
    std::atomic<bool> m_isRunning;
};

} // namespace VaultWolf

#endif // VAULTWOLF_WEB_SERVER_H
