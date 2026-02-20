=====================================
Component Values: Type-Safe Boundaries
=====================================

.. epigraph::

   "The boundary between two worlds is where the magic happens."
   
   -- Unknown (but definitely talking about WebAssembly components)

Moving data between the host and WebAssembly components isn't just about bytes - it's about types, safety, and making sure a u32 stays a u32. Kiln's component value system handles the tricky business of type conversion so you don't have to worry about ABI mismatches or data corruption.

.. admonition:: What You'll Learn
   :class: note

   - Converting between host and guest types
   - Working with ComponentValue for type safety
   - Building efficient value stores
   - Handling complex data structures
   - Debugging type conversion issues

The Type Safety Challenge 🛡️
-----------------------------

WebAssembly components speak in well-defined types, but the host world is messy:

.. code-block:: rust
   :caption: The challenge we're solving

   // Host side (Rust)
   struct HostData {
       id: u64,
       name: String,
       active: bool,
       scores: Vec<f32>,
   }
   
   // Component side (WIT interface)
   // record guest-data {
   //     id: u64,
   //     name: string,
   //     active: bool,
   //     scores: list<f32>,
   // }
   
   // How do we safely convert between these?
   // Kiln ComponentValue to the rescue!

Enter ComponentValue: Your Type-Safe Bridge 🌉
----------------------------------------------

ComponentValue provides safe, efficient conversion between host and guest types:

.. code-block:: rust
   :caption: Basic ComponentValue usage
   :linenos:

   use kiln_foundation::component_value::{ComponentValue, ComponentType};
   use kiln_foundation::prelude::*;
   
   fn basic_value_conversion() {
       // Creating values from Rust types
       let number = ComponentValue::U32(42);
       let text = ComponentValue::String("Hello, Kiln!".to_string());
       let flag = ComponentValue::Bool(true);
       
       // Extract values safely
       match number {
           ComponentValue::U32(n) => println!("Number: {}", n),
           _ => println!("Not a number!"),
       }
       
       // Or use the helper methods
       if let Some(n) = number.as_u32() {
           println!("Number via helper: {}", n);
       }
       
       // Type checking
       assert_eq!(number.type_info(), ComponentType::U32);
       assert_eq!(text.type_info(), ComponentType::String);
   }

Working with Complex Types 🧩
-----------------------------

Real applications need more than primitives:

.. code-block:: rust
   :caption: Complex type handling
   :linenos:

   use kiln_foundation::component_value::{ComponentValue, Record, List};
   use std::collections::HashMap;
   
   fn complex_type_example() {
       // Create a record (like a struct)
       let mut person_record = Record::new();
       person_record.insert("id".to_string(), ComponentValue::U64(12345));
       person_record.insert("name".to_string(), ComponentValue::String("Alice".to_string()));
       person_record.insert("age".to_string(), ComponentValue::U32(30));
       person_record.insert("active".to_string(), ComponentValue::Bool(true));
       
       let person = ComponentValue::Record(person_record);
       
       // Create a list of scores
       let scores = vec![
           ComponentValue::F32(95.5),
           ComponentValue::F32(87.2),
           ComponentValue::F32(92.8),
       ];
       let scores_list = ComponentValue::List(List::new(scores));
       
       // Nested structures work too!
       let mut complex_record = Record::new();
       complex_record.insert("person".to_string(), person);
       complex_record.insert("scores".to_string(), scores_list);
       
       let complex_value = ComponentValue::Record(complex_record);
       
       // Access nested data safely
       if let ComponentValue::Record(ref record) = complex_value {
           if let Some(ComponentValue::Record(ref person_rec)) = record.get("person") {
               if let Some(ComponentValue::String(ref name)) = person_rec.get("name") {
                   println!("Person name: {}", name);
               }
           }
       }
   }

ValueStore: Efficient Value Management 📦
-----------------------------------------

For high-performance scenarios, use ValueStore:

