Creating External Platform Crates
=================================

This guide explains how to create your own external crate that implements Kiln platform support. This approach allows you to support platforms that aren't included in the core Kiln project, maintain your own release cycle, and keep platform-specific dependencies separate.

.. contents:: Table of Contents
   :local:
   :depth: 3

Why Create an External Platform Crate?
--------------------------------------

Creating an external platform crate is ideal when:

- You need support for a proprietary or specialized platform
- The platform has licensing incompatible with Kiln's MIT license
- You want to maintain your own release schedule
- The platform requires heavy dependencies Kiln doesn't want to include
- You're experimenting with new platform support
- You need platform-specific features beyond Kiln's scope

Creating Your Platform Crate
----------------------------

Step 1: Create the Crate Structure
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: bash

   cargo new kiln-platform-myos --lib
   cd kiln-platform-myos

Step 2: Set Up Dependencies
~~~~~~~~~~~~~~~~~~~~~~~~~~~

Edit ``Cargo.toml``:

.. code-block:: toml

   [package]
   name = "kiln-platform-myos"
   version = "0.1.0"
   edition = "2021"
   license = "MIT OR Apache-2.0"  # Choose your license
   description = "MyOS platform support for Kiln"
   
   [dependencies]
   # Core Kiln dependencies - use minimal features
   kiln-platform = { version = "0.2", default-features = false }
   kiln-error = { version = "0.2", default-features = false }
   
   # Your platform-specific dependencies
   # myos-sdk = "1.0"  # Example
   
   [features]
   default = ["std"]
   std = ["kiln-platform/std", "kiln-error/std"]
   alloc = ["kiln-platform/alloc", "kiln-error/alloc"]
   no_std = []
   
   # Your platform-specific features
   advanced-memory = []
   realtime = []
   
   [dev-dependencies]
   criterion = "0.5"

Step 3: Implement Core Traits
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Create ``src/lib.rs``:

.. code-block:: rust

   //! MyOS Platform Support for Kiln
   //! 
   //! This crate provides MyOS-specific implementations of Kiln's
   //! platform abstraction traits.
   
   #![cfg_attr(not(feature = "std"), no_std)]
   
   #[cfg(feature = "alloc")]
   extern crate alloc;
   
   // Re-export core Kiln traits for convenience
   pub use kiln_platform::{PageAllocator, FutexLike, WASM_PAGE_SIZE};
   
   mod allocator;
   mod sync;
   mod platform;
   
   pub use allocator::{MyOsAllocator, MyOsAllocatorBuilder};
   pub use sync::{MyOsFutex, MyOsFutexBuilder};
   pub use platform::{MyOsPlatform, MyOsConfig};
   
   /// Platform detection and initialization
   pub fn detect_platform() -> Result<MyOsPlatform, kiln_error::Error> {
       platform::MyOsPlatform::detect()
   }

Step 4: Implement Memory Allocator
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Create ``src/allocator.rs``:

