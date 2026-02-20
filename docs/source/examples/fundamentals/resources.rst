====================================
Resource Management: Handles with Care
====================================

.. epigraph::

   "Every system is a resource management system in disguise."
   
   -- Systems Programming Wisdom

Resources are the bridge between the abstract world of computation and the concrete reality of system objects. Files, sockets, graphics contexts, database connections - they all need careful lifecycle management. Kiln's resource system makes this both safe and efficient.

.. admonition:: What You'll Learn
   :class: note

   - Understanding resource handles and lifetimes
   - Building resource tables for safe access
   - Implementing custom resource types
   - Cross-component resource sharing
   - Debugging resource leaks

The Resource Challenge 🎯
------------------------

Traditional resource management is fraught with pitfalls:

.. code-block:: rust
   :caption: The dangers we're avoiding

   // Traditional (dangerous) approach
   struct UnsafeResourceManager {
       files: Vec<*mut File>,  // Raw pointers - use after free?
       sockets: Vec<usize>,    // Just IDs - how do we validate?
   }
   
   impl UnsafeResourceManager {
       fn get_file(&self, id: usize) -> Option<&mut File> {
           if id < self.files.len() {
               unsafe { 
                   // This could crash if the file was already closed!
                   Some(&mut *self.files[id])
               }
           } else {
               None
           }
       }
   }

Kiln's resource system eliminates these issues:

.. code-block:: rust
   :caption: The safe Kiln way
   :linenos:

   use kiln_foundation::resource::{ResourceTable, ResourceHandle, Resource};
   use std::fs::File;
   
   // Define a custom resource type
   #[derive(Debug)]
   struct FileResource {
       file: File,
       path: String,
       permissions: FilePermissions,
   }
   
   #[derive(Debug, Clone)]
   enum FilePermissions {
       ReadOnly,
       WriteOnly,
       ReadWrite,
   }
   
   impl Resource for FileResource {
       type Error = std::io::Error;
       
       fn cleanup(&mut self) -> Result<(), Self::Error> {
           // Ensure file is properly closed
           self.file.sync_all()?;
           println!("File {} properly closed", self.path);
           Ok(())
       }
   }
   
   fn safe_file_management() {
       let mut file_table: ResourceTable<FileResource> = ResourceTable::new();
       
       // Create and store a file resource
       let file = File::create("test.txt").unwrap();
       let file_resource = FileResource {
           file,
           path: "test.txt".to_string(),
           permissions: FilePermissions::ReadWrite,
       };
       
       let handle = file_table.insert(file_resource);
       
       // Use the resource safely
       if let Some(file_res) = file_table.get_mut(handle) {
           // Work with the file
           println!("Working with file: {}", file_res.path);
       }
       
       // Cleanup is automatic when table is dropped
   }

Building a Graphics Resource Manager 🎨
---------------------------------------

Let's create a comprehensive example for managing graphics resources:

.. code-block:: rust
   :caption: Graphics resource management system
   :linenos:

   use kiln_foundation::resource::{ResourceTable, ResourceHandle, Resource};
   use kiln_foundation::bounded::BoundedVec;
   use std::collections::HashMap;
   
   // Different types of graphics resources
   #[derive(Debug)]
   enum GraphicsResource {
       Texture(TextureResource),
       Buffer(BufferResource),
       Shader(ShaderResource),
       RenderTarget(RenderTargetResource),
   }
   
   #[derive(Debug)]
   struct TextureResource {
       id: u32,
       width: u32,
       height: u32,
       format: TextureFormat,
       data: Vec<u8>,
   }
   
   #[derive(Debug)]
   struct BufferResource {
       id: u32,
       size: usize,
       buffer_type: BufferType,
       data: Vec<u8>,
   }
   
   #[derive(Debug)]
   struct ShaderResource {
       id: u32,
       source: String,
       shader_type: ShaderType,
       compiled: bool,
   }
   
   #[derive(Debug)]
   struct RenderTargetResource {
       id: u32,
       width: u32,
       height: u32,
       color_attachments: Vec<ResourceHandle<GraphicsResource>>,
   }
   
   #[derive(Debug, Clone)]
   enum TextureFormat { RGBA8, RGB8, DEPTH24 }
   
   #[derive(Debug, Clone)]
   enum BufferType { Vertex, Index, Uniform }
   
   #[derive(Debug, Clone)]
   enum ShaderType { Vertex, Fragment, Compute }
   
   impl Resource for GraphicsResource {
       type Error = GraphicsError;
       
       fn cleanup(&mut self) -> Result<(), Self::Error> {
           match self {
               GraphicsResource::Texture(tex) => {
                   println!("Releasing texture {} ({}x{})", tex.id, tex.width, tex.height);
                   // In a real implementation, you'd call OpenGL/Vulkan cleanup
               }
               GraphicsResource::Buffer(buf) => {
                   println!("Releasing buffer {} ({} bytes)", buf.id, buf.size);
               }
               GraphicsResource::Shader(shader) => {
                   println!("Releasing shader {} ({})", shader.id, shader.source.len());
               }
               GraphicsResource::RenderTarget(rt) => {
                   println!("Releasing render target {} ({}x{})", rt.id, rt.width, rt.height);
               }
           }
           Ok(())
       }
   }
   
   #[derive(Debug)]
   enum GraphicsError {
       InvalidResource,
       AllocationFailed,
       CompilationFailed(String),
   }
   
   struct GraphicsManager {
       resources: ResourceTable<GraphicsResource>,
       texture_handles: HashMap<String, ResourceHandle<GraphicsResource>>,
       next_id: u32,
   }
   
   impl GraphicsManager {
       fn new() -> Self {
           Self {
               resources: ResourceTable::with_capacity(1000),
               texture_handles: HashMap::new(),
               next_id: 1,
           }
       }
       
       fn create_texture(
           &mut self,
           name: &str,
           width: u32,
           height: u32,
           format: TextureFormat,
           data: Vec<u8>
       ) -> Result<ResourceHandle<GraphicsResource>, GraphicsError> {
           let texture = TextureResource {
               id: self.next_id,
               width,
               height,
               format,
               data,
           };
           
           self.next_id += 1;
           
           let handle = self.resources.insert(GraphicsResource::Texture(texture));
           self.texture_handles.insert(name.to_string(), handle);
           
           Ok(handle)
       }
       
       fn create_buffer(
           &mut self,
           buffer_type: BufferType,
           data: Vec<u8>
       ) -> Result<ResourceHandle<GraphicsResource>, GraphicsError> {
           let buffer = BufferResource {
               id: self.next_id,
               size: data.len(),
               buffer_type,
               data,
           };
           
           self.next_id += 1;
           
           Ok(self.resources.insert(GraphicsResource::Buffer(buffer)))
       }
       
       fn create_shader(
           &mut self,
           shader_type: ShaderType,
           source: String
       ) -> Result<ResourceHandle<GraphicsResource>, GraphicsError> {
           let shader = ShaderResource {
               id: self.next_id,
               source,
               shader_type,
               compiled: false,
           };
           
           self.next_id += 1;
           
           Ok(self.resources.insert(GraphicsResource::Shader(shader)))
       }
       
       fn compile_shader(
           &mut self,
           handle: ResourceHandle<GraphicsResource>
       ) -> Result<(), GraphicsError> {
           if let Some(GraphicsResource::Shader(ref mut shader)) = self.resources.get_mut(handle) {
               // Simulate shader compilation
               if shader.source.contains("error") {
                   return Err(GraphicsError::CompilationFailed("Syntax error".to_string()));
               }
               
               shader.compiled = true;
               println!("Shader {} compiled successfully", shader.id);
               Ok(())
           } else {
               Err(GraphicsError::InvalidResource)
           }
       }
       
       fn create_render_target(
           &mut self,
           width: u32,
           height: u32,
           color_attachments: Vec<ResourceHandle<GraphicsResource>>
       ) -> Result<ResourceHandle<GraphicsResource>, GraphicsError> {
           // Validate that all attachments are textures
           for &attachment_handle in &color_attachments {
               match self.resources.get(attachment_handle) {
                   Some(GraphicsResource::Texture(_)) => {} // Valid
                   _ => return Err(GraphicsError::InvalidResource),
               }
           }
           
           let render_target = RenderTargetResource {
               id: self.next_id,
               width,
               height,
               color_attachments,
           };
           
           self.next_id += 1;
           
           Ok(self.resources.insert(GraphicsResource::RenderTarget(render_target)))
       }
       
       fn get_texture_by_name(&self, name: &str) -> Option<&TextureResource> {
           if let Some(&handle) = self.texture_handles.get(name) {
               if let Some(GraphicsResource::Texture(ref texture)) = self.resources.get(handle) {
                   return Some(texture);
               }
           }
           None
       }
       
       fn print_resource_stats(&self) {
           let mut texture_count = 0;
           let mut buffer_count = 0;
           let mut shader_count = 0;
           let mut render_target_count = 0;
           
           for resource in self.resources.iter() {
               match resource {
                   GraphicsResource::Texture(_) => texture_count += 1,
                   GraphicsResource::Buffer(_) => buffer_count += 1,
                   GraphicsResource::Shader(_) => shader_count += 1,
                   GraphicsResource::RenderTarget(_) => render_target_count += 1,
               }
           }
           
           println!("Graphics Resources:");
           println!("  Textures: {}", texture_count);
           println!("  Buffers: {}", buffer_count);
           println!("  Shaders: {}", shader_count);
           println!("  Render Targets: {}", render_target_count);
           println!("  Total: {}", self.resources.len());
       }
   }

