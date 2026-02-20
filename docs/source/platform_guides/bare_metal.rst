============================
Bare Metal Installation Guide
============================

Kiln supports bare-metal deployment for maximum performance and control in custom hardware platforms, embedded systems, and safety-critical applications.

.. contents:: On this page
   :local:
   :depth: 2

Bare Metal Overview
===================

What is Bare Metal?
-------------------

Bare-metal deployment means running Kiln directly on hardware without an operating system, providing:

* **Maximum performance** - No OS overhead
* **Deterministic behavior** - Predictable timing
* **Complete control** - Full hardware access
* **Minimal footprint** - Reduced memory usage
* **Safety certification** - Simplified validation

Supported Platforms
--------------------

**ARM Cortex-M:**
* Cortex-M4F, M7, M33, M55
* STM32, NXP i.MX RT, Nordic nRF series
* Custom ARM-based microcontrollers

**RISC-V:**
* RV32IMC, RV64GC
* SiFive cores, ESP32-C3/C6
* Custom RISC-V implementations

**x86/x64:**
* Intel x86_64 (for testing/development)
* AMD64 compatible processors
* Custom x86 embedded systems

Hardware Requirements
=====================

Minimum Requirements
--------------------

* **RAM:** 32 KB (for minimal configurations)
* **Flash:** 64 KB (runtime + small modules)  
* **CPU:** 32-bit with basic arithmetic
* **Clock:** 1 MHz minimum (higher recommended)

Recommended Configuration
------------------------

* **RAM:** 256 KB - 1 MB
* **Flash:** 512 KB - 2 MB
* **CPU:** ARM Cortex-M4F or equivalent
* **Clock:** 64+ MHz
* **Peripherals:** UART for debugging

Optimal Configuration
--------------------

* **RAM:** 1+ MB (for complex applications)
* **Flash:** 2+ MB (multiple modules + OTA)
* **CPU:** ARM Cortex-M7 or equivalent  
* **Clock:** 100+ MHz
* **Peripherals:** Ethernet, USB, CAN

Development Environment
======================

Toolchain Setup
---------------

**Install Rust for embedded:**

.. code-block:: bash

   # Install Rust
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

   # Add embedded targets
   rustup target add thumbv7em-none-eabihf    # Cortex-M4F/M7
   rustup target add thumbv8m.main-none-eabi  # Cortex-M33
   rustup target add riscv32imc-unknown-none-elf # RISC-V

   # Install cargo tools
   cargo install cargo-binutils
   cargo install probe-run
   cargo install flip-link

**Install debugging tools:**

.. code-block:: bash

   # ARM GDB
   sudo apt install gdb-multiarch

   # OpenOCD for hardware debugging
   sudo apt install openocd

   # Probe-rs for modern debugging
   cargo install probe-rs --features cli

Cross-Compilation Setup
-----------------------

**Configure Cargo for cross-compilation:**

Create `.cargo/config.toml`:

.. code-block:: toml

   [target.thumbv7em-none-eabihf]
   runner = "probe-run --chip STM32F407VGTx"
   rustflags = [
     "-C", "linker=flip-link",
     "-C", "link-arg=-Tlink.x",
     "-C", "link-arg=-Tdefmt.x",
   ]

   [target.riscv32imc-unknown-none-elf]
   runner = "qemu-system-riscv32 -machine sifive_e -nographic -semihosting-config enable=on,target=native -kernel"

   [build]
   target = "thumbv7em-none-eabihf"

Kiln Bare Metal Configuration
============================

no_std Configuration
-------------------

Kiln is designed to work in `no_std` environments:

**Cargo.toml configuration:**

.. code-block:: toml

   [dependencies]
   kiln = { version = "0.1", default-features = false, features = ["bare-metal"] }
   kiln-foundation = { version = "0.1", default-features = false }
   kiln-runtime = { version = "0.1", default-features = false }

   # Bare metal essentials
   cortex-m = "0.7"
   cortex-m-rt = "0.7"
   panic-halt = "0.2"

**Main application structure:**