.. code-block:: rust

   use core::ptr::NonNull;
   use kiln_platform::{PageAllocator, WASM_PAGE_SIZE};
   use kiln_error::{Error, ErrorKind};
   
   /// MyOS memory allocator
   pub struct MyOsAllocator {
       config: AllocatorConfig,
       allocated_pages: usize,
       // Platform-specific fields
       #[cfg(target_os = "myos")]
       heap_id: myos_sdk::HeapId,
   }
   
   #[derive(Clone, Debug)]
   pub struct AllocatorConfig {
       pub max_pages: usize,
       pub use_large_pages: bool,
       pub enable_protection: bool,
   }
   
   impl MyOsAllocator {
       fn new(config: AllocatorConfig) -> Result<Self, Error> {
           #[cfg(target_os = "myos")]
           {
               let heap_id = unsafe {
                   myos_sdk::heap_create(
                       config.max_pages * WASM_PAGE_SIZE,
                       if config.use_large_pages { 
                           myos_sdk::LARGE_PAGE_SIZE 
                       } else { 
                           WASM_PAGE_SIZE 
                       }
                   )
               };
               
               if heap_id == myos_sdk::INVALID_HEAP {
                   return Err(Error::new(
                       ErrorKind::Platform,
                       "Failed to create MyOS heap"
                   ));
               }
               
               Ok(Self {
                   config,
                   allocated_pages: 0,
                   heap_id,
               })
           }
           
           #[cfg(not(target_os = "myos"))]
           {
               // Fallback for development on other platforms
               Ok(Self {
                   config,
                   allocated_pages: 0,
               })
           }
       }
   }
   
   impl PageAllocator for MyOsAllocator {
       fn allocate_pages(&mut self, pages: usize) -> Result<NonNull<u8>, Error> {
           if self.allocated_pages + pages > self.config.max_pages {
               return Err(Error::new(
                   ErrorKind::Memory,
                   "Page limit exceeded"
               ));
           }
           
           let size = pages * WASM_PAGE_SIZE;
           
           #[cfg(target_os = "myos")]
           {
               let ptr = unsafe {
                   myos_sdk::heap_alloc_aligned(
                       self.heap_id,
                       size,
                       WASM_PAGE_SIZE
                   )
               };
               
               if ptr.is_null() {
                   return Err(Error::new(
                       ErrorKind::Memory,
                       "MyOS allocation failed"
                   ));
               }
               
               // Enable protection if requested
               if self.config.enable_protection {
                   unsafe {
                       myos_sdk::memory_protect(
                           ptr,
                           size,
                           myos_sdk::PROT_READ | myos_sdk::PROT_WRITE
                       );
                   }
               }
               
               // Zero memory for security
               unsafe { core::ptr::write_bytes(ptr, 0, size) };
               
               self.allocated_pages += pages;
               NonNull::new(ptr).ok_or_else(|| 
                   Error::new(ErrorKind::Memory, "Null pointer"))
           }
           
           #[cfg(not(target_os = "myos"))]
           {
               // Development fallback
               use core::alloc::{alloc, Layout};
               let layout = Layout::from_size_align(size, WASM_PAGE_SIZE)
                   .map_err(|_| Error::new(ErrorKind::Memory, "Invalid layout"))?;
               let ptr = unsafe { alloc(layout) };
               if ptr.is_null() {
                   return Err(Error::new(ErrorKind::Memory, "Allocation failed"));
               }
               unsafe { core::ptr::write_bytes(ptr, 0, size) };
               self.allocated_pages += pages;
               NonNull::new(ptr).ok_or_else(|| 
                   Error::new(ErrorKind::Memory, "Null pointer"))
           }
       }
       
       fn deallocate_pages(&mut self, ptr: NonNull<u8>, pages: usize) -> Result<(), Error> {
           #[cfg(target_os = "myos")]
           {
               unsafe {
                   myos_sdk::heap_free(self.heap_id, ptr.as_ptr());
               }
           }
           
           #[cfg(not(target_os = "myos"))]
           {
               use core::alloc::{dealloc, Layout};
               let layout = Layout::from_size_align(
                   pages * WASM_PAGE_SIZE, 
                   WASM_PAGE_SIZE
               ).map_err(|_| Error::new(ErrorKind::Memory, "Invalid layout"))?;
               unsafe { dealloc(ptr.as_ptr(), layout) };
           }
           
           self.allocated_pages = self.allocated_pages.saturating_sub(pages);
           Ok(())
       }
       
       fn grow_pages(&mut self, old_ptr: NonNull<u8>, old_pages: usize, new_pages: usize) 
           -> Result<NonNull<u8>, Error> {
           if new_pages <= old_pages {
               return Ok(old_ptr);
           }
           
           // Allocate new memory
           let new_ptr = self.allocate_pages(new_pages)?;
           
           // Copy old data
           unsafe {
               core::ptr::copy_nonoverlapping(
                   old_ptr.as_ptr(),
                   new_ptr.as_ptr(),
                   old_pages * WASM_PAGE_SIZE
               );
           }
           
           // Free old memory
           self.allocated_pages += old_pages; // Restore count before dealloc
           self.deallocate_pages(old_ptr, old_pages)?;
           
           Ok(new_ptr)
       }
       
       fn allocated_pages(&self) -> usize {
           self.allocated_pages
       }
       
       fn max_pages(&self) -> usize {
           self.config.max_pages
       }
   }
   
   /// Builder for MyOS allocator
   pub struct MyOsAllocatorBuilder {
       config: AllocatorConfig,
   }
   
   impl MyOsAllocatorBuilder {
       pub fn new() -> Self {
           Self {
               config: AllocatorConfig {
                   max_pages: 1024,
                   use_large_pages: false,
                   enable_protection: true,
               }
           }
       }
       
       pub fn max_pages(mut self, pages: usize) -> Self {
           self.config.max_pages = pages;
           self
       }
       
       pub fn use_large_pages(mut self, enable: bool) -> Self {
           self.config.use_large_pages = enable;
           self
       }
       
       pub fn enable_protection(mut self, enable: bool) -> Self {
           self.config.enable_protection = enable;
           self
       }
       
       pub fn build(self) -> Result<MyOsAllocator, Error> {
           MyOsAllocator::new(self.config)
       }
   }