Cross-Component Resource Sharing 🤝
-----------------------------------

Resources can be safely shared between components:

.. code-block:: rust
   :caption: Shared resource system
   :linenos:

   use kiln_foundation::resource::{SharedResourceTable, WeakResourceHandle};
   use kiln_sync::WrtMutex;
   use std::sync::Arc;
   
   #[derive(Debug)]
   struct DatabaseConnection {
       id: u32,
       connection_string: String,
       active: bool,
   }
   
   impl Resource for DatabaseConnection {
       type Error = DatabaseError;
       
       fn cleanup(&mut self) -> Result<(), Self::Error> {
           self.active = false;
           println!("Database connection {} closed", self.id);
           Ok(())
       }
   }
   
   #[derive(Debug)]
   enum DatabaseError {
       ConnectionFailed,
       QueryFailed,
   }
   
   struct SharedDatabasePool {
       pool: Arc<WrtMutex<SharedResourceTable<DatabaseConnection>>>,
       next_id: u32,
   }
   
   impl SharedDatabasePool {
       fn new() -> Self {
           Self {
               pool: Arc::new(WrtMutex::new(SharedResourceTable::new())),
               next_id: 1,
           }
       }
       
       fn create_connection(&mut self, connection_string: String) -> ResourceHandle<DatabaseConnection> {
           let connection = DatabaseConnection {
               id: self.next_id,
               connection_string,
               active: true,
           };
           
           self.next_id += 1;
           
           let mut pool = self.pool.lock().unwrap();
           pool.insert(connection)
       }
       
       fn get_connection(&self, handle: ResourceHandle<DatabaseConnection>) -> Option<Arc<DatabaseConnection>> {
           let pool = self.pool.lock().unwrap();
           pool.get_shared(handle)
       }
       
       fn create_weak_reference(&self, handle: ResourceHandle<DatabaseConnection>) -> WeakResourceHandle<DatabaseConnection> {
           let pool = self.pool.lock().unwrap();
           pool.create_weak(handle)
       }
       
       fn close_connection(&self, handle: ResourceHandle<DatabaseConnection>) {
           let mut pool = self.pool.lock().unwrap();
           pool.remove(handle);
       }
   }
   
   // Component A can get a strong reference
   struct ComponentA {
       db_connection: Option<Arc<DatabaseConnection>>,
   }
   
   impl ComponentA {
       fn use_database(&mut self, pool: &SharedDatabasePool, handle: ResourceHandle<DatabaseConnection>) {
           self.db_connection = pool.get_connection(handle);
           
           if let Some(ref conn) = self.db_connection {
               println!("Component A using database connection {}", conn.id);
           }
       }
   }
   
   // Component B can get a weak reference (won't prevent cleanup)
   struct ComponentB {
       db_connection: Option<WeakResourceHandle<DatabaseConnection>>,
   }
   
   impl ComponentB {
       fn monitor_database(&mut self, pool: &SharedDatabasePool, handle: ResourceHandle<DatabaseConnection>) {
           self.db_connection = Some(pool.create_weak_reference(handle));
           
           println!("Component B monitoring database connection");
       }
       
       fn check_connection(&self, pool: &SharedDatabasePool) -> bool {
           if let Some(ref weak_handle) = self.db_connection {
               weak_handle.upgrade(pool).is_some()
           } else {
               false
           }
       }
   }

