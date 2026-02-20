Adding Platform Support
======================

This guide explains how to add support for new platforms to Kiln, either by contributing to the core ``kiln-platform`` crate or by creating an external platform implementation.

Overview
--------

Kiln's Platform Abstraction Layer (PAL) provides a uniform interface for platform-specific operations through two core traits:

- **PageAllocator**: Memory allocation for WebAssembly pages (64KB aligned)
- **FutexLike**: Low-level synchronization primitives

All platform implementations must provide these traits to integrate with Kiln.

Core Traits
-----------

PageAllocator Trait
~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub trait PageAllocator {
       /// Allocate contiguous pages of memory
       fn allocate_pages(&mut self, pages: usize) -> Result<NonNull<u8>, Error>;
       
       /// Deallocate previously allocated pages
       fn deallocate_pages(&mut self, ptr: NonNull<u8>, pages: usize) -> Result<(), Error>;
       
       /// Grow an existing allocation
       fn grow_pages(&mut self, old_ptr: NonNull<u8>, old_pages: usize, new_pages: usize) 
           -> Result<NonNull<u8>, Error>;
       
       /// Get current allocated page count
       fn allocated_pages(&self) -> usize;
       
       /// Get maximum allowed pages
       fn max_pages(&self) -> usize;
   }

Key requirements:

- Pages must be 64KB (``WASM_PAGE_SIZE``) aligned
- Memory should be zeroed for security
- Track allocation counts for resource management
- Handle growth efficiently (realloc or allocate-copy-free)

FutexLike Trait
~~~~~~~~~~~~~~~

.. code-block:: rust

   pub trait FutexLike {
       /// Wait on a futex if value matches expected
       fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<(), Error>;
       
       /// Wake one waiter
       fn wake_one(&self) -> Result<u32, Error>;
       
       /// Wake all waiters
       fn wake_all(&self) -> Result<u32, Error>;
       
       /// Atomic load
       fn load(&self, ordering: Ordering) -> u32;
       
       /// Atomic store
       fn store(&self, value: u32, ordering: Ordering);
       
       /// Atomic compare-exchange
       fn compare_exchange_weak(&self, current: u32, new: u32, 
           success: Ordering, failure: Ordering) -> Result<u32, u32>;
   }

Implementation Options
---------------------

We recommend creating an external platform crate for most use cases. See :doc:`external_platform_crates` for a comprehensive guide.

Option 1: External Platform Crate (Recommended)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Create a separate crate that implements Kiln platform support. This approach:

- Allows you to support platforms not included in core Kiln
- Maintains your own release cycle and dependencies
- Keeps licensing and distribution under your control
- Enables experimentation without affecting core Kiln

See :doc:`external_platform_crates` for the complete guide.

Option 2: Contributing to kiln-platform
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Add your platform directly to the ``kiln-platform`` crate. This is appropriate for:

- Widely-used open-source platforms
- Platforms that should be included in Kiln's core offering
- Platforms with no special dependencies or licensing concerns

1. **Add platform feature to Cargo.toml**:

   .. code-block:: toml

      [features]
      platform-myos = []  # Your platform feature

2. **Create platform modules**:

   - ``src/myos_memory.rs`` - Memory allocator implementation
   - ``src/myos_sync.rs`` - Synchronization implementation
   - ``src/myos_threading.rs`` - Threading support (optional)

3. **Implement the allocator**:

   .. code-block:: rust

      // src/myos_memory.rs
      use crate::{PageAllocator, WASM_PAGE_SIZE};
      use core::ptr::NonNull;
      use kiln_error::{Error, ErrorKind};

      pub struct MyOsAllocator {
          max_pages: usize,
          allocated_pages: usize,
      }

      impl MyOsAllocator {
          pub fn new(max_pages: usize) -> Self {
              Self { max_pages, allocated_pages: 0 }
          }
      }

      impl PageAllocator for MyOsAllocator {
          fn allocate_pages(&mut self, pages: usize) -> Result<NonNull<u8>, Error> {
              if self.allocated_pages + pages > self.max_pages {
                  return Err(Error::new(ErrorKind::Memory, "Page limit exceeded"));
              }

              // Platform-specific allocation
              let size = pages * WASM_PAGE_SIZE;
              let ptr = unsafe { /* your_os_alloc(size, WASM_PAGE_SIZE) */ };
              
              if ptr.is_null() {
                  return Err(Error::new(ErrorKind::Memory, "Allocation failed"));
              }

              // Zero memory for security
              unsafe { core::ptr::write_bytes(ptr, 0, size) };
              
              self.allocated_pages += pages;
              NonNull::new(ptr).ok_or_else(|| 
                  Error::new(ErrorKind::Memory, "Null pointer"))
          }

          // ... implement other trait methods
      }

4. **Implement synchronization**:

   .. code-block:: rust

      // src/myos_sync.rs
      use crate::FutexLike;
      use core::sync::atomic::{AtomicU32, Ordering};
      use core::time::Duration;

      pub struct MyOsFutex {
          value: AtomicU32,
          // Platform-specific sync primitive
      }

      impl FutexLike for MyOsFutex {
          fn wait(&self, expected: u32, timeout: Option<Duration>) -> Result<(), Error> {
              if self.value.load(Ordering::Acquire) != expected {
                  return Ok(()); // Value changed, don't wait
              }
              
              // Platform-specific wait implementation
              Ok(())
          }
          
          // ... implement other trait methods
      }

