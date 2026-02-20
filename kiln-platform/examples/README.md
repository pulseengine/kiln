# WRT Platform Examples

This directory contains comprehensive examples demonstrating how to use and extend the WRT platform abstraction layer.

## Directory Structure

```
examples/
├── concepts/           # Core platform concepts and architecture
├── platforms/          # Platform-specific usage examples  
├── external/           # External platform provider examples
└── templates/          # Templates for creating new platforms
```

## Getting Started

### 1. Understanding Platform Concepts

Start with the conceptual examples to understand WRT's platform abstraction:

- [`concepts/platform_abstraction.rs`](concepts/platform_abstraction.rs) - Core concepts and trait system

### 2. Platform-Specific Examples

Explore platform-specific implementations and usage patterns:

- [`platforms/vxworks_rtp.rs`](platforms/vxworks_rtp.rs) - VxWorks RTP (user-space) usage
- [`platforms/vxworks_lkm.rs`](platforms/vxworks_lkm.rs) - VxWorks LKM (kernel-space) usage  
- [`platforms/vxworks_portable.rs`](platforms/vxworks_portable.rs) - Cross-platform VxWorks code

### 3. External Platform Development

Learn how to create your own platform support:

- [`external/custom_platform.rs`](external/custom_platform.rs) - Complete external platform example
- [`external/integration_guide.rs`](external/integration_guide.rs) - Step-by-step integration guide

## Running Examples

### Prerequisites

Most examples compile on any platform and provide educational output. Platform-specific examples require the target platform or show conceptual information.

### Basic Usage

```bash
# Run concept demonstration
cargo run --example platform_abstraction

# Run VxWorks examples (works on any platform)
cargo run --example vxworks_portable
cargo run --example vxworks_rtp
cargo run --example vxworks_lkm

# Run external platform guides
cargo run --example custom_platform
cargo run --example integration_guide
```

### Platform-Specific Builds

For actual platform-specific functionality:

```bash
# Build for VxWorks (requires VxWorks toolchain)
cargo build --target=vxworks --features=platform-vxworks

# Build with specific platform features
cargo build --features=platform-linux,platform-macos
```

## Example Categories

### 🧠 Concepts (`concepts/`)

Educational examples that explain WRT's platform abstraction concepts:

- **Platform Abstraction**: Core traits and design patterns
- **Zero-Cost Abstractions**: How traits compile to optimal code
- **Cross-Platform Compatibility**: Writing portable platform code

### 🔧 Platform Usage (`platforms/`)

Real-world usage examples for supported platforms:

- **VxWorks RTP**: User-space applications with POSIX APIs
- **VxWorks LKM**: Kernel modules with VxWorks native APIs
- **Portable Code**: Conditional compilation patterns

### 🌐 External Platforms (`external/`)

Complete guides for extending WRT with new platforms:

- **Custom Platform**: Full implementation example
- **Integration Guide**: Step-by-step development process
- **Best Practices**: Testing, publishing, and maintenance

## Key Learning Paths

### For Platform Users

1. **Start Here**: `concepts/platform_abstraction.rs`
2. **Your Platform**: Find your platform in `platforms/`
3. **Integration**: See how it works with WRT runtime

### For Platform Developers

1. **Understand Traits**: `concepts/platform_abstraction.rs`
2. **Study Examples**: `platforms/vxworks_*.rs` 
3. **Follow Guide**: `external/integration_guide.rs`
4. **Use Template**: `../templates/external_platform/`

### For Contributors

1. **Core Concepts**: `concepts/platform_abstraction.rs`
2. **Existing Patterns**: All `platforms/` examples
3. **Extension Model**: `external/custom_platform.rs`

## Templates

The [`../templates/`](../templates/) directory contains starter templates:

- `external_platform/` - Complete crate template for new platforms
- `external_platform_simple.rs` - Single-file template for quick prototyping

## Features Demonstrated

### Core Traits
- ✅ `PageAllocator` - Memory management for WASM pages
- ✅ `FutexLike` - Low-level synchronization primitives

### Platform Capabilities  
- ✅ Memory allocation with alignment requirements
- ✅ Memory growth and deallocation
- ✅ Futex-like wait/wake semantics
- ✅ Timeout handling
- ✅ Error propagation

### Integration Patterns
- ✅ Builder patterns for configuration
- ✅ Capability detection
- ✅ Conditional compilation
- ✅ Fallback implementations
- ✅ Testing strategies

### Advanced Features
- ✅ No-std compatibility
- ✅ Platform-specific optimizations
- ✅ Real-time system support
- ✅ Memory protection
- ✅ Priority inheritance

## Platform Support Matrix

| Platform | Status | Examples | Real Implementation |
|----------|--------|-----------|-------------------|
| Linux | ✅ Core | ✅ | ✅ In wrt-platform |
| macOS | ✅ Core | ✅ | ✅ In wrt-platform |
| QNX | ✅ Core | ✅ | ✅ In wrt-platform |
| VxWorks | ✅ Core | ✅ | ✅ In wrt-platform |
| Zephyr | ✅ Core | ⚠️ | ✅ In wrt-platform |
| Tock OS | ✅ Core | ⚠️ | ✅ In wrt-platform |
| Custom | ✅ External | ✅ | 📝 Your implementation |

**Legend**: ✅ Available, ⚠️ Limited, ❌ Not supported, 📝 Developer provided

## Contributing

When adding new examples:

1. **Follow the structure**: Place examples in the appropriate directory
2. **Add documentation**: Include comprehensive comments and doc strings  
3. **Test thoroughly**: Ensure examples compile and run on all platforms
4. **Update this README**: Add your example to the appropriate section

### Example Template

```rust
//! Example Title
//!
//! Brief description of what this example demonstrates.
//! Include any prerequisites or special build requirements.

// Example code with comprehensive comments
fn main() {
    println!("=== Example Title ===");
    // Implementation...
}
```

## Support

- 📖 **Documentation**: Each example includes comprehensive documentation
- 🐛 **Issues**: Report problems via GitHub issues
- 💬 **Discussions**: Join platform-specific discussions
- 🤝 **Contributing**: See [CONTRIBUTING.md](../../CONTRIBUTING.md)

## Next Steps

After exploring these examples:

1. **Try WRT**: Integrate with the main WRT runtime
2. **Build Applications**: Create WebAssembly applications using your platform
3. **Optimize Performance**: Profile and tune for your specific use case
4. **Contribute Back**: Share your platform implementations with the community