Resource Lifecycle Hooks 🔄
---------------------------

Advanced resource management with lifecycle callbacks:

.. code-block:: rust
   :caption: Resource lifecycle management
   :linenos:

   use kiln_foundation::resource::{ResourceTable, ResourceHandle, LifecycleCallback};
   
   #[derive(Debug)]
   struct ManagedResource {
       id: u32,
       data: String,
       access_count: u32,
   }
   
   impl Resource for ManagedResource {
       type Error = ();
       
       fn cleanup(&mut self) -> Result<(), Self::Error> {
           println!("Cleaning up resource {} (accessed {} times)", self.id, self.access_count);
           Ok(())
       }
   }
   
   struct ResourceTracker {
       resources: ResourceTable<ManagedResource>,
       creation_callback: Option<Box<dyn Fn(ResourceHandle<ManagedResource>, &ManagedResource)>>,
       access_callback: Option<Box<dyn Fn(ResourceHandle<ManagedResource>, &mut ManagedResource)>>,
       cleanup_callback: Option<Box<dyn Fn(ResourceHandle<ManagedResource>)>>,
   }
   
   impl ResourceTracker {
       fn new() -> Self {
           Self {
               resources: ResourceTable::new(),
               creation_callback: None,
               access_callback: None,
               cleanup_callback: None,
           }
       }
       
       fn set_creation_callback<F>(&mut self, callback: F)
       where
           F: Fn(ResourceHandle<ManagedResource>, &ManagedResource) + 'static,
       {
           self.creation_callback = Some(Box::new(callback));
       }
       
       fn set_access_callback<F>(&mut self, callback: F)
       where
           F: Fn(ResourceHandle<ManagedResource>, &mut ManagedResource) + 'static,
       {
           self.access_callback = Some(Box::new(callback));
       }
       
       fn set_cleanup_callback<F>(&mut self, callback: F)
       where
           F: Fn(ResourceHandle<ManagedResource>) + 'static,
       {
           self.cleanup_callback = Some(Box::new(callback));
       }
       
       fn create_resource(&mut self, id: u32, data: String) -> ResourceHandle<ManagedResource> {
           let resource = ManagedResource {
               id,
               data,
               access_count: 0,
           };
           
           let handle = self.resources.insert(resource);
           
           // Trigger creation callback
           if let (Some(ref callback), Some(resource)) = (&self.creation_callback, self.resources.get(handle)) {
               callback(handle, resource);
           }
           
           handle
       }
       
       fn access_resource(&mut self, handle: ResourceHandle<ManagedResource>) -> Option<&ManagedResource> {
           if let Some(resource) = self.resources.get_mut(handle) {
               resource.access_count += 1;
               
               // Trigger access callback
               if let Some(ref callback) = self.access_callback {
                   callback(handle, resource);
               }
               
               // Return immutable reference
               self.resources.get(handle)
           } else {
               None
           }
       }
       
       fn remove_resource(&mut self, handle: ResourceHandle<ManagedResource>) {
           // Trigger cleanup callback before removal
           if let Some(ref callback) = self.cleanup_callback {
               callback(handle);
           }
           
           self.resources.remove(handle);
       }
   }
   
   fn lifecycle_example() {
       let mut tracker = ResourceTracker::new();
       
       // Set up callbacks
       tracker.set_creation_callback(|handle, resource| {
           println!("Created resource {} with handle {:?}", resource.id, handle);
       });
       
       tracker.set_access_callback(|handle, resource| {
           println!("Accessed resource {} (count: {})", resource.id, resource.access_count);
           
           // Log frequently accessed resources
           if resource.access_count % 10 == 0 {
               println!("  Resource {} is heavily used!", resource.id);
           }
       });
       
       tracker.set_cleanup_callback(|handle| {
           println!("Removing resource with handle {:?}", handle);
       });
       
       // Use the resource tracker
       let handle = tracker.create_resource(1, "Important data".to_string());
       
       // Access the resource multiple times
       for _ in 0..15 {
           tracker.access_resource(handle);
       }
       
       // Clean up
       tracker.remove_resource(handle);
   }

Resource Debugging and Monitoring 🔍
------------------------------------

Tools for tracking resource usage and detecting leaks:

