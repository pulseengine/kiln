# Kiln VxWorks Dev Container
# Persistent development environment with VxWorks SDK + Rust + QEMU
FROM --platform=linux/amd64 ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# Install dev tools + QEMU + expect
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates git build-essential pkg-config \
    qemu-system-x86 expect \
    sudo less vim nano \
    && rm -rf /var/lib/apt/lists/*

# Download VxWorks SDK (free under NCLA)
# https://forums.windriver.com/t/vxworks-software-development-kit-sdk/43
RUN mkdir -p /opt/wrsdk && \
    curl -fSL https://d13321s3lxgewa.cloudfront.net/wrsdk-vxworks7-qemu-1.16.1.tar.bz2 \
      -o /tmp/wrsdk.tar.bz2 && \
    tar xjf /tmp/wrsdk.tar.bz2 -C /opt/wrsdk --strip-components=1 && \
    rm /tmp/wrsdk.tar.bz2

# sdkenv.sh requires bash as /bin/sh
RUN ln -sf /bin/bash /bin/sh

# Install host Rust toolchain (for native builds + cargo)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

# Set up VxWorks SDK environment in profile
# Must unset WIND_SDK_HOME first so sdkenv.sh doesn't skip setup
RUN echo 'unset WIND_SDK_HOME' >> /root/.bashrc && \
    echo '. /opt/wrsdk/sdkenv.sh' >> /root/.bashrc && \
    echo '. ${WIND_SDK_HOME}/vxsdk/sysroot/usr/rust/rustenv.linux' >> /root/.bashrc && \
    echo 'export RUST_VSB_DIR="${WIND_CC_SYSROOT}"' >> /root/.bashrc
ENV SHELL=/bin/bash

# Copy test scripts
COPY scripts/kiln-vxworks-entrypoint.sh /usr/local/bin/kiln-vxworks
COPY scripts/vxworks-smoke-test.exp /usr/local/bin/vxworks-smoke-test.exp
RUN chmod +x /usr/local/bin/kiln-vxworks /usr/local/bin/vxworks-smoke-test.exp

WORKDIR /workspace
