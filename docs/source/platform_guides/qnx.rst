======================
QNX Installation Guide
======================

Kiln provides specialized support for QNX Neutrino, the real-time operating system used in safety-critical automotive, medical, and industrial applications.

.. contents:: On this page
   :local:
   :depth: 2

QNX Platform Support
====================

Supported Versions
------------------

**QNX SDP (Software Development Platform):**

* QNX SDP 7.1 - Full support
* QNX SDP 7.0 - Full support  
* QNX SDP 6.6 - Limited support

**Target Architectures:**

* **x86_64** - Primary development platform
* **aarch64** - ARM 64-bit (automotive grade)
* **armv7** - ARM 32-bit (embedded systems)

**Safety Certifications:**

* ISO 26262 (Automotive) - ASIL B/C ready
* IEC 61508 (Industrial) - SIL 2/3 ready
* DO-178C (Avionics) - DAL C ready

Prerequisites
=============

QNX Development Environment
---------------------------

**Required components:**

1. **QNX SDP** - Software Development Platform
2. **QNX Momentics IDE** - Integrated development environment
3. **Target hardware** or **QNX VMware image**

**Install QNX SDP:**

.. code-block:: bash

   # Download QNX SDP from qnx.com (requires license)
   # Extract and install
   ./qnx-sdp-7.1-install

   # Set environment variables
   source ~/qnx710/qnxsdp-env.sh

**Verify installation:**

.. code-block:: bash

   echo $QNX_HOST
   echo $QNX_TARGET
   qcc --version

Cross-Compilation Setup
-----------------------

**Install Rust for QNX targets:**

.. code-block:: bash

   # Add QNX targets to Rust
   rustup target add x86_64-pc-nto-qnx710
   rustup target add aarch64-unknown-nto-qnx710

**Configure Cargo for cross-compilation:**

Create `~/.cargo/config.toml`:

.. code-block:: toml

   [target.x86_64-pc-nto-qnx710]
   linker = "qcc"
   ar = "ntoaarch64-ar"

   [target.aarch64-unknown-nto-qnx710]
   linker = "ntoaarch64-gcc"
   ar = "ntoaarch64-ar"

   [env]
   QNX_HOST = "/home/user/qnx710/host/linux/x86_64"
   QNX_TARGET = "/home/user/qnx710/target/qnx7"

Installation Methods
====================

Source Installation
-------------------

**Build Kiln for QNX:**

.. code-block:: bash

   # Set QNX environment
   source ~/qnx710/qnxsdp-env.sh

   # Clone Kiln repository
   git clone https://github.com/pulseengine/kiln.git
   cd kiln

   # Build for QNX x86_64
   cargo build --target x86_64-pc-nto-qnx710 --release

   # Build for QNX ARM64
   cargo build --target aarch64-unknown-nto-qnx710 --release

**Cross-compile with justfile:**

.. code-block:: bash

   # Build with QNX platform features
   cargo build --features platform-qnx

   # Build for specific QNX targets (requires QNX toolchain)
   cargo build --target x86_64-pc-nto-qnx710
   cargo build --target aarch64-unknown-nto-qnx710

Momentics IDE Integration
-------------------------

**Import Kiln as Momentics project:**

1. Open QNX Momentics IDE
2. File → Import → General → Existing Projects
3. Select Kiln directory
4. Configure build targets

**Create new QNX project with Kiln:**

.. code-block:: bash

   # Create QNX application project
   qnx-create-project --type=application --name=kiln-app

   # Add Kiln dependency to Makefile
   LIBS += -lkiln

QNX-Specific Configuration
=========================

Resource Managers
-----------------

Kiln integrates with QNX resource managers:

**Memory management:**

.. code-block:: c

   // Configure memory allocator for QNX
   #include <sys/mman.h>
   
   // Use QNX-specific memory allocation
   void* memory = mmap(NULL, size, PROT_READ | PROT_WRITE, 
                       MAP_PRIVATE | MAP_ANON, DEVMEM_FD, 0);

**Process management:**

.. code-block:: toml

   # Kiln configuration for QNX
   [qnx]
   priority = 10          # Real-time priority
   scheduling = "FIFO"    # Scheduling policy
   cpu_affinity = [0, 1]  # Pin to specific CPUs

Message Passing
---------------

**Pulses and messages:**

.. code-block:: rust

   // QNX message passing integration
   use kiln_qnx::messaging::*;

   let channel = ChannelCreate(0)?;
   let connection = ConnectAttach(0, 0, channel, _NTO_SIDE_CHANNEL, 0)?;

Real-Time Configuration
=======================

Scheduling and Priorities
-------------------------

**Configure real-time scheduling:**

.. code-block:: bash

   # Set Kiln process priority
   pidin -p kilnd
   nice -n -10 kilnd module.wasm

   # Use real-time scheduling
   chrt -f 50 kilnd module.wasm

**Thread priorities:**

.. code-block:: toml

   # Kiln thread configuration
   [runtime.threads]
   main_priority = 50
   worker_priority = 45
   gc_priority = 30

Memory Management
-----------------

**Configure memory pools:**

.. code-block:: toml

   [memory]
   # Use QNX memory pools
   pool_size = "16MB"
   page_size = 4096
   
   # Enable memory locking
   lock_memory = true
   
   # QNX-specific options
   use_typed_memory = true
   memory_class = "below4G"

