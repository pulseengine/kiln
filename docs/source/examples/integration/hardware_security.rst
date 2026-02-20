======================================
Hardware Security Features
======================================

.. epigraph::

   "Software security is like a lock on a screen door. Hardware security is the actual door."
   
   -- Security engineer wisdom

Modern CPUs provide powerful security features that Kiln leverages to protect WebAssembly execution. The ``kiln-platform`` crate provides abstractions for these hardware capabilities, enabling defense-in-depth against sophisticated attacks.

.. admonition:: What You'll Learn
   :class: note

   - Hardware security abstractions in Kiln
   - ARM security features (PAC, MTE, BTI, TrustZone)
   - Intel security features (CET, MPK)
   - RISC-V security features (PMP, CFI)
   - Side-channel resistance techniques
   - Compile-time feature detection
   - Zero-cost security abstractions

Hardware Security Architecture 🏗️
---------------------------------

Kiln provides a unified abstraction for hardware security features across architectures:

.. code-block:: rust
   :caption: Hardware optimization traits
   :linenos:

   use kiln_platform::hardware_optimizations::{
       HardwareOptimization,
       SecurityLevel,
       HardwareOptimizer
   };
   
   // Generic trait for all hardware optimizations
   pub trait HardwareOptimization<A> {
       fn security_level() -> SecurityLevel;
       fn is_available() -> bool;
       fn enable() -> Result<Self, Error> where Self: Sized;
       fn optimize_memory(&self, ptr: *mut u8, size: usize) -> Result<(), Error>;
   }
   
   // Runtime feature detection
   let mut optimizer = HardwareOptimizer::<arch::Arm>::new();
   optimizer.detect_optimizations()?;
   
   match optimizer.security_level() {
       SecurityLevel::SecureExecution => println!("Hardware secure execution available"),
       SecurityLevel::Advanced => println!("Advanced security features available"),
       SecurityLevel::Basic => println!("Basic security features only"),
       SecurityLevel::None => println!("No hardware security features"),
   }

ARM Security Features 💪
-----------------------

Pointer Authentication (PAC)
~~~~~~~~~~~~~~~~~~~~~~~~~~~

ARM PAC provides cryptographic protection for pointers:

.. code-block:: rust
   :caption: ARM Pointer Authentication abstraction
   :linenos:

   use kiln_platform::hardware_optimizations::arm::PointerAuthentication;
   
   fn enable_pointer_authentication() -> Result<(), Error> {
       // Check compile-time availability
       if !PointerAuthentication::is_available() {
           println!("PAC not available on this system");
           return Ok(());
       }
       
       // Enable PAC (platform handles the details)
       let pac = PointerAuthentication::enable()?;
       
       // PAC configuration
       let config = PointerAuthentication::default();
       // config.pac_ia = true;  // Instruction address PAC
       // config.pac_da = true;  // Data address PAC  
       // config.pac_ga = false; // Generic authentication
       
       // Apply to memory region (implementation would sign pointers)
       pac.optimize_memory(code_ptr, code_size)?;
       
       Ok(())
   }

Memory Tagging Extension (MTE)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

MTE provides hardware-accelerated memory safety:

.. code-block:: rust
   :caption: ARM MTE abstraction  
   :linenos:

   use kiln_platform::hardware_optimizations::arm::{
       MemoryTagging,
       MteMode,
       TagStrategy
   };
   
   fn enable_memory_tagging() -> Result<(), Error> {
       if !MemoryTagging::is_available() {
           println!("MTE not available");
           return Ok(());
       }
       
       // Enable MTE with configuration
       let mte = MemoryTagging::enable()?;
       
       // Default configuration includes:
       // - mode: MteMode::Synchronous (immediate checking)
       // - tag_strategy: TagStrategy::Random
       let config = MemoryTagging::default();
       
       // Apply MTE to memory region
       mte.optimize_memory(heap_ptr, heap_size)?;
       
       Ok(())
   }
   
   // Platform-specific allocator with MTE
   #[cfg(all(target_arch = "aarch64", feature = "linux-mte"))]
   fn create_mte_protected_allocator() -> Result<(), Error> {
       use kiln_platform::{LinuxArm64MteAllocator, LinuxArm64MteAllocatorBuilder};
       
       let allocator = LinuxArm64MteAllocatorBuilder::new()
           .with_mte_mode(MteMode::Synchronous)
           .with_maximum_pages(256)
           .build()?;
           
       // Allocated memory will be MTE-protected
       let (ptr, size) = allocator.allocate(64, None)?;
       
       Ok(())
   }

Branch Target Identification (BTI)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

BTI provides control flow integrity:

.. code-block:: rust
   :caption: ARM BTI abstraction
   :linenos:

   use kiln_platform::hardware_optimizations::arm::{
       BranchTargetIdentification,
       BtiMode,
       BtiExceptionLevel
   };
   
   fn enable_bti_protection() -> Result<(), Error> {
       if !BranchTargetIdentification::is_available() {
           println!("BTI not available");
           return Ok(());
       }
       
       // Enable BTI
       let bti = BranchTargetIdentification::enable()?;
       
       // Default configuration:
       // - enable_bti: true
       // - exception_level: BtiExceptionLevel::Both  
       // - guarded_pages: true
       // - bti_mode: BtiMode::CallAndJump
       let config = BranchTargetIdentification::default();
       
       // Apply BTI protection to code memory
       bti.optimize_memory(code_ptr, code_size)?;
       
       Ok(())
   }
   
   // BTI modes available
   fn demonstrate_bti_modes() {
       use kiln_platform::hardware_optimizations::arm::BtiMode;
       
       let modes = [
           BtiMode::Standard,     // Standard BTI (bti instruction)
           BtiMode::CallOnly,     // Call-specific BTI (bti c)
           BtiMode::JumpOnly,     // Jump-specific BTI (bti j)  
           BtiMode::CallAndJump,  // Both call and jump BTI (bti jc)
       ];
   }

TrustZone Support
~~~~~~~~~~~~~~~~

TrustZone provides secure/non-secure world separation:

.. code-block:: rust
   :caption: ARM TrustZone abstraction
   :linenos:

   use kiln_platform::hardware_optimizations::arm::TrustZone;
   
   fn check_trustzone_support() -> Result<(), Error> {
       if !TrustZone::is_available() {
           println!("TrustZone not available");
           return Ok(());
       }
       
       // Enable TrustZone features
       let tz = TrustZone::enable()?;
       
       // Default configuration:
       // - secure_world: false
       // - secure_regions: &[] (empty)
       let config = TrustZone::default();
       
       // Apply security configuration to memory
       tz.optimize_memory(secure_ptr, secure_size)?;
       
       Ok(())
   }

Intel Security Features 🛡️
--------------------------

Control-flow Enforcement Technology (CET)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Intel CET provides shadow stack and indirect branch tracking:

.. code-block:: rust
   :caption: Intel CET abstraction
   :linenos:

   use kiln_platform::hardware_optimizations::intel::ControlFlowEnforcement;
   
   fn enable_intel_cet() -> Result<(), Error> {
       if !ControlFlowEnforcement::is_available() {
           println!("CET not available");
           return Ok(());
       }
       
       // Enable CET
       let cet = ControlFlowEnforcement::enable()?;
       
       // Default configuration:
       // - shadow_stack: true
       // - indirect_branch_tracking: true
       let config = ControlFlowEnforcement::default();
       
       // Apply CET protection to memory region
       cet.optimize_memory(code_ptr, code_size)?;
       
       Ok(())
   }

Memory Protection Keys (MPK)
~~~~~~~~~~~~~~~~~~~~~~~~~~~

MPK provides fine-grained memory protection:

.. code-block:: rust
   :caption: Intel MPK abstraction
   :linenos:

   use kiln_platform::hardware_optimizations::intel::{
       MemoryProtectionKeys,
       AccessRights
   };
   
   fn enable_memory_protection_keys() -> Result<(), Error> {
       if !MemoryProtectionKeys::is_available() {
           println!("MPK not available");
           return Ok(());
       }
       
       // Enable MPK
       let mpk = MemoryProtectionKeys::enable()?;
       
       // Default configuration:
       // - key_assignments: [0; 16]
       // - access_rights: [RWX; 16]
       let config = MemoryProtectionKeys::default();
       
       // Apply protection keys to memory
       mpk.optimize_memory(protected_ptr, protected_size)?;
       
       Ok(())
   }
   
   // Access rights structure
   fn demonstrate_access_rights() {
       use kiln_platform::hardware_optimizations::intel::AccessRights;
       
       let rights = AccessRights {
           read: true,
           write: true,
           execute: false,
       };
   }

RISC-V Security Features 🔒
------------------------

Physical Memory Protection (PMP)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

RISC-V PMP provides hardware memory access control:

.. code-block:: rust
   :caption: RISC-V PMP abstraction
   :linenos:

   use kiln_platform::hardware_optimizations::riscv::PhysicalMemoryProtection;
   
   fn enable_riscv_pmp() -> Result<(), Error> {
       if !PhysicalMemoryProtection::is_available() {
           println!("PMP not available");
           return Ok(());
       }
       
       // Enable PMP
       let pmp = PhysicalMemoryProtection::enable()?;
       
       // Default configuration includes array of PMP entries
       let config = PhysicalMemoryProtection::default();
       
       // Apply PMP protection
       pmp.optimize_memory(protected_ptr, protected_size)?;
       
       Ok(())
   }
   
   // PMP configuration structures
   fn demonstrate_pmp_config() {
       use kiln_platform::hardware_optimizations::riscv::{
           PmpEntry, PmpConfig, AddressMode
       };
       
       let entry = PmpEntry {
           address: 0x8000_0000,
           config: PmpConfig {
               read: true,
               write: true,
               execute: false,
               address_mode: AddressMode::Napot, // Power-of-2 region
           },
       };
   }