.. code-block:: rust
   :caption: ValueStore for efficient operations
   :linenos:

   use kiln_foundation::component_value_store::{ValueStore, ValueHandle};
   use kiln_foundation::component_value::ComponentValue;
   
   struct HighPerformanceProcessor {
       store: ValueStore,
       temp_handles: Vec<ValueHandle>,
   }
   
   impl HighPerformanceProcessor {
       fn new() -> Self {
           Self {
               store: ValueStore::with_capacity(1000),
               temp_handles: Vec::new(),
           }
       }
       
       fn process_batch(&mut self, values: &[ComponentValue]) -> Vec<ValueHandle> {
           let mut results = Vec::new();
           
           for value in values {
               // Store values efficiently
               let handle = self.store.insert(value.clone());
               
               // Process the value
               let processed = self.process_single_value(handle);
               results.push(processed);
           }
           
           results
       }
       
       fn process_single_value(&mut self, handle: ValueHandle) -> ValueHandle {
           // Get value without copying
           if let Some(value) = self.store.get(handle) {
               match value {
                   ComponentValue::U32(n) => {
                       // Double the number
                       let doubled = ComponentValue::U32(n * 2);
                       self.store.insert(doubled)
                   }
                   ComponentValue::String(s) => {
                       // Uppercase the string
                       let upper = ComponentValue::String(s.to_uppercase());
                       self.store.insert(upper)
                   }
                   ComponentValue::List(ref list) => {
                       // Process each element in the list
                       let mut new_items = Vec::new();
                       for item in list.items() {
                           let item_handle = self.store.insert(item.clone());
                           let processed_handle = self.process_single_value(item_handle);
                           if let Some(processed_value) = self.store.get(processed_handle) {
                               new_items.push(processed_value.clone());
                           }
                       }
                       let new_list = ComponentValue::List(List::new(new_items));
                       self.store.insert(new_list)
                   }
                   _ => handle, // Return unchanged for other types
               }
           } else {
               handle
           }
       }
       
       fn cleanup(&mut self) {
           // Remove temporary values to free memory
           for &handle in &self.temp_handles {
               self.store.remove(handle);
           }
           self.temp_handles.clear();
       }
   }

Real-World Example: API Gateway 🚪
----------------------------------

Let's build an API gateway that handles different data formats:

.. code-block:: rust
   :caption: Component-based API gateway
   :linenos:

   use kiln_foundation::component_value::{ComponentValue, Record, List};
   use kiln_foundation::component_value_store::ValueStore;
   use std::collections::HashMap;
   
   #[derive(Debug)]
   enum ApiError {
       InvalidFormat,
       MissingField(String),
       TypeMismatch,
       ProcessingError,
   }
   
   struct ApiGateway {
       value_store: ValueStore,
       request_handlers: HashMap<String, fn(&ComponentValue) -> Result<ComponentValue, ApiError>>,
   }
   
   impl ApiGateway {
       fn new() -> Self {
           let mut gateway = Self {
               value_store: ValueStore::with_capacity(10000),
               request_handlers: HashMap::new(),
           };
           
           // Register handlers for different endpoints
           gateway.register_handler("user/create", Self::handle_user_creation);
           gateway.register_handler("user/update", Self::handle_user_update);
           gateway.register_handler("data/process", Self::handle_data_processing);
           
           gateway
       }
       
       fn register_handler(
           &mut self,
           endpoint: &str,
           handler: fn(&ComponentValue) -> Result<ComponentValue, ApiError>
       ) {
           self.request_handlers.insert(endpoint.to_string(), handler);
       }
       
       fn handle_request(&mut self, endpoint: &str, request: ComponentValue) -> Result<ComponentValue, ApiError> {
           // Store the request for processing
           let request_handle = self.value_store.insert(request);
           
           // Find and execute the handler
           if let Some(&handler) = self.request_handlers.get(endpoint) {
               if let Some(request_value) = self.value_store.get(request_handle) {
                   let result = handler(request_value)?;
                   Ok(result)
               } else {
                   Err(ApiError::ProcessingError)
               }
           } else {
               Err(ApiError::InvalidFormat)
           }
       }
       
       fn handle_user_creation(request: &ComponentValue) -> Result<ComponentValue, ApiError> {
           // Parse user creation request
           let record = request.as_record().ok_or(ApiError::InvalidFormat)?;
           
           // Validate required fields
           let name = record.get("name")
               .and_then(|v| v.as_string())
               .ok_or_else(|| ApiError::MissingField("name".to_string()))?;
           
           let email = record.get("email")
               .and_then(|v| v.as_string())
               .ok_or_else(|| ApiError::MissingField("email".to_string()))?;
           
           let age = record.get("age")
               .and_then(|v| v.as_u32())
               .unwrap_or(0);
           
           // Create user (simulate database operation)
           let user_id = generate_user_id();
           
           // Build response
           let mut response = Record::new();
           response.insert("user_id".to_string(), ComponentValue::U64(user_id));
           response.insert("name".to_string(), ComponentValue::String(name.clone()));
           response.insert("email".to_string(), ComponentValue::String(email.clone()));
           response.insert("age".to_string(), ComponentValue::U32(age));
           response.insert("created_at".to_string(), ComponentValue::U64(get_timestamp()));
           response.insert("status".to_string(), ComponentValue::String("active".to_string()));
           
           Ok(ComponentValue::Record(response))
       }
       
       fn handle_user_update(request: &ComponentValue) -> Result<ComponentValue, ApiError> {
           let record = request.as_record().ok_or(ApiError::InvalidFormat)?;
           
           let user_id = record.get("user_id")
               .and_then(|v| v.as_u64())
               .ok_or_else(|| ApiError::MissingField("user_id".to_string()))?;
           
           // Build update response
           let mut response = Record::new();
           response.insert("user_id".to_string(), ComponentValue::U64(user_id));
           response.insert("updated_at".to_string(), ComponentValue::U64(get_timestamp()));
           response.insert("status".to_string(), ComponentValue::String("updated".to_string()));
           
           // Copy any updated fields
           for (key, value) in record.iter() {
               if key != "user_id" {
                   response.insert(key.clone(), value.clone());
               }
           }
           
           Ok(ComponentValue::Record(response))
       }
       
       fn handle_data_processing(request: &ComponentValue) -> Result<ComponentValue, ApiError> {
           let record = request.as_record().ok_or(ApiError::InvalidFormat)?;
           
           let data_list = record.get("data")
               .and_then(|v| v.as_list())
               .ok_or_else(|| ApiError::MissingField("data".to_string()))?;
           
           // Process each item in the data list
           let mut processed_items = Vec::new();
           for item in data_list.items() {
               match item {
                   ComponentValue::F32(n) => {
                       // Square the number
                       processed_items.push(ComponentValue::F32(n * n));
                   }
                   ComponentValue::String(s) => {
                       // Reverse the string
                       let reversed: String = s.chars().rev().collect();
                       processed_items.push(ComponentValue::String(reversed));
                   }
                   _ => processed_items.push(item.clone()),
               }
           }
           
           let mut response = Record::new();
           response.insert("processed_data".to_string(), 
               ComponentValue::List(List::new(processed_items)));
           response.insert("processed_at".to_string(), ComponentValue::U64(get_timestamp()));
           
           Ok(ComponentValue::Record(response))
       }
   }
   
   // Utility functions
   fn generate_user_id() -> u64 {
       use std::time::{SystemTime, UNIX_EPOCH};
       SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
   }
   
   fn get_timestamp() -> u64 {
       use std::time::{SystemTime, UNIX_EPOCH};
       SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
   }