.. code-block:: rust

   #![no_std]
   #![no_main]

   use panic_halt as _;
   use cortex_m_rt::entry;
   use kiln::prelude::*;

   #[entry]
   fn main() -> ! {
       // Initialize hardware
       let dp = init_hardware();
       
       // Initialize Kiln runtime
       let mut runtime = WrtRuntime::new();
       
       // Load WebAssembly module from flash
       let module_bytes = include_bytes!("../modules/app.wasm");
       let module = runtime.load_module(module_bytes)?;
       
       // Execute main function
       let result = runtime.invoke(&module, "main", &[])?;
       
       loop {
           // Main application loop
           runtime.run_scheduled_tasks();
       }
   }

Memory Management
-----------------

**Static memory allocation:**

.. code-block:: rust

   use heapless::pool::{Pool, Node};
   use kiln_foundation::memory::MemoryProvider;

   // Pre-allocated memory pool
   static mut MEMORY: [Node<[u8; 1024]>; 32] = [Node::new(); 32];
   static POOL: Pool<[u8; 1024]> = Pool::new();

   struct BareMetalMemory;

   impl MemoryProvider for BareMetalMemory {
       fn allocate(&self, size: usize) -> Option<*mut u8> {
           if size <= 1024 {
               POOL.alloc().map(|node| node.as_mut_ptr())
           } else {
               None
           }
       }
       
       fn deallocate(&self, ptr: *mut u8) {
           unsafe {
               POOL.free(ptr as *mut Node<[u8; 1024]>);
           }
       }
   }

**Linker script configuration:**

Create `memory.x`:

.. code-block:: text

   MEMORY
   {
     FLASH : ORIGIN = 0x08000000, LENGTH = 1024K
     RAM : ORIGIN = 0x20000000, LENGTH = 192K
   }

   /* Kiln-specific sections */
   SECTIONS
   {
     .kiln_modules : {
       KEEP(*(.kiln_modules))
     } > FLASH
     
     .kiln_heap : {
       . = ALIGN(8);
       __kiln_heap_start = .;
       . = . + 64K;
       __kiln_heap_end = .;
     } > RAM
   }

Hardware Abstraction Layer
=========================

Platform Initialization
-----------------------

**Clock and peripheral setup:**

.. code-block:: rust

   use cortex_m::peripheral::Peripherals;
   use stm32f4xx_hal::{prelude::*, pac};

   fn init_hardware() -> pac::Peripherals {
       let dp = pac::Peripherals::take().unwrap();
       let cp = Peripherals::take().unwrap();

       // Configure clocks
       let rcc = dp.RCC.constrain();
       let clocks = rcc.cfgr.sysclk(84.mhz()).freeze();

       // Initialize Kiln-required peripherals
       init_timer(&dp, &clocks);
       init_uart(&dp, &clocks);
       
       dp
   }

**Timer for scheduling:**

.. code-block:: rust

   use cortex_m::interrupt::{free, Mutex};
   use core::cell::RefCell;

   static TIMER_COUNTER: Mutex<RefCell<u32>> = Mutex::new(RefCell::new(0));

   #[interrupt]
   fn TIM2() {
       free(|cs| {
           let mut counter = TIMER_COUNTER.borrow(cs).borrow_mut();
           *counter += 1;
           
           // Signal Kiln scheduler
           kiln_scheduler_tick();
       });
   }

Peripheral Integration
---------------------

**UART for debugging:**

.. code-block:: rust

   use nb::block;
   use stm32f4xx_hal::serial::{Serial, config::Config};

   static mut UART: Option<Serial<pac::USART1>> = None;

   pub fn debug_print(msg: &str) {
       unsafe {
           if let Some(ref mut uart) = UART {
               for byte in msg.bytes() {
                   block!(uart.write(byte)).ok();
               }
           }
       }
   }

**GPIO for status indication:**

.. code-block:: rust

   use stm32f4xx_hal::gpio::{gpioa::PA5, Output, PushPull};

   static mut STATUS_LED: Option<PA5<Output<PushPull>>> = None;

   pub fn set_status_led(state: bool) {
       unsafe {
           if let Some(ref mut led) = STATUS_LED {
               if state {
                   led.set_high();
               } else {
                   led.set_low();
               }
           }
       }
   }

Real-Time Considerations
=======================

Interrupt Handling
------------------

**Kiln interrupt integration:**