Control Flow Integrity (CFI)
~~~~~~~~~~~~~~~~~~~~~~~~~~~

RISC-V CFI extensions for control flow protection:

.. code-block:: rust
   :caption: RISC-V CFI abstraction
   :linenos:

   use kiln_platform::hardware_optimizations::riscv::{
       ControlFlowIntegrity,
       CfiExceptionMode
   };
   
   fn enable_riscv_cfi() -> Result<(), Error> {
       if !ControlFlowIntegrity::is_available() {
           println!("CFI not available");
           return Ok(());
       }
       
       // Enable CFI
       let cfi = ControlFlowIntegrity::enable()?;
       
       // Default configuration:
       // - shadow_stack: true
       // - landing_pads: true  
       // - backward_edge_cfi: true
       // - forward_edge_cfi: true
       // - exception_mode: CfiExceptionMode::Exception
       let config = ControlFlowIntegrity::default();
       
       // Apply CFI protection
       cfi.optimize_memory(code_ptr, code_size)?;
       
       Ok(())
   }

Compile-Time Feature Detection 🔍
---------------------------------

Detect security features at compile time:

.. code-block:: rust
   :caption: Compile-time security detection
   :linenos:

   use kiln_platform::hardware_optimizations::compile_time;
   
   fn check_compile_time_security() {
       // Detect security level at compile time
       const SECURITY_LEVEL: SecurityLevel = compile_time::detect_security_level();
       
       match SECURITY_LEVEL {
           SecurityLevel::Advanced => {
               println!("Advanced hardware security available at compile time");
           }
           SecurityLevel::Basic => {
               println!("Basic security features only");
           }
           _ => {}
       }
       
       // Check for advanced features
       if compile_time::has_advanced_security() {
           println!("Can use advanced security optimizations");
       }
   }

Side-Channel Resistance 🕵️
--------------------------

Kiln provides abstractions for side-channel resistant code:

.. code-block:: rust
   :caption: Side-channel resistance levels
   :linenos:

   use kiln_platform::side_channel_resistance::{
       ResistanceLevel,
       AttackVector
   };
   
   // Resistance levels available
   fn demonstrate_resistance_levels() {
       let levels = [
           ResistanceLevel::None,          // No protection
           ResistanceLevel::Basic,         // Basic timing protection
           ResistanceLevel::Enhanced,      // Cache and timing protection
           ResistanceLevel::Maximum,       // All known protections
       ];
       
       // Attack vectors we defend against
       let vectors = [
           AttackVector::Timing,           // Timing attacks
           AttackVector::Cache,            // Cache-based attacks
           AttackVector::Power,            // Power analysis
           AttackVector::Electromagnetic,  // EM emanations
           AttackVector::Speculative,      // Spectre-style attacks
       ];
   }

Constant-Time Operations
~~~~~~~~~~~~~~~~~~~~~~~

The constant_time module provides timing-safe operations:

.. code-block:: rust
   :caption: Constant-time utilities
   :linenos:

   use kiln_platform::side_channel_resistance::constant_time;
   
   fn use_constant_time_ops() -> Result<(), Error> {
       let secret_a = [1u8; 32];
       let secret_b = [2u8; 32];
       
       // Compare memory in constant time
       let equal = constant_time::compare_memory(
           secret_a.as_ptr(),
           secret_b.as_ptr(), 
           32
       );
       
       // The comparison takes the same time regardless of content
       assert!(!equal);
       
       Ok(())
   }

Cache-Aware Allocation
~~~~~~~~~~~~~~~~~~~~~

Minimize cache-based information leakage:

.. code-block:: rust
   :caption: Cache-aware memory management
   :linenos:

   use kiln_platform::side_channel_resistance::cache_aware_allocation;
   
   fn setup_cache_aware_memory() -> Result<(), Error> {
       // Allocate memory with cache-line alignment
       let size = 1024;
       let aligned_buffer = cache_aware_allocation::allocate_aligned(size)?;
       
       // The allocation is cache-line aligned to prevent false sharing
       // and information leakage through cache timing
       
       Ok(())
   }

Access Pattern Obfuscation  
~~~~~~~~~~~~~~~~~~~~~~~~~

Hide memory access patterns:

