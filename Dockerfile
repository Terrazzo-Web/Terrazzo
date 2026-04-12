FROM ubuntu:24.04

# Install necessary packages and tools
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && \
    apt-get install -y \
        bash \
        ca-certificates \
        curl \
        git \
        unzip \
        zip \
        build-essential \
        openjdk-21-jdk \
        libssl-dev \
        pkg-config \
        protobuf-compiler \
        npm && \
    rm -rf /var/lib/apt/lists/*

# Install Rust
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:${PATH}
RUN curl https://sh.rustup.rs --proto '=https' --tlsv1.2 -sSf | \
    sh -s -- -y --profile minimal --default-toolchain stable && \
    rustup target add wasm32-unknown-unknown

# Install Bazel
RUN curl -fsSL https://github.com/bazelbuild/bazelisk/releases/latest/download/bazelisk-linux-amd64 \
        -o /usr/local/bin/bazel && \
    chmod +x /usr/local/bin/bazel

# Make sure playwright dependencies are installed
COPY package.json package-lock.json /root/
RUN cd /root \
    npm ci && \
    npx playwright install --with-deps chromium && \
    rm -rf /var/lib/apt/lists/*

CMD ["tail", "-f", "/dev/null"]