Step 5: Implement Synchronization
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Create ``src/sync.rs``:

.. code-block:: rust

   use core::sync::atomic::{AtomicU32, Ordering};
   use core::time::Duration;
   use kiln_platform::FutexLike;
   use kiln_error::{Error, ErrorKind};
   
   /// MyOS futex implementation
   pub struct MyOsFutex {
       value: AtomicU32,
       #[cfg(target_os = "myos")]
       sem_handle: myos_sdk::Semaphore,
   }
   
   impl MyOsFutex {
       pub fn new(initial: u32) -> Result<Self, Error> {
           #[cfg(target_os = "myos")]
           {
               let sem_handle = unsafe {
                   myos_sdk::sem_create(0, myos_sdk::SEM_BINARY)
               };
               
               if sem_handle == myos_sdk::INVALID_SEM {
                   return Err(Error::new(
                       ErrorKind::Platform,
                       "Failed to create MyOS semaphore"
                   ));
               }
               
               Ok(Self {
                   value: AtomicU32::new(initial),
                   sem_handle,
               })
           }
           
           #[cfg(not(target_os = "myos"))]
           {
               Ok(Self {
                   value: AtomicU32::new(initial),
               })
           }
       }
   }
   
   impl FutexLike for MyOsFutex {
       fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<(), Error> {
           if self.value.load(Ordering::Acquire) != expected {
               return Ok(());
           }
           
           #[cfg(target_os = "myos")]
           {
               let timeout_ms = timeout
                   .map(|d| d.as_millis() as u32)
                   .unwrap_or(myos_sdk::WAIT_FOREVER);
               
               let result = unsafe {
                   myos_sdk::sem_wait(self.sem_handle, timeout_ms)
               };
               
               if result != myos_sdk::OK {
                   return Err(Error::new(
                       ErrorKind::Platform,
                       "MyOS semaphore wait failed"
                   ));
               }
           }
           
           Ok(())
       }
       
       fn wake_one(&self) -> Result<u32, Error> {
           #[cfg(target_os = "myos")]
           {
               unsafe { myos_sdk::sem_signal(self.sem_handle) };
           }
           Ok(1)
       }
       
       fn wake_all(&self) -> Result<u32, Error> {
           #[cfg(target_os = "myos")]
           {
               unsafe { myos_sdk::sem_broadcast(self.sem_handle) };
           }
           Ok(u32::MAX)
       }
       
       fn load(&self, ordering: Ordering) -> u32 {
           self.value.load(ordering)
       }
       
       fn store(&self, value: u32, ordering: Ordering) {
           self.value.store(value, ordering);
       }
       
       fn compare_exchange_weak(
           &self,
           current: u32,
           new: u32,
           success: Ordering,
           failure: Ordering,
       ) -> Result<u32, u32> {
           self.value.compare_exchange_weak(current, new, success, failure)
       }
   }
   
   impl Drop for MyOsFutex {
       fn drop(&mut self) {
           #[cfg(target_os = "myos")]
           {
               unsafe {
                   myos_sdk::sem_destroy(self.sem_handle);
               }
           }
       }
   }
   
   /// Builder for MyOS futex
   pub struct MyOsFutexBuilder {
       initial_value: u32,
   }
   
   impl MyOsFutexBuilder {
       pub fn new() -> Self {
           Self { initial_value: 0 }
       }
       
       pub fn initial_value(mut self, value: u32) -> Self {
           self.initial_value = value;
           self
       }
       
       pub fn build(self) -> Result<MyOsFutex, Error> {
           MyOsFutex::new(self.initial_value)
       }
   }