.. code-block:: rust
   :caption: Obfuscated access patterns
   :linenos:

   use kiln_platform::side_channel_resistance::access_obfuscation;
   
   fn obfuscate_memory_access() -> Result<(), Error> {
       let data = vec![1u32; 256];
       let index = 42; // Secret index
       
       // Access without revealing the index through timing
       let value = access_obfuscation::oblivious_select(&data, index)?;
       
       // All array elements are touched to hide which was selected
       assert_eq!(value, 1);
       
       Ok(())
   }

Formal Verification Support 🧮
-----------------------------

Kiln includes formal verification annotations:

.. code-block:: rust
   :caption: Formal verification integration
   :linenos:

   use kiln_platform::formal_verification::annotations::*;
   
   // Memory safety verification
   #[verified_memory_safe]
   fn allocate_verified(size: usize) -> Result<*mut u8, Error> {
       // This function has been verified to be memory safe
       // using bounded model checking
       platform_allocate(size)
   }
   
   // Constant-time verification
   #[constant_time]
   fn compare_secrets(a: &[u8], b: &[u8]) -> bool {
       // Verified to execute in constant time
       constant_time::compare_memory(a.as_ptr(), b.as_ptr(), a.len())
   }
   
   // Concurrency verification  
   #[data_race_free]
   #[deadlock_free]
   fn concurrent_operation() {
       // Verified free of data races and deadlocks
   }

Platform Integration
~~~~~~~~~~~~~~~~~~~

Integrate security features with platform abstractions:

.. code-block:: rust
   :caption: Security-aware platform configuration
   :linenos:

   use kiln_platform::side_channel_resistance::platform_integration;
   
   fn create_secure_platform() -> Result<(), Error> {
       // Configure platform with security features
       let config = platform_integration::configure_platform()
           .with_constant_time_operations()
           .with_cache_protection()
           .with_speculation_barriers()
           .build()?;
           
       // Platform automatically applies appropriate defenses
       let platform = platform_select::create_auto_platform();
       
       Ok(())
   }

Real-World Usage Examples 🌍
-------------------------

Complete Security Setup
~~~~~~~~~~~~~~~~~~~~~~

Combine multiple security features:

.. code-block:: rust
   :caption: Production security configuration
   :linenos:

   use kiln_platform::{
       hardware_optimizations::HardwareOptimizer,
       side_channel_resistance::ResistanceLevel,
       platform_abstraction::paradigm,
   };
   
   fn setup_production_security() -> Result<(), Error> {
       // Detect available hardware features
       let mut optimizer = HardwareOptimizer::<arch::Arm>::new();
       optimizer.detect_optimizations()?;
       
       println!("Security level: {:?}", optimizer.security_level());
       
       // Create security-first platform
       let config = PlatformConfig::<paradigm::SecurityFirst>::new()
           .with_max_pages(256)
           .with_isolation_level(IsolationLevel::Hardware);
           
       let platform = UnifiedPlatform::new(config);
       let allocator = platform.create_allocator()?;
       
       // Allocate memory with hardware protection
       let (ptr, size) = allocator.allocate(64, Some(256))?;
       
       // Apply available hardware protections
       #[cfg(target_arch = "aarch64")]
       {
           if BranchTargetIdentification::is_available() {
               let bti = BranchTargetIdentification::enable()?;
               bti.optimize_memory(ptr.as_ptr(), size)?;
           }
       }
       
       Ok(())
   }

Best Practices 📚
-----------------

1. **Check Availability First** - Always verify hardware support before use
2. **Use Compile-Time Detection** - Leverage `compile_time` module for zero-cost checks
3. **Layer Security** - Combine multiple protection mechanisms
4. **Test on Target Hardware** - Security features vary by CPU model
5. **Measure Performance Impact** - Security features have varying costs

Key Takeaways 🌟
---------------

**Zero-Cost Abstractions:**
   The platform layer compiles security checks down to direct hardware instructions with no runtime overhead.

**Graceful Degradation:**
   When hardware features aren't available, the code still runs safely with software fallbacks.

**Unified Interface:**
   Same API across ARM, Intel, and RISC-V architectures - the platform layer handles the differences.

**Formal Verification:**
   Security properties can be mathematically proven using the verification annotations.

.. admonition:: Security Philosophy
   :class: warning

   Hardware security features are powerful tools, but they're just one layer in a defense-in-depth strategy. The ``kiln-platform`` crate makes these features accessible while maintaining portability and performance.

Next Steps 🎯
-------------

- Check :doc:`platform_detection` to detect available security features
- Review :doc:`linux_features` for Linux-specific security options
- Explore :doc:`performance_optimizations` for security/performance trade-offs
- See :doc:`memory_management` for memory protection strategies