5. **Add to lib.rs**:

   .. code-block:: rust

      // Platform-specific modules
      #[cfg(all(feature = "platform-myos", target_os = "myos"))]
      pub mod myos_memory;
      #[cfg(all(feature = "platform-myos", target_os = "myos"))]
      pub mod myos_sync;

      // Export types
      #[cfg(all(feature = "platform-myos", target_os = "myos"))]
      pub use myos_memory::{MyOsAllocator, MyOsAllocatorBuilder};
      #[cfg(all(feature = "platform-myos", target_os = "myos"))]
      pub use myos_sync::{MyOsFutex, MyOsFutexBuilder};

6. **Update platform_abstraction.rs** to include your platform in the compile-time dispatch.

For a complete external platform crate guide, see :doc:`external_platform_crates`.

Best Practices
--------------

Memory Allocation
~~~~~~~~~~~~~~~~~

1. **Alignment**: Always ensure WASM_PAGE_SIZE (64KB) alignment
2. **Zero Memory**: Clear allocated memory for security
3. **Error Handling**: Provide clear error messages
4. **Resource Tracking**: Accurately track allocated pages
5. **Growth Strategy**: Implement efficient memory growth

Synchronization
~~~~~~~~~~~~~~~

1. **Atomic Operations**: Use platform atomics correctly
2. **Spurious Wakeups**: Handle them properly in wait operations
3. **Timeout Handling**: Convert Duration to platform-specific format
4. **Wake Counts**: Return accurate wake counts

Platform Detection
~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   pub fn detect_platform_capabilities() -> PlatformCapabilities {
       PlatformCapabilities {
           has_mmu: cfg!(target_feature = "mmu"),
           page_size: 4096, // Platform page size
           max_memory: query_available_memory(),
           supports_atomics: true,
           // ... other capabilities
       }
   }

Testing Requirements
--------------------

Unit Tests
~~~~~~~~~~

Test each trait method:

.. code-block:: rust

   #[cfg(test)]
   mod tests {
       use super::*;

       #[test]
       fn test_allocate_deallocate() {
           let mut allocator = MyOsAllocator::new(10);
           
           let ptr = allocator.allocate_pages(2).unwrap();
           assert_eq!(allocator.allocated_pages(), 2);
           
           allocator.deallocate_pages(ptr, 2).unwrap();
           assert_eq!(allocator.allocated_pages(), 0);
       }

       #[test]
       fn test_page_limit() {
           let mut allocator = MyOsAllocator::new(5);
           assert!(allocator.allocate_pages(10).is_err());
       }
   }

Integration Tests
~~~~~~~~~~~~~~~~~

Test with Kiln components:

.. code-block:: rust

   #[test]
   fn test_with_kiln_memory() {
       let allocator = MyOsAllocator::new(100);
       let memory = Memory::new(allocator, 10, Some(50)).unwrap();
       
       assert_eq!(memory.size(), 10);
       memory.grow(5).unwrap();
       assert_eq!(memory.size(), 15);
   }

Platform-Specific Features
--------------------------

Guard Pages
~~~~~~~~~~~

If your platform supports guard pages:

.. code-block:: rust

   pub struct MyOsAllocatorBuilder {
       max_pages: usize,
       guard_pages: bool,
   }

   impl MyOsAllocatorBuilder {
       pub fn with_guard_pages(mut self, enable: bool) -> Self {
           self.guard_pages = enable;
           self
       }
   }

Memory Tagging
~~~~~~~~~~~~~~

For platforms with memory tagging (like ARM MTE):

.. code-block:: rust

   pub enum TagMode {
       Synchronous,
       Asynchronous,
       Asymmetric,
   }

   impl MyOsAllocator {
       pub fn set_tag_mode(&mut self, mode: TagMode) {
           // Platform-specific implementation
       }
   }

Example: VxWorks Implementation
-------------------------------

VxWorks demonstrates dual-context support (kernel and user space):

.. code-block:: rust

   pub enum VxWorksContext {
       Lkm,  // Loadable Kernel Module
       Rtp,  // Real-Time Process
   }

   pub struct VxWorksAllocator {
       context: VxWorksContext,
       // Context-specific fields
   }

   impl VxWorksAllocator {
       fn allocate_memory(&self, size: usize) -> Result<*mut u8, Error> {
           match self.context {
               VxWorksContext::Lkm => {
                   // Kernel allocation
                   unsafe { memPartAlloc(self.partition_id, size) }
               }
               VxWorksContext::Rtp => {
                   // User-space allocation
                   unsafe { malloc(size) }
               }
           }
       }
   }

Documentation Requirements
--------------------------

Your platform implementation should include:

1. **README.md**: Overview and usage examples
2. **API Documentation**: Rustdoc comments for all public items
3. **Platform Limitations**: Document any constraints
4. **Performance Characteristics**: Expected performance metrics
5. **Security Considerations**: Platform-specific security features

Submission Checklist
--------------------

Before submitting your platform implementation:

- [ ] Implements PageAllocator trait completely
- [ ] Implements FutexLike trait completely
- [ ] All tests pass on target platform
- [ ] Documentation is complete
- [ ] Code follows Kiln style guidelines
- [ ] No use of panic! or unwrap
- [ ] Proper error handling throughout
- [ ] Feature flags properly configured
- [ ] Compatible with no_std environments
- [ ] Security considerations documented

Getting Help
------------

- Review existing platform implementations in ``kiln-platform/src/``
- Check the platform layer tests in ``kiln-platform/tests/``
- Open an issue on GitHub for guidance
- Join the Kiln community discussions

Next Steps
----------

1. Choose between contributing to kiln-platform or creating an external crate
2. Implement the required traits for your platform
3. Add comprehensive tests
4. Document your implementation
5. Submit a pull request or publish your crate

For more details on the platform abstraction layer, see :doc:`/architecture/platform_layer`.