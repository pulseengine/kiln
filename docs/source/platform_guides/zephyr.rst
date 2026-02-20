==========================
Zephyr RTOS Installation Guide
==========================

Kiln provides comprehensive support for Zephyr RTOS, enabling WebAssembly execution in resource-constrained embedded systems and IoT devices.

.. contents:: On this page
   :local:
   :depth: 2

Zephyr Platform Support
=======================

Supported Zephyr Versions
-------------------------

* **Zephyr 3.4 LTS** - Full support
* **Zephyr 3.5+** - Full support
* **Zephyr 3.2, 3.3** - Limited support

**Board Support:**

* **Development boards:** ``native_posix``, ``qemu_x86``, ``qemu_cortex_m3``
* **ARM Cortex-M:** STM32, nRF52, nRF91 series
* **RISC-V:** ESP32-C3, SiFive boards
* **x86:** Intel Apollo Lake, Quark

**Memory Requirements:**

* **Minimum:** 64 KB RAM, 128 KB Flash
* **Recommended:** 256 KB RAM, 512 KB Flash
* **Optimal:** 1 MB RAM, 2 MB Flash

Prerequisites
=============

Development Environment Setup
-----------------------------

Based on the justfile configuration, Kiln includes Zephyr targets. Let's set up the environment:

**Install Zephyr dependencies:**

.. code-block:: bash

   # Install Python and west
   python3 -m pip install --user west

   # Install Zephyr SDK
   wget https://github.com/zephyrproject-rtos/sdk-ng/releases/download/v0.16.5-1/zephyr-sdk-0.16.5-1_linux-x86_64.tar.gz
   tar xvf zephyr-sdk-0.16.5-1_linux-x86_64.tar.gz
   cd zephyr-sdk-0.16.5-1
   ./setup.sh

**Set up Zephyr workspace:**

.. code-block:: bash

   # Initialize Zephyr workspace (as configured in justfile)
   just zephyr-setup-venv
   just zephyr-init

**Install build tools:**

.. code-block:: bash

   # Install cmake and ninja
   sudo apt install cmake ninja-build

   # Verify installation
   west --version
   cmake --version

Kiln Zephyr Integration
======================

According to the justfile, Kiln has pre-configured Zephyr targets:

.. code-block:: bash

   # Available Zephyr commands from justfile:
   just zephyr-setup-sdk       # Set up Zephyr SDK
   just zephyr-setup-venv      # Set up Python virtual environment
   just zephyr-init            # Initialize Zephyr workspace
   just zephyr-build           # Build applications
   just zephyr-run             # Run applications

Build Kiln for Zephyr
---------------------

**Configure for embedded targets:**

.. code-block:: bash

   # Add Zephyr-compatible Rust targets
   rustup target add thumbv7em-none-eabihf    # ARM Cortex-M4F
   rustup target add thumbv8m.main-none-eabi  # ARM Cortex-M33
   rustup target add riscv32imc-unknown-none-elf # RISC-V

**Build Kiln with no_std:**

.. code-block:: bash

   # Build for embedded (no_std)
   cargo build --target thumbv7em-none-eabihf --no-default-features --features embedded

   # Verify no_std compatibility
   just verify-no-std

Platform Configuration
======================

Zephyr Integration Layer
------------------------

Kiln provides Zephyr-specific integration through the platform layer:

**Create Zephyr application with Kiln:**

.. code-block:: c

   // main.c - Zephyr application
   #include <zephyr/kernel.h>
   #include <zephyr/device.h>
   #include <kiln_zephyr.h>

   void main(void) {
       printk("Starting Kiln on Zephyr\\n");
       
       // Initialize Kiln runtime
       kiln_runtime_t* runtime = kiln_init();
       
       // Load WebAssembly module
       const uint8_t* module_bytes = get_wasm_module();
       size_t module_size = get_wasm_module_size();
       
       kiln_module_t* module = kiln_load_module(runtime, module_bytes, module_size);
       if (module) {
           kiln_execute(module, "main", NULL, 0);
       }
       
       kiln_cleanup(runtime);
   }

**CMakeLists.txt configuration:**

.. code-block:: cmake

   # CMakeLists.txt
   cmake_minimum_required(VERSION 3.20.0)
   find_package(Zephyr REQUIRED HINTS $ENV{ZEPHYR_BASE})
   project(kiln_zephyr_app)

   target_sources(app PRIVATE src/main.c)
   
   # Add Kiln library
   target_link_libraries(app PRIVATE kiln)
   target_include_directories(app PRIVATE include)

**prj.conf (Zephyr configuration):**

