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
            if (msg->type == ix::WebSocketMessageType::Message && on_message_) {
                on_message_(msg->str);
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

int main(int argc, char* argv[])
{
    bool native_output = false;
    bool webrtc_output = false;
    bool verbose = false;
    int width = 640, height = 480;
    std::string signalling_url = "ws://localhost:8080";
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
            if (verbose) std::cout << "[Signaling] Received: " << msg << std::endl;
            // Parse JSON and dispatch to WebRTC session
            try {
                auto j = nlohmann::json::parse(msg);
                if (j["type"] == "offer" || j["type"] == "answer") {
                    webrtc_session_set_remote_description(webrtc_ctx.session, msg.c_str());
                } else if (j["type"] == "candidate") {
                    webrtc_session_add_ice_candidate(webrtc_ctx.session, msg.c_str());
                }
            } catch (...) {
                std::cerr << "[Signaling] Failed to parse message: " << msg << std::endl;
            }
        });
        // Register callback to send local signaling messages to the browser
        webrtc_session_set_signal_callback(
            webrtc_ctx.session,
            [](const char* msg, void* user_data) {
                if (user_data && msg) {
                    static_cast<SignalingClient*>(user_data)->send(msg);
                    // Print outgoing message if verbose
                    std::cout << "[Signaling] Sent: " << msg << std::endl;
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
                std::unique_lock<std::mutex> lock(dirty_mutex);
                dirty_cv.wait(lock, [&]() { return scene_dirty || !running; });
                if (!running) break;
                scene_dirty = false;
                lock.unlock();
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

    if (native_output) {
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
        // If only webrtc output, keep main thread alive until interrupted
        std::cout << "Press Ctrl+C to exit..." << std::endl;
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