Type Conversion Builder Pattern 🏗️
----------------------------------

For complex conversions, use the builder pattern:

.. code-block:: rust
   :caption: Type conversion builder

   use kiln_foundation::component_value::{ComponentValue, Record, ValueBuilder};
   
   struct ConfigBuilder {
       builder: ValueBuilder,
   }
   
   impl ConfigBuilder {
       fn new() -> Self {
           Self {
               builder: ValueBuilder::new(),
           }
       }
       
       fn with_database_config(mut self, host: &str, port: u16, name: &str) -> Self {
           let mut db_config = Record::new();
           db_config.insert("host".to_string(), ComponentValue::String(host.to_string()));
           db_config.insert("port".to_string(), ComponentValue::U32(port as u32));
           db_config.insert("database".to_string(), ComponentValue::String(name.to_string()));
           
           self.builder.add_field("database", ComponentValue::Record(db_config));
           self
       }
       
       fn with_logging_config(mut self, level: &str, file: Option<&str>) -> Self {
           let mut log_config = Record::new();
           log_config.insert("level".to_string(), ComponentValue::String(level.to_string()));
           
           if let Some(file_path) = file {
               log_config.insert("file".to_string(), ComponentValue::String(file_path.to_string()));
           }
           
           self.builder.add_field("logging", ComponentValue::Record(log_config));
           self
       }
       
       fn with_features(mut self, features: &[&str]) -> Self {
           let feature_values: Vec<ComponentValue> = features.iter()
               .map(|&f| ComponentValue::String(f.to_string()))
               .collect();
           
           self.builder.add_field("features", ComponentValue::List(List::new(feature_values)));
           self
       }
       
       fn build(self) -> ComponentValue {
           self.builder.build()
       }
   }
   
   // Usage
   fn build_config_example() {
       let config = ConfigBuilder::new()
           .with_database_config("localhost", 5432, "kiln_app")
           .with_logging_config("info", Some("/var/log/kiln.log"))
           .with_features(&["authentication", "metrics", "caching"])
           .build();
       
       println!("Generated config: {:?}", config);
   }

