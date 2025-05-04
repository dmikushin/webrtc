# Use the official Rust image as the base
FROM rust:1.80

# Install build tools and pkg-config for native dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev libvpx-dev && \
    rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /workspace

# Default command
CMD ["bash"]
