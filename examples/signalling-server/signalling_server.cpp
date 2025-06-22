// Minimal C++ WebSocket Signaling Server using uWebSockets
#include <uWebSockets/App.h>
#include <iostream>
#include <vector>
#include <algorithm>
#include <atomic>
#include <stdexcept>
#include <mutex>
#include <ctime>
#include <iomanip>
#include <sstream>

struct PerSocketData {};

class Logger {
public:
    static void error(const std::string& message) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::cerr << "[ERROR] " << getCurrentTime() << " " << message << std::endl;
    }
    
    static void info(const std::string& message) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::cout << "[INFO] " << getCurrentTime() << " " << message << std::endl;
    }
    
    static void warn(const std::string& message) {
        std::lock_guard<std::mutex> lock(mutex_);
        std::cout << "[WARN] " << getCurrentTime() << " " << message << std::endl;
    }

private:
    static std::mutex mutex_;
    
    static std::string getCurrentTime() {
        auto now = std::time(nullptr);
        auto* tm = std::localtime(&now);
        std::ostringstream oss;
        oss << std::put_time(tm, "%Y-%m-%d %H:%M:%S");
        return oss.str();
    }
};

std::mutex Logger::mutex_;

class ClientManager {
public:
    void addClient(uWS::WebSocket<false, true, PerSocketData>* ws) {
        std::lock_guard<std::mutex> lock(mutex_);
        try {
            clients_.push_back(ws);
            Logger::info("Client connected. Total: " + std::to_string(clients_.size()));
        } catch (const std::exception& e) {
            Logger::error("Failed to add client: " + std::string(e.what()));
            throw;
        }
    }
    
    void removeClient(uWS::WebSocket<false, true, PerSocketData>* ws) {
        std::lock_guard<std::mutex> lock(mutex_);
        try {
            auto it = std::find(clients_.begin(), clients_.end(), ws);
            if (it != clients_.end()) {
                clients_.erase(it);
                Logger::info("Client disconnected. Total: " + std::to_string(clients_.size()));
            } else {
                Logger::warn("Attempted to remove non-existent client");
            }
        } catch (const std::exception& e) {
            Logger::error("Failed to remove client: " + std::string(e.what()));
        }
    }
    
    void broadcastMessage(uWS::WebSocket<false, true, PerSocketData>* sender, 
                         std::string_view message, bool verbose) {
        std::lock_guard<std::mutex> lock(mutex_);
        int sent_count = 0;
        int failed_count = 0;
        
        for (auto* client : clients_) {
            if (client != sender) {
                try {
                    client->send(message, uWS::OpCode::TEXT);
                    sent_count++;
                    if (verbose) {
                        Logger::info("Message relayed to client");
                    }
                } catch (const std::exception& e) {
                    failed_count++;
                    Logger::error("Failed to send message to client: " + std::string(e.what()));
                }
            }
        }
        
        if (failed_count > 0) {
            Logger::warn("Failed to send message to " + std::to_string(failed_count) + " clients");
        }
        
        if (verbose && sent_count > 0) {
            Logger::info("Message sent to " + std::to_string(sent_count) + " clients");
        }
    }
    
    size_t getClientCount() const {
        std::lock_guard<std::mutex> lock(mutex_);
        return clients_.size();
    }

private:
    mutable std::mutex mutex_;
    std::vector<uWS::WebSocket<false, true, PerSocketData>*> clients_;
};

int main(int argc, char* argv[]) {
    try {
        Logger::info("Starting signaling server...");
        
        std::atomic<bool> verbose{false};
        int port = 8080;
        
        for (int i = 1; i < argc; ++i) {
            try {
                std::string arg(argv[i]);
                if (arg == "--verbose") {
                    verbose = true;
                    Logger::info("Verbose mode enabled");
                } else if (arg == "--port" && i + 1 < argc) {
                    port = std::stoi(argv[++i]);
                    if (port <= 0 || port > 65535) {
                        throw std::invalid_argument("Port must be between 1 and 65535");
                    }
                    Logger::info("Using port: " + std::to_string(port));
                } else if (arg == "--help") {
                    std::cout << "Usage: " << argv[0] << " [OPTIONS]\n";
                    std::cout << "Options:\n";
                    std::cout << "  --verbose    Enable verbose logging\n";
                    std::cout << "  --port PORT  Set server port (default: 8080)\n";
                    std::cout << "  --help       Show this help message\n";
                    return 0;
                } else {
                    Logger::warn("Unknown argument: " + arg);
                }
            } catch (const std::exception& e) {
                Logger::error("Error parsing argument '" + std::string(argv[i]) + "': " + e.what());
                return 1;
            }
        }

        ClientManager clientManager;

        auto app = uWS::App().ws<PerSocketData>("/*", {
            .open = [&clientManager, &verbose](auto* ws) {
                try {
                    clientManager.addClient(ws);
                } catch (const std::exception& e) {
                    Logger::error("Failed to handle client connection: " + std::string(e.what()));
                }
            },
            .message = [&clientManager, &verbose](auto* ws, std::string_view msg, uWS::OpCode) {
                try {
                    if (verbose) {
                        Logger::info("Received message (" + std::to_string(msg.length()) + " bytes)");
                    }
                    
                    if (msg.empty()) {
                        Logger::warn("Received empty message, ignoring");
                        return;
                    }
                    
                    if (msg.length() > 65536) {
                        Logger::warn("Received oversized message (" + std::to_string(msg.length()) + " bytes), ignoring");
                        return;
                    }
                    
                    clientManager.broadcastMessage(ws, msg, verbose);
                } catch (const std::exception& e) {
                    Logger::error("Failed to handle message: " + std::string(e.what()));
                }
            },
            .close = [&clientManager, &verbose](auto* ws, int code, std::string_view msg) {
                try {
                    if (verbose) {
                        Logger::info("Client disconnecting with code: " + std::to_string(code));
                    }
                    clientManager.removeClient(ws);
                } catch (const std::exception& e) {
                    Logger::error("Failed to handle client disconnection: " + std::string(e.what()));
                }
            }
        });

        bool serverStarted = false;
        app.listen(port, [&serverStarted, port](auto* token) {
            if (token) {
                Logger::info("Signaling server listening on ws://localhost:" + std::to_string(port));
                serverStarted = true;
            } else {
                Logger::error("Failed to start server on port " + std::to_string(port));
            }
        });

        if (!serverStarted) {
            Logger::error("Server failed to start, check if port " + std::to_string(port) + " is available");
            return 1;
        }

        Logger::info("Server started successfully. Press Ctrl+C to stop.");
        app.run();

    } catch (const std::exception& e) {
        Logger::error("Fatal error: " + std::string(e.what()));
        return 1;
    } catch (...) {
        Logger::error("Unknown fatal error occurred");
        return 1;
    }

    Logger::info("Server shutting down gracefully");
    return 0;
}