.. code-block:: kconfig

   # Kernel configuration
   CONFIG_MAIN_STACK_SIZE=8192
   CONFIG_HEAP_MEM_POOL_SIZE=65536
   
   # Enable newlib for better C library support
   CONFIG_NEWLIB_LIBC=y
   CONFIG_NEWLIB_LIBC_NANO=n
   
   # Memory management
   CONFIG_KERNEL_MEM_POOL=y
   CONFIG_MEM_POOL_HEAP_BACKEND=y
   
   # Networking (if needed)
   CONFIG_NETWORKING=y
   CONFIG_NET_TCP=y
   CONFIG_NET_UDP=y

Memory Management
-----------------

**Configure memory layout:**

.. code-block:: dts

   // Device tree overlay (boards/your_board.overlay)
   / {
       chosen {
           zephyr,sram = &sram0;
           zephyr,flash = &flash0;
       };
   };

   &sram0 {
       reg = <0x20000000 0x40000>; // 256KB RAM
   };

**Memory pool configuration:**

.. code-block:: c

   // Configure Kiln memory pool for Zephyr
   #define KILN_HEAP_SIZE (32 * 1024)  // 32KB heap
   K_HEAP_DEFINE(kiln_heap, KILN_HEAP_SIZE);

   void* kiln_malloc(size_t size) {
       return k_heap_alloc(&kiln_heap, size, K_NO_WAIT);
   }

   void kiln_free(void* ptr) {
       k_heap_free(&kiln_heap, ptr);
   }

Board-Specific Configuration
===========================

Native POSIX (Development)
--------------------------

**Build and test on native_posix:**

.. code-block:: bash

   # Build for native POSIX (as configured in justfile)
   just zephyr-build hello_world native_posix

   # Run the application
   just zephyr-run hello_world native_posix

   # Or manually:
   west build -b native_posix samples/basic/hello_world
   west build -t run

ARM Cortex-M (Production)
--------------------------

**STM32 boards:**

.. code-block:: bash

   # Build for STM32F4 Discovery
   west build -b stm32f4_disco samples/basic/hello_world

   # Flash to board
   west flash

**nRF52 boards:**

.. code-block:: bash

   # Build for nRF52840 DK
   west build -b nrf52840dk_nrf52840 samples/basic/hello_world

   # Flash via J-Link
   west flash --runner jlink

**Custom board configuration:**

.. code-block:: dts

   // Custom board device tree
   /dts-v1/;
   #include <st/f4/stm32f407Xg.dtsi>

   / {
       model = "Custom Kiln Board";
       compatible = "custom,kiln-board", "st,stm32f407";

       chosen {
           zephyr,sram = &sram0;
           zephyr,flash = &flash0;
       };
   };

RISC-V Targets
--------------

**ESP32-C3:**

.. code-block:: bash

   # Build for ESP32-C3
   west build -b esp32c3_devkitm samples/basic/hello_world

   # Flash via esptool
   west flash

**SiFive boards:**

.. code-block:: bash

   # Build for HiFive1
   west build -b hifive1 samples/basic/hello_world

Real-Time Configuration
======================

Thread Configuration
--------------------

**Configure Kiln threads for real-time:**

.. code-block:: c

   // Thread priorities for real-time operation
   #define KILN_MAIN_THREAD_PRIORITY    5
   #define KILN_WORKER_THREAD_PRIORITY  7
   #define KILN_GC_THREAD_PRIORITY      10

   // Stack sizes
   #define KILN_MAIN_STACK_SIZE    4096
   #define KILN_WORKER_STACK_SIZE  2048

   K_THREAD_DEFINE(kiln_main_thread, KILN_MAIN_STACK_SIZE,
                   kiln_main_thread_entry, NULL, NULL, NULL,
                   KILN_MAIN_THREAD_PRIORITY, 0, 0);

Interrupt Handling
-----------------

**Kiln interrupt integration:**

.. code-block:: c

   // Interrupt-safe Kiln operations
   void timer_isr(const struct device* dev) {
       // Signal Kiln runtime from interrupt context
       kiln_signal_from_isr();
   }

   // Configure timer for Kiln scheduling
   static const struct device* timer_dev = DEVICE_DT_GET(DT_ALIAS(timer0));
   irq_connect_dynamic(DT_IRQN(DT_ALIAS(timer0)), 0, timer_isr, NULL, 0);

Power Management
===============

Low Power Integration
--------------------

**Configure power states:**

.. code-block:: kconfig

   # Power management
   CONFIG_PM=y
   CONFIG_PM_DEVICE=y
   CONFIG_PM_DEVICE_RUNTIME=y

**Kiln power awareness:**

