//
// VaultWolf Main Server
// Web-based trading API server for Interactive Brokers
// Author: VaultWolf Team
// Date: 2025-11-21
//

#include <iostream>
#include <csignal>
#include <atomic>
#include <thread>
#include <chrono>

#include "manager/vault_manager.h"
#include "web/web_server.h"

// Global flag for graceful shutdown
std::atomic<bool> g_running(true);

// Signal handler for graceful shutdown
void signalHandler(int signal) {
    std::cout << "\nReceived signal " << signal << ", shutting down gracefully..." << std::endl;
    g_running.store(false);
}

void printUsage(const char* programName) {
    std::cout << "Usage: " << programName << " [options]\n\n";
    std::cout << "Options:\n";
    std::cout << "  --ib-host <host>        IB TWS/Gateway host (default: 127.0.0.1)\n";
    std::cout << "  --ib-port <port>        IB TWS/Gateway port (default: 4002)\n";
    std::cout << "  --ib-client-id <id>     IB client ID (default: 0)\n";
    std::cout << "  --web-port <port>       Web server port (default: 5000)\n";
    std::cout << "  --help                  Show this help message\n\n";
    std::cout << "Examples:\n";
    std::cout << "  " << programName << "\n";
    std::cout << "  " << programName << " --ib-port 7497 --web-port 8080\n";
    std::cout << "  " << programName << " --ib-host 192.168.1.100\n\n";
}

void printBanner() {
    std::cout << "========================================\n";
    std::cout << "   VaultWolf Trading API Server\n";
    std::cout << "   Interactive Brokers Integration\n";
    std::cout << "   Version 1.0.0\n";
    std::cout << "========================================\n\n";
}

int main(int argc, char* argv[]) {
    // Parse command line arguments
    std::string ibHost = "127.0.0.1";
    int ibPort = 4002;
    int ibClientId = 0;
    int webPort = 5000;

    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];

        if (arg == "--help") {
            printUsage(argv[0]);
            return 0;
        }
        else if (arg == "--ib-host" && i + 1 < argc) {
            ibHost = argv[++i];
        }
        else if (arg == "--ib-port" && i + 1 < argc) {
            ibPort = std::atoi(argv[++i]);
        }
        else if (arg == "--ib-client-id" && i + 1 < argc) {
            ibClientId = std::atoi(argv[++i]);
        }
        else if (arg == "--web-port" && i + 1 < argc) {
            webPort = std::atoi(argv[++i]);
        }
        else {
            std::cerr << "Unknown argument: " << arg << std::endl;
            printUsage(argv[0]);
            return 1;
        }
    }

    // Print banner
    printBanner();

    // Setup signal handlers
    std::signal(SIGINT, signalHandler);
    std::signal(SIGTERM, signalHandler);

    try {
        // Create manager
        std::cout << "Initializing VaultWolf Manager..." << std::endl;
        auto manager = std::make_shared<VaultWolf::VaultWolfManager>();

        // Connect to IB
        std::cout << "Connecting to IB TWS/Gateway at " << ibHost << ":" << ibPort
                  << " (client ID: " << ibClientId << ")..." << std::endl;

        if (!manager->connectToIB(ibHost.c_str(), ibPort, ibClientId)) {
            std::cerr << "Failed to connect to IB TWS/Gateway!" << std::endl;
            std::cerr << "Please ensure:\n";
            std::cerr << "  1. TWS or IB Gateway is running\n";
            std::cerr << "  2. API connections are enabled in TWS/Gateway settings\n";
            std::cerr << "  3. The host and port are correct\n";
            return 1;
        }

        std::cout << "Successfully connected to IB!" << std::endl;

        // Start message processing
        std::cout << "Starting message processing thread..." << std::endl;
        manager->startMessageProcessing();

        // Give it a moment to receive initial data
        std::this_thread::sleep_for(std::chrono::seconds(2));

        // Get managed accounts
        auto accounts = manager->getManagedAccounts();
        if (!accounts.empty()) {
            std::cout << "Managed accounts: ";
            for (size_t i = 0; i < accounts.size(); ++i) {
                std::cout << accounts[i];
                if (i < accounts.size() - 1) std::cout << ", ";
            }
            std::cout << std::endl;
        }

        // Create and start web server
        std::cout << "\nStarting Web Server on port " << webPort << "..." << std::endl;
        VaultWolf::WebServer webServer(manager, webPort);

        std::cout << "\n========================================\n";
        std::cout << "   VaultWolf Server is READY!\n";
        std::cout << "========================================\n\n";
        std::cout << "API Endpoints:\n";
        std::cout << "  Health Check:     GET  http://localhost:" << webPort << "/health\n\n";
        std::cout << "  Market Data:\n";
        std::cout << "    Real-time:      GET  http://localhost:" << webPort << "/api/market/realtime?symbol=SPY\n";
        std::cout << "    Historical:     GET  http://localhost:" << webPort << "/api/market/historical?symbol=SPY&duration=1%20D\n";
        std::cout << "    Subscribe:      POST http://localhost:" << webPort << "/api/market/subscribe\n";
        std::cout << "    Unsubscribe:    POST http://localhost:" << webPort << "/api/market/unsubscribe\n\n";
        std::cout << "  Account:\n";
        std::cout << "    Summary:        GET  http://localhost:" << webPort << "/api/account/summary\n";
        std::cout << "    Positions:      GET  http://localhost:" << webPort << "/api/account/positions\n\n";
        std::cout << "  Orders:\n";
        std::cout << "    Place Order:    POST http://localhost:" << webPort << "/api/order/place\n";
        std::cout << "    Cancel Order:   POST http://localhost:" << webPort << "/api/order/cancel\n";
        std::cout << "    Modify Order:   POST http://localhost:" << webPort << "/api/order/modify\n";
        std::cout << "    List Orders:    GET  http://localhost:" << webPort << "/api/order/list\n";
        std::cout << "    Get Order:      GET  http://localhost:" << webPort << "/api/order/:id\n\n";
        std::cout << "Press Ctrl+C to stop the server...\n\n";

        // Start web server (blocking)
        std::thread webThread([&webServer]() {
            webServer.start();
        });

        // Main loop - wait for shutdown signal
        while (g_running.load() && manager->isConnected()) {
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        }

        // Graceful shutdown
        std::cout << "\nShutting down..." << std::endl;

        // Stop web server
        std::cout << "Stopping web server..." << std::endl;
        webServer.stop();
        if (webThread.joinable()) {
            webThread.join();
        }

        // Stop message processing
        std::cout << "Stopping message processing..." << std::endl;
        manager->stopMessageProcessing();

        // Disconnect from IB
        std::cout << "Disconnecting from IB..." << std::endl;
        manager->disconnectFromIB();

        std::cout << "Shutdown complete. Goodbye!" << std::endl;

    } catch (const std::exception& e) {
        std::cerr << "Fatal error: " << e.what() << std::endl;
        return 1;
    }

    return 0;
}