**Avoid memory fragmentation:**

.. code-block:: bash

   # Pre-allocate memory pools
   export KILN_PREALLOC_SIZE=67108864  # 64MB

Interrupt Handling
-----------------

**Configure interrupt priorities:**

.. code-block:: bash

   # Show interrupt assignments
   pidin -P interrupts

   # Set Kiln interrupt affinity
   echo 2 > /proc/irq/24/smp_affinity

Safety and Reliability
======================

Fault Tolerance
---------------

**Process monitoring:**

.. code-block:: bash

   # Use QNX High Availability
   ham_node -i 1 -p 100 kilnd

   # Configure watchdog
   wdtkick -t 5000 &

**Error handling:**

.. code-block:: toml

   [safety]
   # Enable comprehensive error checking
   strict_validation = true
   memory_protection = true
   
   # QNX-specific safety features
   enable_guardian = true
   watchdog_timeout = 5000

Memory Protection
-----------------

**Address space layout:**

.. code-block:: bash

   # Show memory layout
   pidin -m kilnd

   # Configure memory protection
   mprotect address size PROT_READ

**Stack protection:**

.. code-block:: toml

   [stack]
   # Guard pages for stack overflow detection
   guard_pages = 2
   stack_size = 1048576

Performance Optimization
========================

QNX-Specific Optimizations
--------------------------

**CPU affinity:**

.. code-block:: bash

   # Bind to specific CPU cores
   runon -c 1,2 kilnd module.wasm

   # Check CPU affinity
   pidin -A kilnd

**Memory optimization:**

.. code-block:: bash

   # Use huge pages
   mmap -h 2M

   # Prefault memory
   echo 1 > /proc/sys/vm/drop_caches

Network Performance
-------------------

**io-pkt optimization:**

.. code-block:: bash

   # Optimize network stack
   io-pkt-v6-hc -d e1000 -p tcpip

   # Tune network buffers
   sysctl -w net.inet.tcp.sendspace=65536

Deployment
==========

Target System Deployment
------------------------

**Transfer to QNX target:**

.. code-block:: bash

   # Copy via network
   scp target/aarch64-unknown-nto-qnx710/release/kilnd root@qnx-target:/usr/bin/

   # Copy via USB
   mount -t dos /dev/umass0 /mnt
   cp kilnd /mnt/

**System integration:**

.. code-block:: bash

   # Add to system startup
   echo "kilnd /opt/modules/app.wasm &" >> /etc/rc.d/rc.local

   # Create system service
   slinger -d -P /usr/bin/kilnd

Automotive Integration
---------------------

**AUTOSAR compatibility:**

.. code-block:: c

   // AUTOSAR RTE integration
   #include "Rte_KilnComponent.h"
   
   Std_ReturnType KilnComponent_Init(void) {
       return kiln_runtime_init();
   }

**CAN bus integration:**

.. code-block:: bash

   # Start CAN driver
   dev-can-mx6x -c 1000000

   # Configure Kiln for CAN
   export KILN_CAN_INTERFACE=can0

Testing and Validation
======================

QNX Test Environment
--------------------

**VM setup:**

.. code-block:: bash

   # Start QNX VM
   qvm create qnx710-vm
   qvm start qnx710-vm

   # Run tests with QNX features (requires QNX environment)
   cargo test --features platform-qnx

**Hardware-in-the-loop testing:**

.. code-block:: bash

   # Connect to target hardware
   qconn target_ip

   # Run automated tests on QNX target
   # Note: Requires cross-compilation and QNX target environment
   cargo test --target x86_64-pc-nto-qnx710

Real-Time Testing
-----------------

**Latency measurement:**

.. code-block:: bash

   # Measure interrupt latency
   tracelogger -n 1000 -f /tmp/trace.kev

   # Analyze timing
   traceviz /tmp/trace.kev

**Load testing:**

.. code-block:: bash

   # Stress test under load
   cpuhog 90 &
   kilnd --stress-test module.wasm

Troubleshooting
===============

Common Issues
-------------

**Build failures:**

.. code-block:: bash

   # Check QNX environment
   echo $QNX_HOST $QNX_TARGET

   # Verify cross-compiler
   qcc --version
   ntoaarch64-gcc --version

**Runtime issues:**

.. code-block:: bash

   # Check library dependencies
   ldd kilnd

   # Debug with slogger
   slogger &
   slog2info

**Performance problems:**

.. code-block:: bash

   # Profile with system profiler
   profiler -P kilnd &

   # Check real-time behavior
   tracelogger -s 1000

Memory Issues
-------------

**Memory leaks:**

.. code-block:: bash

   # Use QNX memory analysis
   memtrace -o /tmp/memtrace.out kilnd module.wasm

   # Show memory statistics
   pidin -m kilnd

**Stack overflow:**

.. code-block:: bash

   # Increase stack size
   export KILN_STACK_SIZE=2097152

   # Enable stack checking
   export KILN_STACK_CHECK=1

Next Steps
==========

* Review :doc:`../examples/platform/qnx_features` for platform-specific examples
* Explore :doc:`../architecture/qnx_platform` for technical architecture
* See :doc:`../safety/index` for safety-critical development guidelines