.. code-block:: c

   // Power-aware Kiln execution
   void kiln_idle_hook(void) {
       // Enter low power state when Kiln is idle
       pm_state_set(PM_STATE_SUSPEND_TO_IDLE);
   }

   // Configure Kiln for power efficiency
   kiln_config_t config = {
       .power_mode = KILN_POWER_LOW,
       .idle_callback = kiln_idle_hook,
       .sleep_threshold_ms = 10
   };

Networking Integration
=====================

Network Stack Configuration
--------------------------

**Enable networking:**

.. code-block:: kconfig

   # Networking
   CONFIG_NETWORKING=y
   CONFIG_NET_IPV4=y
   CONFIG_NET_UDP=y
   CONFIG_NET_TCP=y
   CONFIG_NET_SOCKETS=y

**Kiln network interface:**

.. code-block:: c

   // Network-enabled Kiln module
   #include <zephyr/net/socket.h>

   int kiln_network_handler(kiln_call_t* call) {
       int sock = socket(AF_INET, SOCK_STREAM, 0);
       // Handle network operations from WebAssembly
       return 0;
   }

Testing and Debugging
=====================

Debugging on Zephyr
-------------------

**Enable debugging:**

.. code-block:: kconfig

   # Debugging configuration
   CONFIG_DEBUG=y
   CONFIG_DEBUG_INFO=y
   CONFIG_ASSERT=y
   CONFIG_CONSOLE=y
   CONFIG_UART_CONSOLE=y

**Debug with OpenOCD:**

.. code-block:: bash

   # Start OpenOCD server
   west debugserver

   # Connect with GDB (in another terminal)
   west debug

**Serial console debugging:**

.. code-block:: bash

   # Monitor serial output
   minicom -D /dev/ttyACM0 -b 115200

Performance Testing
-------------------

**Benchmark Kiln on Zephyr:**

.. code-block:: c

   // Performance measurement
   #include <zephyr/timing/timing.h>

   void benchmark_kiln(void) {
       timing_t start, end;
       uint64_t cycles;

       timing_start();
       start = timing_counter_get();
       
       // Execute WebAssembly module
       kiln_execute(module, "benchmark", NULL, 0);
       
       end = timing_counter_get();
       cycles = timing_cycles_get(&start, &end);
       
       printk("Execution took %lld cycles\\n", cycles);
   }

Deployment
==========

Production Deployment
--------------------

**Flash layout optimization:**

.. code-block:: dts

   // Optimized flash layout for Kiln
   &flash0 {
       partitions {
           compatible = "fixed-partitions";
           #address-cells = <1>;
           #size-cells = <1>;

           boot_partition: partition@0 {
               label = "mcuboot";
               reg = <0x00000000 0x10000>;
           };
           
           slot0_partition: partition@10000 {
               label = "image-0";
               reg = <0x00010000 0x60000>;
           };
           
           wasm_storage: partition@70000 {
               label = "wasm-modules";
               reg = <0x00070000 0x10000>;
           };
       };
   };

**Over-the-air updates:**

.. code-block:: c

   // OTA update for WebAssembly modules
   int kiln_ota_update(const uint8_t* new_module, size_t size) {
       // Validate module
       if (!kiln_validate_module(new_module, size)) {
           return -EINVAL;
       }
       
       // Write to flash storage
       flash_write(flash_dev, WASM_STORAGE_OFFSET, new_module, size);
       
       // Reload runtime
       kiln_reload_module();
       return 0;
   }

Troubleshooting
===============

Common Issues
-------------

**Memory allocation failures:**

.. code-block:: bash

   # Increase heap size in prj.conf
   CONFIG_HEAP_MEM_POOL_SIZE=131072  # 128KB

   # Check memory usage
   kernel statistics shell command: "kernel stacks"

**Stack overflow:**

.. code-block:: kconfig

   # Increase stack sizes
   CONFIG_MAIN_STACK_SIZE=16384
   CONFIG_IDLE_STACK_SIZE=1024

**Flash storage issues:**

.. code-block:: bash

   # Check flash configuration
   west build -t menuconfig
   # Navigate to Device Drivers -> Flash

Performance Issues
-----------------

**Optimize build for size:**

.. code-block:: kconfig

   CONFIG_SIZE_OPTIMIZATIONS=y
   CONFIG_LTO=y

**Disable unnecessary features:**

.. code-block:: kconfig

   CONFIG_PRINTK=n
   CONFIG_CONSOLE=n
   CONFIG_UART_CONSOLE=n

Next Steps
==========

* Explore :doc:`../examples/platform/embedded_platforms` for embedded-specific examples
* Review :doc:`../architecture/platform_layer` for Zephyr integration details
* See :doc:`../development/no_std_development` for embedded development guidelines