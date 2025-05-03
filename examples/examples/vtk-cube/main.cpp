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

void render_webrtc(WebRTCContext* ctx, int width, int height, const unsigned char* yuv_pixels) {
    if (ctx && ctx->session) {
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

int main(int argc, char* argv[])
{
    bool native_output = false;
    bool webrtc_output = false;
    int width = 640, height = 480;
    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg == "--native") native_output = true;
        if (arg == "--webrtc") webrtc_output = true;
        if (arg == "--size" && i + 2 < argc) {
            width = std::stoi(argv[++i]);
            height = std::stoi(argv[++i]);
        }
    }
    if (!native_output && !webrtc_output) native_output = true; // Default

    WebRTCContext webrtc_ctx;
    if (webrtc_output) {
        // Create WebRTC session (config can be null or a stub for now)
        webrtc_ctx.session = webrtc_session_create(nullptr, webrtc_input_callback, nullptr);
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
                render_webrtc(&webrtc_ctx, dims[0], dims[1], yuv.data());
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

    if (webrtc_output && webrtc_ctx.session) {
        webrtc_session_destroy(webrtc_ctx.session);
    }

    return 0;
}