.. code-block:: rust

   use cortex_m::interrupt::{self, Mutex};
   use core::cell::RefCell;

   // Interrupt-safe Kiln operations
   type KilnState = Mutex<RefCell<Option<WrtRuntime>>>;
   static KILN_RUNTIME: KilnState = Mutex::new(RefCell::new(None));

   #[interrupt]
   fn EXTI0() {
       interrupt::free(|cs| {
           if let Some(ref mut runtime) = KILN_RUNTIME.borrow(cs).borrow_mut().as_mut() {
               // Handle external event in Kiln
               runtime.handle_interrupt_event();
           }
       });
   }

**Critical sections:**

.. code-block:: rust

   use cortex_m::interrupt;

   fn kiln_critical_section<F, R>(f: F) -> R 
   where
       F: FnOnce() -> R,
   {
       interrupt::free(|_| f())
   }

Deterministic Execution
----------------------

**Fixed-time execution:**

.. code-block:: rust

   use cortex_m::peripheral::DWT;

   struct TimingConstraints {
       max_cycles: u32,
       deadline_cycles: u32,
   }

   fn execute_with_timing(
       runtime: &mut WrtRuntime,
       module: &KilnModule,
       constraints: &TimingConstraints
   ) -> Result<(), KilnError> {
       let start = DWT::cycle_count();
       
       // Execute with cycle limit
       runtime.set_fuel(constraints.max_cycles);
       let result = runtime.invoke(module, "main", &[])?;
       
       let end = DWT::cycle_count();
       let elapsed = end.wrapping_sub(start);
       
       if elapsed > constraints.deadline_cycles {
           return Err(KilnError::DeadlineMissed);
       }
       
       Ok(())
   }

Power Management
===============

Low Power Integration
--------------------

**Sleep modes:**

.. code-block:: rust

   use cortex_m::asm;
   use stm32f4xx_hal::pwr::{Pwr, PwrExt};

   enum PowerState {
       Active,
       Sleep,
       Stop,
       Standby,
   }

   fn enter_power_state(state: PowerState) {
       match state {
           PowerState::Sleep => {
               asm::wfi(); // Wait for interrupt
           },
           PowerState::Stop => {
               // Configure stop mode
               asm::wfi();
           },
           PowerState::Standby => {
               // Configure standby mode
               asm::wfi();
           },
           PowerState::Active => {
               // Already active
           }
       }
   }

**Kiln power integration:**

.. code-block:: rust

   impl WrtRuntime {
       fn enter_idle(&mut self) {
           // Prepare for low power
           self.save_context();
           enter_power_state(PowerState::Stop);
           self.restore_context();
       }
   }

Module Management
================

Flash Storage
------------

**Embed modules in flash:**

.. code-block:: rust

   // Include WebAssembly modules at compile time
   const APP_MODULE: &[u8] = include_bytes!("../modules/app.wasm");
   const SENSOR_MODULE: &[u8] = include_bytes!("../modules/sensor.wasm");

   fn load_modules(runtime: &mut WrtRuntime) -> Result<(), KilnError> {
       let app = runtime.load_module(APP_MODULE)?;
       let sensor = runtime.load_module(SENSOR_MODULE)?;
       
       // Register modules
       runtime.register_module("app", app);
       runtime.register_module("sensor", sensor);
       
       Ok(())
   }

**Dynamic loading from external flash:**

.. code-block:: rust

   use embedded_hal::spi::SpiDevice;

   fn load_from_external_flash<SPI>(
       spi: &mut SPI,
       address: u32,
       size: usize
   ) -> Result<Vec<u8>, KilnError> 
   where
       SPI: SpiDevice
   {
       let mut buffer = vec![0u8; size];
       
       // Read from external flash
       spi.write(&[0x03, (address >> 16) as u8, (address >> 8) as u8, address as u8])?;
       spi.read(&mut buffer)?;
       
       Ok(buffer)
   }

Testing and Debugging
=====================

Hardware Testing
---------------

**Unit tests on hardware:**

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;
       
       #[test]
       fn test_kiln_basic_execution() {
           let mut runtime = WrtRuntime::new();
           let module = runtime.load_module(SIMPLE_MODULE).unwrap();
           
           let result = runtime.invoke(&module, "add", &[1, 2]).unwrap();
           assert_eq!(result, 3);
       }
   }

   // Run with: cargo test --target thumbv7em-none-eabihf

