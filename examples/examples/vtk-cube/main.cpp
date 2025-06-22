// Minimal VTK program to render a cube
#include <vtkActor.h>
#include <vtkCubeSource.h>
#include <vtkPolyDataMapper.h>
#include <vtkRenderWindow.h>
#include <vtkRenderWindowInteractor.h>
#include <vtkRenderer.h>
#include <vtkWindowToImageFilter.h>
#include <vtkImageExport.h>
#include <vtkSmartPointer.h>
#include <vtkCallbackCommand.h>
#include <cstring>
#include <iostream>
#include <thread>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include "webrtc_c_api.h"
#include <uWebSockets/App.h>
#include <uWebSockets/WebSocket.h>
#include <nlohmann/json.hpp> // For JSON parsing (add to project if not present)
#include <ixwebsocket/IXWebSocket.h>

// Stub for YUV conversion (replace with real implementation as needed)
void rgb_to_yuv420p(const unsigned char* rgb, int width, int height, unsigned char* yuv) {
    // This is a stub. In production, use a proper RGB to YUV420p conversion.
    // For now, just zero the buffer.
    size_t yuv_size = width * height * 3 / 2;
    std::memset(yuv, 0, yuv_size);
}

struct WebRTCContext {
    webrtc_session_t* session = nullptr;
};

// Example input callback (stub)
void webrtc_input_callback(const void* data, int len, void* user_data) {
    // Handle input events from the client (mouse, keyboard, etc.)
    std::cout << "[WebRTC] Received input event of length " << len << std::endl;
}

void render_webrtc(WebRTCContext* ctx, int width, int height, const unsigned char* yuv_pixels, bool verbose, size_t frame_idx = 0) {
    if (ctx && ctx->session) {
        if (verbose) {
            auto now = std::chrono::system_clock::now();
            std::time_t now_c = std::chrono::system_clock::to_time_t(now);
            std::cout << "[WebRTC][Streaming] Frame " << frame_idx
                      << ", size: " << width << "x" << height
                      << ", timestamp: " << std::put_time(std::localtime(&now_c), "%F %T")
                      << std::endl;
        }
        webrtc_session_send_frame(ctx->session, width, height, yuv_pixels);
    }
}

void render(int width, int height, const unsigned char* yuv_pixels, bool native_output) {
    if (native_output) {
        // Native output handled in main loop
        return;
    } else {
        // Stub for WebRTC output
        std::cout << "[STUB] Sending frame over WebRTC: " << width << "x" << height << std::endl;
    }
}

// Signaling client logic using ixwebsocket
class SignalingClient {
public:
    SignalingClient(const std::string& url, std::function<void(const std::string&)> on_message)
        : url_(url), on_message_(on_message) {}

    void start() {
        ws_.setUrl(url_);
        ws_.setOnMessageCallback([this](const ix::WebSocketMessagePtr& msg) {
            if (msg->type == ix::WebSocketMessageType::Open) {
                std::cout << "[SignalingClient] WebSocket connection opened to: " << url_ << std::endl;
            } else if (msg->type == ix::WebSocketMessageType::Message && on_message_) {
                std::cout << "[SignalingClient] Message received: " << msg->str << std::endl; // Added log
                on_message_(msg->str);
            } else if (msg->type == ix::WebSocketMessageType::Error) {
                std::cerr << "[SignalingClient] WebSocket error: " << msg->errorInfo.reason << std::endl;
            } else if (msg->type == ix::WebSocketMessageType::Close) {
                std::cout << "[SignalingClient] WebSocket connection closed. Code: " << msg->closeInfo.code << " Reason: " << msg->closeInfo.reason << std::endl;
            }
        });
        ws_.start();
    }
    void stop() {
        ws_.stop();
    }
    void send(const std::string& msg) {
        ws_.send(msg);
    }
private:
    std::string url_;
    std::function<void(const std::string&)> on_message_;
    ix::WebSocket ws_;
};

// Global verbose flag for C-style callbacks
static bool verbose_global_for_signal_callback = false;

