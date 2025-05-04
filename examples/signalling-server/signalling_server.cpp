// Minimal C++ WebSocket Signaling Server using uWebSockets
#include <uWebSockets/App.h>
#include <iostream>
#include <vector>
#include <algorithm>
#include <atomic>

struct PerSocketData {};

int main(int argc, char* argv[]) {
    std::atomic<bool> verbose{false};
    for (int i = 1; i < argc; ++i) {
        if (std::string(argv[i]) == "--verbose") verbose = true;
    }
    std::vector<uWS::WebSocket<false, true, PerSocketData>*> clients;

    uWS::App().ws<PerSocketData>("/*", {
        .open = [&clients, &verbose](auto* ws) {
            clients.push_back(ws);
            std::cout << "Client connected. Total: " << clients.size() << std::endl;
        },
        .message = [&clients, &verbose](auto* ws, std::string_view msg, uWS::OpCode) {
            if (verbose) {
                std::cout << "Received message: " << msg << std::endl;
            }
            for (auto* client : clients) {
                if (client != ws) {
                    client->send(msg, uWS::OpCode::TEXT);
                    if (verbose) {
                        std::cout << "Relayed message to client." << std::endl;
                    }
                }
            }
        },
        .close = [&clients, &verbose](auto* ws, int /*code*/, std::string_view /*msg*/) {
            clients.erase(std::remove(clients.begin(), clients.end(), ws), clients.end());
            std::cout << "Client disconnected. Total: " << clients.size() << std::endl;
        }
    }).listen(8080, [](auto* token) {
        if (token) {
            std::cout << "Signaling server listening on ws://localhost:8080" << std::endl;
        }
    }).run();

    return 0;
}