Debugging Value Conversions 🔍
------------------------------

Built-in tools for debugging type issues:

.. code-block:: rust
   :caption: Debugging helpers

   use kiln_foundation::component_value::{ComponentValue, ValueDebugger};
   
   fn debug_value_conversion() {
       let complex_value = create_complex_value();
       
       // Pretty-print the value structure
       let debugger = ValueDebugger::new();
       println!("Value structure:");
       debugger.print_structure(&complex_value, 0);
       
       // Validate value integrity
       match debugger.validate(&complex_value) {
           Ok(()) => println!("Value is valid"),
           Err(issues) => {
               println!("Value validation issues:");
               for issue in issues {
                   println!("  - {}", issue);
               }
           }
       }
       
       // Memory usage analysis
       let memory_info = debugger.analyze_memory_usage(&complex_value);
       println!("Memory usage: {} bytes", memory_info.total_size);
       println!("String overhead: {} bytes", memory_info.string_overhead);
   }
   
   fn create_complex_value() -> ComponentValue {
       // Create a complex nested structure for testing
       let mut root = Record::new();
       root.insert("version".to_string(), ComponentValue::U32(1));
       
       let mut nested = Record::new();
       nested.insert("id".to_string(), ComponentValue::U64(12345));
       nested.insert("data".to_string(), ComponentValue::String("test".to_string()));
       
       root.insert("nested".to_string(), ComponentValue::Record(nested));
       ComponentValue::Record(root)
   }

Performance Optimization Tips 🏁
--------------------------------

.. admonition:: Performance Tips
   :class: tip

   1. **Reuse ValueStore**: Don't create new stores for each operation
   2. **Batch Operations**: Process multiple values together
   3. **Avoid Deep Nesting**: Flat structures are faster
   4. **Use Handles**: For temporary values, use handles instead of cloning
   5. **Profile Conversions**: Measure where time is spent

Error Handling Patterns 🛡️
---------------------------

Robust error handling for production systems:

.. code-block:: rust
   :caption: Comprehensive error handling

   use kiln_foundation::component_value::{ComponentValue, ConversionError};
   
   #[derive(Debug)]
   enum ValueProcessingError {
       ConversionFailed(ConversionError),
       ValidationFailed(String),
       IncompatibleTypes,
       ResourceExhausted,
   }
   
   impl From<ConversionError> for ValueProcessingError {
       fn from(err: ConversionError) -> Self {
           ValueProcessingError::ConversionFailed(err)
       }
   }
   
   fn safe_value_processing(input: ComponentValue) -> Result<ComponentValue, ValueProcessingError> {
       // Validate input structure
       validate_input_structure(&input)?;
       
       // Attempt conversion
       let converted = convert_value_safely(input)?;
       
       // Validate output
       validate_output_structure(&converted)?;
       
       Ok(converted)
   }
   
   fn validate_input_structure(value: &ComponentValue) -> Result<(), ValueProcessingError> {
       match value {
           ComponentValue::Record(record) => {
               if record.is_empty() {
                   return Err(ValueProcessingError::ValidationFailed(
                       "Empty record not allowed".to_string()
                   ));
               }
           }
           ComponentValue::List(list) => {
               if list.len() > 1000 {
                   return Err(ValueProcessingError::ResourceExhausted);
               }
           }
           _ => {}
       }
       Ok(())
   }
   
   fn convert_value_safely(value: ComponentValue) -> Result<ComponentValue, ValueProcessingError> {
       // Safe conversion logic here
       Ok(value) // Simplified
   }
   
   fn validate_output_structure(value: &ComponentValue) -> Result<(), ValueProcessingError> {
       // Output validation logic
       Ok(())
   }

Your Turn! 🎮
-------------

Try these challenges:

1. **Build a Schema Validator**: Validate ComponentValues against a schema
2. **Create a Value Transformer**: Transform values based on rules
3. **Implement Serialization**: Convert ComponentValues to JSON/YAML

Next Steps 🚶
-------------

- Explore resource management: :doc:`resources`
- See values in action: :doc:`../component/type_conversion`
- Learn advanced patterns: :doc:`../advanced/index`

Remember: Type safety isn't just about preventing crashes - it's about building systems you can trust! 🛡️