Step 6: Create High-Level Platform Interface
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Create ``src/platform.rs``:

.. code-block:: rust

   use crate::{MyOsAllocator, MyOsAllocatorBuilder, MyOsFutex, MyOsFutexBuilder};
   use kiln_platform::{PageAllocator, FutexLike};
   use kiln_error::Error;
   
   /// MyOS platform configuration
   #[derive(Clone, Debug)]
   pub struct MyOsConfig {
       pub max_memory_pages: usize,
       pub enable_large_pages: bool,
       pub enable_memory_protection: bool,
       pub thread_stack_size: usize,
   }
   
   impl Default for MyOsConfig {
       fn default() -> Self {
           Self {
               max_memory_pages: 1024,
               enable_large_pages: false,
               enable_memory_protection: true,
               thread_stack_size: 64 * 1024, // 64KB
           }
       }
   }
   
   /// High-level MyOS platform adapter
   pub struct MyOsPlatform {
       config: MyOsConfig,
       capabilities: PlatformCapabilities,
   }
   
   #[derive(Debug, Clone)]
   pub struct PlatformCapabilities {
       pub os_version: String,
       pub cpu_cores: usize,
       pub total_memory: usize,
       pub page_sizes: Vec<usize>,
       pub has_memory_protection: bool,
       pub has_large_page_support: bool,
       pub max_threads: usize,
   }
   
   impl MyOsPlatform {
       /// Create platform with configuration
       pub fn new(config: MyOsConfig) -> Self {
           let capabilities = Self::detect_capabilities();
           Self { config, capabilities }
       }
       
       /// Detect platform capabilities
       pub fn detect() -> Result<Self, Error> {
           let config = MyOsConfig::default();
           Ok(Self::new(config))
       }
       
       /// Get platform capabilities
       pub fn capabilities(&self) -> &PlatformCapabilities {
           &self.capabilities
       }
       
       /// Create platform-specific allocator
       pub fn create_allocator(&self) -> Result<impl PageAllocator, Error> {
           MyOsAllocatorBuilder::new()
               .max_pages(self.config.max_memory_pages)
               .use_large_pages(self.config.enable_large_pages)
               .enable_protection(self.config.enable_memory_protection)
               .build()
       }
       
       /// Create platform-specific futex
       pub fn create_futex(&self) -> Result<impl FutexLike, Error> {
           MyOsFutexBuilder::new().build()
       }
       
       /// Create allocator as trait object
       pub fn create_allocator_boxed(&self) -> Result<Box<dyn PageAllocator>, Error> {
           Ok(Box::new(self.create_allocator()?))
       }
       
       /// Create futex as trait object
       pub fn create_futex_boxed(&self) -> Result<Box<dyn FutexLike>, Error> {
           Ok(Box::new(self.create_futex()?))
       }
       
       fn detect_capabilities() -> PlatformCapabilities {
           #[cfg(target_os = "myos")]
           {
               // Query actual platform capabilities
               PlatformCapabilities {
                   os_version: unsafe { 
                       myos_sdk::get_version_string() 
                   },
                   cpu_cores: unsafe { 
                       myos_sdk::get_cpu_count() 
                   },
                   total_memory: unsafe { 
                       myos_sdk::get_total_memory() 
                   },
                   page_sizes: vec![4096, 2 * 1024 * 1024], // 4KB, 2MB
                   has_memory_protection: true,
                   has_large_page_support: true,
                   max_threads: 1024,
               }
           }
           
           #[cfg(not(target_os = "myos"))]
           {
               // Development fallback
               PlatformCapabilities {
                   os_version: "MyOS Dev 1.0".to_string(),
                   cpu_cores: 4,
                   total_memory: 8 * 1024 * 1024 * 1024, // 8GB
                   page_sizes: vec![4096],
                   has_memory_protection: false,
                   has_large_page_support: false,
                   max_threads: 256,
               }
           }
       }
   }
   
   /// Platform capability queries
   impl MyOsPlatform {
       pub fn is_real_platform(&self) -> bool {
           cfg!(target_os = "myos")
       }
       
       pub fn recommended_memory_pages(&self) -> usize {
           // Use 25% of available memory for WASM
           let wasm_memory = self.capabilities.total_memory / 4;
           wasm_memory / kiln_platform::WASM_PAGE_SIZE
       }
       
       pub fn supports_large_pages(&self) -> bool {
           self.capabilities.has_large_page_support
       }
   }

