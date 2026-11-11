FROM docker.io/library/ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential ca-certificates clang cmake pkg-config perl python3 \
    libssl-dev libdbus-1-dev libgtk-3-dev libxdo-dev libxkbcommon-dev \
    libwayland-dev libudev-dev dpkg-dev file binutils desktop-file-utils \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build/source
