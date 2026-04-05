#!/bin/bash
set -e

# Source VxWorks SDK environment (unset first to avoid "already sourced" skip)
unset WIND_SDK_HOME
. /opt/wrsdk/sdkenv.sh
. "${WIND_SDK_HOME}/vxsdk/sysroot/usr/rust/rustenv.linux"
export RUST_VSB_DIR="${WIND_CC_SYSROOT}"

cd /workspace

case "${1:-help}" in
  build)
    echo "=== Cross-compiling kilnd for VxWorks ==="
    cargo build --bin kilnd --target x86_64-wrs-vxworks \
      --features "std,kiln-execution,platform-vxworks"
    echo "Built: target/x86_64-wrs-vxworks/debug/kilnd"
    ;;

  test)
    echo "=== Cross-compiling kilnd for VxWorks ==="
    cargo build --bin kilnd --target x86_64-wrs-vxworks \
      --features "std,kiln-execution,platform-vxworks"

    echo "=== Downloading test components ==="
    mkdir -p /tmp/wasm-components && cd /tmp/wasm-components
    if [ ! -f wasm-components-0.2.0.tar.gz ]; then
      curl -fSL -o wasm-components-0.2.0.tar.gz \
        https://github.com/pulseengine/wasm-component-examples/releases/download/v0.2.0/wasm-components-0.2.0.tar.gz
    fi
    tar -xzf wasm-components-0.2.0.tar.gz 2>/dev/null || true
    cd /workspace

    echo "=== Preparing QEMU disk ==="
    mkdir -p /tmp/kiln-disk
    cp target/x86_64-wrs-vxworks/debug/kilnd /tmp/kiln-disk/kilnd.vxe
    cp /tmp/wasm-components/release-0.2.0/rust/hello_rust.wasm /tmp/kiln-disk/

    KERNEL=$(find "${WIND_SDK_HOME}" -name "vxWorks" -path "*/bsps/*" | head -1)
    echo "Kernel: $KERNEL"
    echo "Disk contents:"
    ls -la /tmp/kiln-disk/

    echo "=== Booting VxWorks QEMU ==="
    export KERNEL
    expect /usr/local/bin/vxworks-smoke-test.exp
    ;;

  shell)
    echo "Rust: $(rustc --version)"
    echo "Cargo: $(cargo --version)"
    exec bash
    ;;

  *)
    echo "Usage: docker run -v \$(pwd):/workspace kiln-vxworks <command>"
    echo "  build   Cross-compile kilnd for VxWorks"
    echo "  test    Build + run smoke tests in VxWorks QEMU"
    echo "  shell   Interactive shell with VxWorks SDK"
    ;;
esac