Using Your Platform Crate
-------------------------

In Application Cargo.toml
~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: toml

   [dependencies]
   kiln = "0.2"
   kiln-platform-myos = "0.1"

Basic Usage Example
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_platform_myos::{MyOsPlatform, MyOsConfig};
   
   fn main() -> Result<(), Box<dyn std::error::Error>> {
       // Detect and configure platform
       let platform = MyOsPlatform::detect()?;
       println!("Running on: {:?}", platform.capabilities());
       
       // Create Kiln components with MyOS platform
       let allocator = platform.create_allocator_boxed()?;
       let futex = platform.create_futex_boxed()?;
       
       // Use with Kiln runtime
       let runtime = kiln::Runtime::builder()
           .with_allocator(allocator)
           .with_futex(futex)
           .build()?;
       
       Ok(())
   }

Advanced Integration Example
~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_platform_myos::MyOsPlatform;
   use kiln_platform::{PageAllocator, FutexLike};
   
   /// Generic function that works with any platform
   fn create_wasm_runtime<A, F>(
       allocator: A,
       futex: F,
   ) -> Result<WasmRuntime<A, F>, Error>
   where
       A: PageAllocator,
       F: FutexLike,
   {
       WasmRuntime::new(allocator, futex)
   }
   
   fn main() -> Result<(), Box<dyn std::error::Error>> {
       let platform = MyOsPlatform::detect()?;
       
       // Create concrete types (avoids boxing overhead)
       let allocator = platform.create_allocator()?;
       let futex = platform.create_futex()?;
       
       // Use with generic runtime
       let runtime = create_wasm_runtime(allocator, futex)?;
       
       Ok(())
   }

Testing Your Platform Crate
---------------------------

Unit Tests
~~~~~~~~~~

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;
       use kiln_platform::{PageAllocator, FutexLike};
       
       #[test]
       fn test_allocator_basic() {
           let platform = MyOsPlatform::detect().unwrap();
           let mut allocator = platform.create_allocator().unwrap();
           
           // Test allocation
           let ptr = allocator.allocate_pages(10).unwrap();
           assert_eq!(allocator.allocated_pages(), 10);
           
           // Test deallocation
           allocator.deallocate_pages(ptr, 10).unwrap();
           assert_eq!(allocator.allocated_pages(), 0);
       }
       
       #[test]
       fn test_futex_operations() {
           let platform = MyOsPlatform::detect().unwrap();
           let futex = platform.create_futex().unwrap();
           
           futex.store(42, core::sync::atomic::Ordering::SeqCst);
           assert_eq!(futex.load(core::sync::atomic::Ordering::SeqCst), 42);
       }
   }

Integration Tests
~~~~~~~~~~~~~~~~~

Create ``tests/integration.rs``:

.. code-block:: rust

   use kiln_platform_myos::MyOsPlatform;
   use kiln_platform::{PageAllocator, FutexLike};
   
   #[test]
   fn test_with_kiln_traits() {
       fn generic_test<A: PageAllocator, F: FutexLike>(
           mut allocator: A,
           futex: F,
       ) {
           // This ensures our types work with Kiln's trait bounds
           let pages = allocator.allocate_pages(5).unwrap();
           futex.store(100, core::sync::atomic::Ordering::Relaxed);
           allocator.deallocate_pages(pages, 5).unwrap();
       }
       
       let platform = MyOsPlatform::detect().unwrap();
       let allocator = platform.create_allocator().unwrap();
       let futex = platform.create_futex().unwrap();
       
       generic_test(allocator, futex);
   }

Publishing Your Crate
---------------------