int main(int argc, char* argv[])
{
    bool native_output = false;
    bool webrtc_output = false;
    bool verbose = false;
    int width = 640, height = 480;
    std::string signalling_url = "ws://localhost:8888";
    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "--native") native_output = true;
        if (arg == "--webrtc") webrtc_output = true;
        if (arg == "--size" && i + 2 < argc) {
            width = std::stoi(argv[++i]);
            height = std::stoi(argv[++i]);
        }
        if (arg == "--signalling" && i + 1 < argc) {
            signalling_url = argv[++i];
        }
        if (arg == "--verbose") verbose = true;
    }
    if (!native_output && !webrtc_output) native_output = true; // Default
    verbose_global_for_signal_callback = verbose; // Set global verbose flag

    WebRTCContext webrtc_ctx;
    std::unique_ptr<SignalingClient> signalling_client;
    if (webrtc_output) {
        // Create WebRTC session (config can be null or a stub for now)
        webrtc_ctx.session = webrtc_session_create(nullptr, webrtc_input_callback, nullptr);
        // Print diagnostics (ICE credentials, selected candidate, etc.)
        if (verbose && webrtc_ctx.session) {
            char* diag_json = webrtc_session_get_diagnostics(webrtc_ctx.session);
            if (diag_json) {
                std::cout << "[WebRTC][Diagnostics] " << diag_json << std::endl;
                free(diag_json);
            } else {
                std::cout << "[WebRTC][Diagnostics] (unavailable)" << std::endl;
            }
        }
        // Set up signaling client
        signalling_client = std::make_unique<SignalingClient>(signalling_url, [&](const std::string& msg) {
            // This is the on_message_ callback passed to SignalingClient
            // The [SignalingClient] Message received: log is now inside IXWebSocket's callback
            if (verbose) std::cout << "[WebRTC App] Processing message from SignalingClient: " << msg << std::endl;
            // Parse JSON and dispatch to WebRTC session
            try {
                auto j = nlohmann::json::parse(msg);
                std::string type = j.value("type", ""); // Get type, default to empty string if not found

                if (type == "Offer" || type == "Answer") { // Match "Offer" and "Answer" from Rust client
                    if (j.contains("data") && j["data"].contains("sdp")) {
                        std::string sdp = j["data"]["sdp"].get<std::string>();
                        // Construct the JSON string expected by webrtc_session_set_remote_description
                        // It typically expects an object like: { "type": "offer", "sdp": "..." }
                        // The C API might be parsing this directly.
                        // We need to ensure the C API gets the correct format.
                        // Let's try passing a reconstructed simple JSON if the C API expects that,
                        // or check if the C API can handle just the SDP string directly (less likely for set_remote_description).
                        // For now, let's assume the C API's webrtc_session_set_remote_description wants a JSON string
                        // that looks like { "type": "offer/answer", "sdp": "..." }
                        nlohmann::json sdp_payload;
                        sdp_payload["type"] = type == "Offer" ? "offer" : "answer"; // C API might expect lowercase
                        sdp_payload["sdp"] = sdp;
                        std::string sdp_payload_str = sdp_payload.dump();
                        if (verbose) std::cout << "[WebRTC App] Parsed SDP: " << sdp << std::endl;
                        if (verbose) std::cout << "[WebRTC App] Passing to C API (set_remote_description): " << sdp_payload_str << std::endl;
                        webrtc_session_set_remote_description(webrtc_ctx.session, sdp_payload_str.c_str());
                    } else {
                        std::cerr << "[Signaling] Malformed Offer/Answer: missing data.sdp field: " << msg << std::endl;
                    }
                } else if (type == "IceCandidate") { // Match "IceCandidate" from Rust client
                    if (j.contains("data") && j["data"].contains("candidate") && j["data"].contains("sdp_mid") && j["data"].contains("sdp_mline_index")) {
                        // Reconstruct the JSON string expected by webrtc_session_add_ice_candidate
                        // e.g., { "candidate": "...", "sdpMid": "...", "sdpMLineIndex": ... }
                        nlohmann::json ice_payload;
                        ice_payload["candidate"] = j["data"]["candidate"].get<std::string>();
                        ice_payload["sdpMid"] = j["data"]["sdp_mid"].get<std::string>(); // Ensure key case matches C API expectation
                        ice_payload["sdpMLineIndex"] = j["data"]["sdp_mline_index"].get<unsigned int>(); // Ensure key case
                        std::string ice_payload_str = ice_payload.dump();
                        if (verbose) std::cout << "[WebRTC App] Parsed ICE Candidate: " << ice_payload_str << std::endl;
                        if (verbose) std::cout << "[WebRTC App] Passing to C API (add_ice_candidate): " << ice_payload_str << std::endl;
                        webrtc_session_add_ice_candidate(webrtc_ctx.session, ice_payload_str.c_str());
                    } else {
                        std::cerr << "[Signaling] Malformed IceCandidate: missing fields: " << msg << std::endl;
                    }
                }
            } catch (const nlohmann::json::parse_error& e) {
                std::cerr << "[Signaling] Failed to parse JSON message: " << msg << " Error: " << e.what() << std::endl;
            } catch (const std::exception& e) {
                std::cerr << "[Signaling] Error processing message: " << msg << " Error: " << e.what() << std::endl;
            }
        });
        // Register callback to send local signaling messages to the browser
        webrtc_session_set_signal_callback(
            webrtc_ctx.session,
            [](const char* flat_msg_cstr, void* user_data) {
                if (user_data && flat_msg_cstr) {
                    std::string flat_msg_str(flat_msg_cstr);
                    if (verbose_global_for_signal_callback) {
                        std::cout << "[WebRTC App] C API generated flat message: " << flat_msg_str << std::endl;
                    }

                    try {
                        auto j_flat = nlohmann::json::parse(flat_msg_str);
                        std::string type = j_flat.value("type", ""); 
                        
                        if (verbose_global_for_signal_callback) {
                            std::cout << "[DEBUG C++ CB] Parsed type: \'" << type << "\'" << std::endl;
                            std::cout << "[DEBUG C++ CB] j_flat.dump(2): " << j_flat.dump(2) << std::endl;
                            std::cout << "[DEBUG C++ CB] j_flat.contains(\"candidate\"): " << (j_flat.contains("candidate") ? "true" : "false") << std::endl;
                            std::cout << "[DEBUG C++ CB] j_flat.contains(\"sdpMid\"): " << (j_flat.contains("sdpMid") ? "true" : "false") << std::endl;
                            std::cout << "[DEBUG C++ CB] j_flat.contains(\"sdpMLineIndex\"): " << (j_flat.contains("sdpMLineIndex") ? "true" : "false") << std::endl;
                        }
                        
                        nlohmann::json j_nested_outer;
                        nlohmann::json j_nested_data;
                        bool message_handled = false;

                        if (type == "answer") {
                            j_nested_outer["type"] = "Answer"; // Match Rust client's expected case
                            j_nested_data["sdp"] = j_flat.value("sdp", "");
                            j_nested_outer["data"] = j_nested_data;
                            message_handled = true;
                        } else if (type == "offer") {
                            // This case should ideally not happen if server is only answering.
                            // But if C API could generate an offer (e.g. for renegotiation)
                            j_nested_outer["type"] = "Offer";
                            j_nested_data["sdp"] = j_flat.value("sdp", "");
                            j_nested_outer["data"] = j_nested_data;
                            message_handled = true;
                        } else if (j_flat.contains("candidate") && j_flat.contains("sdpMid") && j_flat.contains("sdpMLineIndex")) {
                            // This is an ICE candidate from the C API (which doesn't set a "type" field for candidates)
                            // Let's ensure 'type' is empty or not 'answer'/'offer' to be more specific,
                            // though checking for candidate fields is quite robust.
                            if (type.empty() || (type != "answer" && type != "offer")) {
                                j_nested_outer["type"] = "IceCandidate"; // Match Rust client's expected case
                                j_nested_data["candidate"] = j_flat.value("candidate", "");
                                // sdpMid from C API is often empty or "0", ensure it's string.
                                j_nested_data["sdp_mid"] = j_flat.value("sdpMid", ""); 
                                j_nested_data["sdp_mline_index"] = j_flat.value("sdpMLineIndex", 0);
                                // usernameFragment is often null, Rust side expects it to be omitted or string.
                                // The current Rust SignalMessage for IceCandidate doesn't include usernameFragment, so we omit it.
                                j_nested_outer["data"] = j_nested_data;
                                message_handled = true;
                            }
                        }
                        
                        if (message_handled) {
                            std::string nested_msg_str = j_nested_outer.dump();
                            if (verbose_global_for_signal_callback) {
                                std::cout << "[WebRTC App] Sending nested message: " << nested_msg_str << std::endl;
                            }
                            static_cast<SignalingClient*>(user_data)->send(nested_msg_str);
                        } else {
                            // If type is unknown or not one we want to nest, send as-is or log error
                            std::cerr << "[WebRTC App] Unknown message type or structure from C API for nesting: " << flat_msg_str << std::endl;
                            static_cast<SignalingClient*>(user_data)->send(flat_msg_str); // Send original flat message
                        }

                    } catch (const nlohmann::json::parse_error& e) {
                        std::cerr << "[WebRTC App] Failed to parse flat JSON from C API: " << flat_msg_str << " Error: " << e.what() << std::endl;
                        static_cast<SignalingClient*>(user_data)->send(flat_msg_str); // Send original on error
                    } catch (const std::exception& e) {
                        std::cerr << "[WebRTC App] Error processing message from C API for nesting: " << flat_msg_str << " Error: " << e.what() << std::endl;
                        static_cast<SignalingClient*>(user_data)->send(flat_msg_str); // Send original on error
                    }
                }
            },
            signalling_client.get()
        );
        if (verbose) std::cout << "[Signaling] Connecting to " << signalling_url << std::endl;
        signalling_client->start();
    }

    // Create a cube
    vtkNew<vtkCubeSource> cubeSource;
    cubeSource->SetXLength(1.0);
    cubeSource->SetYLength(1.0);
    cubeSource->SetZLength(1.0);

    // Create a mapper
    vtkNew<vtkPolyDataMapper> mapper;
    mapper->SetInputConnection(cubeSource->GetOutputPort());

    // Create an actor
    vtkNew<vtkActor> actor;
    actor->SetMapper(mapper);

    // Create a renderer, render window, and interactor
    vtkNew<vtkRenderer> renderer;
    vtkNew<vtkRenderWindow> renderWindow;
    renderWindow->AddRenderer(renderer);
    renderWindow->SetSize(width, height);

    // Add the actor to the scene
    renderer->AddActor(actor);
    renderer->SetBackground(0.1, 0.2, 0.4); // Background color

    std::atomic<bool> running{true};
    std::atomic<bool> scene_dirty{true}; // Start dirty to send first frame
    std::mutex dirty_mutex;
    std::condition_variable dirty_cv;
    std::thread webrtc_thread;
    if (webrtc_output) {
        webrtc_thread = std::thread([=, &running, &scene_dirty, &dirty_mutex, &dirty_cv, &webrtc_ctx]() {
            // Create a separate VTK pipeline for the webrtc thread
            vtkNew<vtkCubeSource> cubeSourceW;
            cubeSourceW->SetXLength(1.0);
            cubeSourceW->SetYLength(1.0);
            cubeSourceW->SetZLength(1.0);
            vtkNew<vtkPolyDataMapper> mapperW;
            mapperW->SetInputConnection(cubeSourceW->GetOutputPort());
            vtkNew<vtkActor> actorW;
            actorW->SetMapper(mapperW);
            vtkNew<vtkRenderer> rendererW;
            vtkNew<vtkRenderWindow> offscreenRenderWindow;
            offscreenRenderWindow->AddRenderer(rendererW);
            offscreenRenderWindow->SetSize(width, height);
            offscreenRenderWindow->OffScreenRenderingOn();
            rendererW->AddActor(actorW);
            rendererW->SetBackground(0.1, 0.2, 0.4);
            size_t frame_idx = 0;
            while (running) {
                if (native_output) { // Only use condition variable if native output is also active
                    std::unique_lock<std::mutex> lock(dirty_mutex);
                    dirty_cv.wait(lock, [&]() { return scene_dirty || !running; });
                    if (!running) break;
                    scene_dirty = false;
                    lock.unlock();
                } else { // If not native_output, we are in WebRTC-only mode, stream continuously
                    // Ensure we don't spin too fast if rendering is very quick,
                    // though render_webrtc itself has a sleep.
                    // We might also want to set scene_dirty = true here if other logic depends on it,
                    // but for simple continuous streaming, just proceeding to render is fine.
                }

                offscreenRenderWindow->Render();
                vtkNew<vtkWindowToImageFilter> windowToImageFilter;
                windowToImageFilter->SetInput(offscreenRenderWindow);
                windowToImageFilter->Update();
                vtkImageData* image = windowToImageFilter->GetOutput();
                int dims[3];
                image->GetDimensions(dims);
                int num_pixels = dims[0] * dims[1];
                std::vector<unsigned char> rgb(num_pixels * 3);
                vtkNew<vtkImageExport> exporter;
                exporter->SetInputData(image);
                exporter->ImageLowerLeftOn();
                exporter->Update();
                exporter->Export(rgb.data());
                std::vector<unsigned char> yuv(num_pixels * 3 / 2);
                rgb_to_yuv420p(rgb.data(), dims[0], dims[1], yuv.data());
                render_webrtc(&webrtc_ctx, dims[0], dims[1], yuv.data(), verbose, frame_idx++);
                std::this_thread::sleep_for(std::chrono::milliseconds(33)); // ~30 FPS
            }
        });
    }

    if (native_output && !webrtc_output) {
        // Native-only mode: interactive VTK window
        // Attach observers to mark scene as dirty on interaction
        struct DirtyData {
            std::atomic<bool>* scene_dirty;
            std::condition_variable* dirty_cv;
        } dirtyData{&scene_dirty, &dirty_cv};
        auto mark_dirty_cb = [](vtkObject*, unsigned long, void* clientData, void*) {
            DirtyData* data = static_cast<DirtyData*>(clientData);
            *(data->scene_dirty) = true;
            data->dirty_cv->notify_one();
        };
        vtkSmartPointer<vtkCallbackCommand> callback = vtkSmartPointer<vtkCallbackCommand>::New();
        callback->SetCallback(mark_dirty_cb);
        callback->SetClientData(&dirtyData);
        renderWindow->AddObserver(vtkCommand::ModifiedEvent, callback);
        renderer->AddObserver(vtkCommand::ModifiedEvent, callback);
        renderWindow->AddObserver(vtkCommand::WindowResizeEvent, callback);
        renderWindow->AddObserver(vtkCommand::RenderEvent, callback);
        // Add more events as needed (e.g., mouse, keyboard)
        vtkNew<vtkRenderWindowInteractor> renderWindowInteractor;
        renderWindowInteractor->SetRenderWindow(renderWindow);
        renderWindow->SetWindowName("VTK Cube Example");
        renderWindow->Render();
        renderWindowInteractor->Start();
        running = false;
        dirty_cv.notify_one();
    } else if (webrtc_output) {
        // WebRTC mode (with or without native): keep main thread alive for signaling
        if (verbose) std::cout << "WebRTC mode active, waiting for signaling..." << std::endl;
        while (running) {
            std::this_thread::sleep_for(std::chrono::seconds(1));
        }
        dirty_cv.notify_one();
    }

    if (webrtc_thread.joinable()) {
        running = false;
        dirty_cv.notify_one();
        webrtc_thread.join();
    }

    if (webrtc_output && signalling_client) {
        signalling_client->stop();
    }

    if (webrtc_output && webrtc_ctx.session) {
        webrtc_session_destroy(webrtc_ctx.session);
    }

    return 0;
}
