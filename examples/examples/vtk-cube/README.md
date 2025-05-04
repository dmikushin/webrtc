# VTK Cube WebRTC Example

This example demonstrates remote control and streaming of a VTK-rendered 3D scene (a cube) from a native server to a web browser client using WebRTC. It includes a C++ VTK server, a C++ WebSocket signaling server, and a browser-based WebRTC client.

## Components

- **vtk-cube**: C++ server rendering a VTK scene, streaming video via WebRTC, and accepting remote input.
- **signalling-server**: Minimal C++ WebSocket relay for WebRTC signaling.
- **vtk-cube-client**: HTML/JavaScript WebRTC client for video display and remote control.

---

## Prerequisites

- C++ compiler, CMake, VTK, uWebSockets, usockets, OpenSSL, zlib, pthread
- Rust toolchain (for building the webrtc shared library)
- Modern web browser (for the client)

---

## Build Instructions

### 1. Build the Rust WebRTC Shared Library

From the project root:
```sh
# Build the Rust webrtc shared library (libwebrtc.so)
docker-compose up --build
# or, if you have Rust installed:
cargo build --release -p webrtc
```

### 2. Build the Signaling Server

```sh
cd examples/signalling-server
mkdir -p build && cd build
cmake ..
make
```

### 3. Build the VTK Cube Example

```sh
cd ../../examples/vtk-cube
mkdir -p build && cd build
cmake ..
make
```

---

## Usage Instructions

### 1. Start the Signaling Server

```sh
cd examples/signalling-server/build
./signalling-server --verbose
```

### 2. Start the VTK Cube Server

```sh
cd ../../examples/vtk-cube/build
# For both native window and WebRTC streaming (default signaling server):
./vtk_cube --native --webrtc
# For headless WebRTC streaming only:
./vtk_cube --webrtc
# To specify a custom signaling server URL:
./vtk_cube --webrtc --signalling ws://your-signalling-server:8080
```

- By default, the signaling server URL is `ws://localhost:8080`.
- Use the `--signalling` option to specify a different signaling server address if needed.

### 3. Open the WebRTC Client in Your Browser

Open `examples/vtk-cube-client/index.html` in a modern browser (Chrome, Firefox, etc.).

- The client will connect to the signaling server at `ws://localhost:8080`.
- You should see the VTK-rendered cube video stream.
- Mouse and keyboard events in the browser are sent to the server via WebRTC data channel.

---

## Troubleshooting

- Ensure `libwebrtc.so` is built and available in `target/release/`.
- The signaling server, VTK server, and browser client must all be able to connect to each other (check firewalls and network settings).
- Use `--verbose` on the signaling server for detailed logs.
- If you change ports or addresses, update them in the HTML client and server configs.

---

## Customization

- To change the rendered scene, modify the VTK pipeline in `main.cpp`.
- To extend input handling, update the JavaScript client and the server's input callback.

---

## Credits

- VTK: https://vtk.org/
- uWebSockets: https://github.com/uNetworking/uWebSockets
- webrtc.rs: https://github.com/webrtc-rs/webrtc