.. code-block:: rust
   :caption: Resource monitoring system

   use std::collections::HashMap;
   use std::time::{Instant, Duration};
   
   #[derive(Debug, Clone)]
   struct ResourceMetrics {
       created_at: Instant,
       last_accessed: Instant,
       access_count: u32,
       size_bytes: usize,
   }
   
   struct ResourceMonitor<R: Resource> {
       table: ResourceTable<R>,
       metrics: HashMap<ResourceHandle<R>, ResourceMetrics>,
       total_created: u64,
       total_destroyed: u64,
   }
   
   impl<R: Resource> ResourceMonitor<R> {
       fn new() -> Self {
           Self {
               table: ResourceTable::new(),
               metrics: HashMap::new(),
               total_created: 0,
               total_destroyed: 0,
           }
       }
       
       fn insert(&mut self, resource: R) -> ResourceHandle<R> {
           let handle = self.table.insert(resource);
           
           let metrics = ResourceMetrics {
               created_at: Instant::now(),
               last_accessed: Instant::now(),
               access_count: 0,
               size_bytes: std::mem::size_of::<R>(),
           };
           
           self.metrics.insert(handle, metrics);
           self.total_created += 1;
           
           handle
       }
       
       fn get(&mut self, handle: ResourceHandle<R>) -> Option<&R> {
           if let Some(metrics) = self.metrics.get_mut(&handle) {
               metrics.last_accessed = Instant::now();
               metrics.access_count += 1;
           }
           
           self.table.get(handle)
       }
       
       fn remove(&mut self, handle: ResourceHandle<R>) {
           self.table.remove(handle);
           self.metrics.remove(&handle);
           self.total_destroyed += 1;
       }
       
       fn get_resource_stats(&self) -> ResourceStats {
           let current_count = self.metrics.len();
           let total_memory = self.metrics.values()
               .map(|m| m.size_bytes)
               .sum();
           
           let avg_age = if !self.metrics.is_empty() {
               let total_age: Duration = self.metrics.values()
                   .map(|m| m.created_at.elapsed())
                   .sum();
               total_age / current_count as u32
           } else {
               Duration::from_secs(0)
           };
           
           ResourceStats {
               active_resources: current_count,
               total_created: self.total_created,
               total_destroyed: self.total_destroyed,
               memory_usage_bytes: total_memory,
               average_age: avg_age,
           }
       }
       
       fn find_stale_resources(&self, max_age: Duration) -> Vec<ResourceHandle<R>> {
           self.metrics.iter()
               .filter(|(_, metrics)| metrics.last_accessed.elapsed() > max_age)
               .map(|(&handle, _)| handle)
               .collect()
       }
       
       fn cleanup_stale_resources(&mut self, max_age: Duration) -> usize {
           let stale_handles = self.find_stale_resources(max_age);
           let count = stale_handles.len();
           
           for handle in stale_handles {
               self.remove(handle);
           }
           
           count
       }
   }
   
   #[derive(Debug)]
   struct ResourceStats {
       active_resources: usize,
       total_created: u64,
       total_destroyed: u64,
       memory_usage_bytes: usize,
       average_age: Duration,
   }

Best Practices for Resource Management 💡
-----------------------------------------

.. admonition:: Resource Management Wisdom
   :class: tip

   1. **RAII**: Resources should clean up automatically
   2. **Weak References**: Use for optional/monitoring relationships
   3. **Lifecycle Hooks**: Monitor creation, access, and cleanup
   4. **Bounded Tables**: Set limits to prevent resource exhaustion
   5. **Regular Cleanup**: Implement periodic stale resource cleanup

Common Pitfalls 🕳️
------------------

.. admonition:: Avoid These Mistakes!
   :class: warning

   1. **Handle Reuse**: Don't assume handles remain valid after removal
   2. **Circular References**: Strong references can create cycles
   3. **Unbounded Growth**: Always set limits on resource tables
   4. **Expensive Cleanup**: Keep cleanup operations fast and simple

Your Turn! 🎮
-------------

Try these challenges:

1. **Build a File Cache**: Resource table for cached file contents
2. **Create a Connection Pool**: Shared database connections
3. **Implement a Texture Atlas**: Graphics texture management

Next Steps 🚶
-------------

- See resources in components: :doc:`../component/index`
- Learn about memory management: :doc:`safe_memory`
- Explore advanced patterns: :doc:`../advanced/index`

Remember: Good resource management is invisible - it just works! The best resource code prevents problems before they happen. 🛡️