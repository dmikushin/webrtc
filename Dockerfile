# Stage 1: Build CachyOS v3 rootfs
FROM cachyos/cachyos:latest as rootfs

COPY pacman-v3.conf /etc/pacman.conf
RUN pacman -Syu --noconfirm && \
    rm -rf /var/lib/pacman/sync/* && \
    find /var/cache/pacman/ -type f -delete

# Stage 2: Build environment for webrtc
FROM scratch

LABEL org.opencontainers.image.description="CachyOS - Arch-based distribution offering an easy installation, several customizations, and unique performance optimization. - v3 optimized Packages"

COPY --from=rootfs / /

# Install build dependencies and Rust
RUN pacman-key --init && \
    pacman-key --populate archlinux && \
    pacman -Syu --noconfirm \
    base-devel \
    pkgconf \
    openssl \
    libvpx \
    git \
    cmake \
    clang \
    python \
    wget \
    curl \
    ca-certificates \
    && pacman -Scc --noconfirm

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable && \
    echo 'source $HOME/.cargo/env' >> /etc/profile
ENV PATH="/root/.cargo/bin:$PATH"

WORKDIR /workspace

CMD ["/usr/bin/bash"]
