// Minimal C++ WebSocket Signaling Server using uWebSockets
#include <uWebSockets/App.h>
#include <iostream>
#include <vector>
#include <algorithm>

struct PerSocketData {};

int main() {
    std::vector<uWS::WebSocket<false, true, PerSocketData>*> clients;

    uWS::App().ws<PerSocketData>("/*", {
        .open = [&clients](auto* ws) {
            clients.push_back(ws);
            std::cout << "Client connected. Total: " << clients.size() << std::endl;
        },
        .message = [&clients](auto* ws, std::string_view msg, uWS::OpCode) {
            for (auto* client : clients) {
                if (client != ws) {
                    client->send(msg, uWS::OpCode::TEXT);
                }
            }
        },
        .close = [&clients](auto* ws, int /*code*/, std::string_view /*msg*/) {
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