1. **Documentation**: Add comprehensive docs with examples
2. **CI/CD**: Set up GitHub Actions for your target platform
3. **Versioning**: Follow semantic versioning
4. **Examples**: Include runnable examples in ``examples/``
5. **Benchmarks**: Add performance benchmarks
6. **Platform Detection**: Document how to detect if running on your platform

Best Practices
--------------

Design Considerations
~~~~~~~~~~~~~~~~~~~~~

1. **Fallback Implementations**: Provide fallbacks for development on other platforms
2. **Feature Flags**: Use features for optional functionality
3. **Error Handling**: Use ``kiln_error::Error`` for consistency
4. **Zero-Cost Abstractions**: Minimize runtime overhead
5. **No Unwrap**: Never panic in production code

Platform Detection
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   /// Check if we're running on the target platform
   pub fn is_myos_available() -> bool {
       cfg!(target_os = "myos") && check_runtime_availability()
   }
   
   fn check_runtime_availability() -> bool {
       #[cfg(target_os = "myos")]
       {
           // Try to call a MyOS-specific function
           unsafe { myos_sdk::get_version() != 0 }
       }
       
       #[cfg(not(target_os = "myos"))]
       {
           false
       }
   }

Configuration Pattern
~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   /// Allow users to configure platform behavior
   pub struct MyOsPlatformBuilder {
       config: MyOsConfig,
   }
   
   impl MyOsPlatformBuilder {
       pub fn new() -> Self {
           Self {
               config: MyOsConfig::default(),
           }
       }
       
       pub fn memory_pages(mut self, pages: usize) -> Self {
           self.config.max_memory_pages = pages;
           self
       }
       
       pub fn detect_capabilities(mut self) -> Self {
           // Auto-detect optimal settings
           let caps = MyOsPlatform::detect_capabilities();
           self.config.max_memory_pages = 
               caps.total_memory / kiln_platform::WASM_PAGE_SIZE / 4;
           self
       }
       
       pub fn build(self) -> MyOsPlatform {
           MyOsPlatform::new(self.config)
       }
   }

Real-World Example: Supporting Multiple Platforms
-------------------------------------------------

Your crate can support multiple related platforms:

.. code-block:: rust

   pub enum PlatformVariant {
       MyOsDesktop,
       MyOsEmbedded,
       MyOsMobile,
   }
   
   impl MyOsPlatform {
       pub fn detect_variant() -> PlatformVariant {
           #[cfg(target_os = "myos")]
           {
               match unsafe { myos_sdk::get_platform_type() } {
                   myos_sdk::PLATFORM_DESKTOP => PlatformVariant::MyOsDesktop,
                   myos_sdk::PLATFORM_EMBEDDED => PlatformVariant::MyOsEmbedded,
                   myos_sdk::PLATFORM_MOBILE => PlatformVariant::MyOsMobile,
                   _ => PlatformVariant::MyOsDesktop,
               }
           }
           
           #[cfg(not(target_os = "myos"))]
           {
               PlatformVariant::MyOsDesktop
           }
       }
       
       pub fn create_optimized_allocator(&self) -> Result<impl PageAllocator, Error> {
           match Self::detect_variant() {
               PlatformVariant::MyOsDesktop => {
                   // Use large pages on desktop
                   MyOsAllocatorBuilder::new()
                       .use_large_pages(true)
                       .max_pages(4096)
                       .build()
               }
               PlatformVariant::MyOsEmbedded => {
                   // Conservative settings for embedded
                   MyOsAllocatorBuilder::new()
                       .use_large_pages(false)
                       .max_pages(256)
                       .enable_protection(false)
                       .build()
               }
               PlatformVariant::MyOsMobile => {
                   // Balanced settings for mobile
                   MyOsAllocatorBuilder::new()
                       .max_pages(1024)
                       .build()
               }
           }
       }
   }

Getting Help
------------

- Review ``kiln-platform`` source for trait definitions
- Look at existing platforms in ``kiln-platform/src/`` for patterns
- Check the ``kiln-platform/examples/`` directory
- Open issues on the Kiln repository for questions
- Join community discussions about platform support

Your external platform crate can provide first-class support for any platform while maintaining complete independence from the core Kiln project!