**Integration testing:**

.. code-block:: bash

   # Test on hardware with probe-run
   cargo run --release --bin integration_test

   # Test with QEMU
   cargo run --target riscv32imc-unknown-none-elf

Debugging Techniques
-------------------

**Debug output via ITM:**

.. code-block:: rust

   use cortex_m::itm;

   fn debug_trace(msg: &str) {
       if let Some(mut itm) = itm::write_str(&mut cp.ITM.stim[0]) {
           itm.write_str(msg).ok();
       }
   }

**Real-time tracing:**

.. code-block:: rust

   use rtt_target::{rprintln, rtt_init_print};

   #[entry]
   fn main() -> ! {
       rtt_init_print!();
       rprintln!("Kiln starting...");
       
       // Your code here
   }

Performance Optimization
========================

Code Size Optimization
----------------------

**Optimize for size:**

.. code-block:: toml

   [profile.release]
   opt-level = "z"     # Optimize for size
   lto = true          # Link-time optimization
   codegen-units = 1   # Better optimization
   panic = "abort"     # Smaller panic handler

**Feature selection:**

.. code-block:: toml

   [features]
   default = []
   full = ["std", "alloc"]
   bare-metal = ["no-std", "static-memory"]
   minimal = ["no-std", "static-memory", "no-float"]

**Strip unused code:**

.. code-block:: bash

   # Use cargo-bloat to analyze size
   cargo install cargo-bloat
   cargo bloat --release --crates

Performance Tuning
------------------

**Compiler optimizations:**

.. code-block:: bash

   # Target-specific optimizations
   export RUSTFLAGS="-C target-cpu=cortex-m4 -C target-feature=+fp-armv8"

**Profile-guided optimization:**

.. code-block:: rust

   // Hot path optimization
   #[inline(always)]
   fn critical_function() {
       // Performance-critical code
   }

   // Cold path optimization  
   #[cold]
   fn error_handler() {
       // Error handling code
   }

Deployment Strategies
====================

Bootloader Integration
---------------------

**Simple bootloader:**

.. code-block:: rust

   #[no_mangle]
   #[link_section = ".boot"]
   pub unsafe extern "C" fn bootloader_main() {
       // Initialize minimal hardware
       init_clocks();
       
       // Verify application integrity
       if verify_application() {
           // Jump to main application
           jump_to_application();
       } else {
           // Enter recovery mode
           recovery_mode();
       }
   }

**Over-the-air updates:**

.. code-block:: rust

   fn ota_update(new_firmware: &[u8]) -> Result<(), OtaError> {
       // Verify signature
       verify_signature(new_firmware)?;
       
       // Write to backup partition
       write_to_flash(BACKUP_PARTITION, new_firmware)?;
       
       // Set boot flag
       set_boot_partition(BACKUP_PARTITION);
       
       // Restart
       cortex_m::peripheral::SCB::sys_reset();
   }

Production Considerations
------------------------

**Watchdog integration:**

.. code-block:: rust

   use stm32f4xx_hal::watchdog::IndependentWatchdog;

   static mut WATCHDOG: Option<IndependentWatchdog> = None;

   fn init_watchdog() {
       unsafe {
           WATCHDOG = Some(IndependentWatchdog::new(dp.IWDG));
           WATCHDOG.as_mut().unwrap().start(1000.ms());
       }
   }

   fn kiln_main_loop() {
       loop {
           // Execute Kiln tasks
           runtime.run_scheduled_tasks();
           
           // Pet the watchdog
           unsafe {
               if let Some(ref mut wd) = WATCHDOG {
                   wd.feed();
               }
           }
       }
   }

**Error recovery:**

.. code-block:: rust

   #[panic_handler]
   fn panic_handler(info: &PanicInfo) -> ! {
       // Log panic information
       debug_print(&format!("Panic: {:?}", info));
       
       // Attempt graceful shutdown
       shutdown_peripherals();
       
       // Reset system
       cortex_m::peripheral::SCB::sys_reset();
   }

Next Steps
==========

* Explore :doc:`../examples/platform/embedded_platforms` for practical examples
* Review :doc:`../architecture/safe_memory` for memory safety in bare-metal
* See :doc:`../development/no_std_development` for advanced embedded development