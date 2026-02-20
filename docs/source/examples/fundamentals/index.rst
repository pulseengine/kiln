=========================
Safety Classification Examples
=========================

This section provides practical examples of using Kiln's unified safety classification system for cross-standard compatibility and compile-time safety verification.

.. contents:: On this page
   :local:
   :depth: 2

Basic Cross-Standard Usage
--------------------------

Comparing Safety Levels
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_safety::SafetyIntegrityLevel;

   fn compare_safety_levels() {
       // Automotive ASIL C and Industrial SIL 3 both have severity 750
       let automotive = SafetyIntegrityLevel::ASIL_C;
       let industrial = SafetyIntegrityLevel::SIL_3;
       
       assert_eq!(automotive.numeric_severity(), 750);
       assert_eq!(industrial.numeric_severity(), 750);
       
       // They can handle each other's requirements
       assert!(automotive.can_handle(&industrial));
       assert!(industrial.can_handle(&automotive));
       
       println!("Automotive: {} ({})", 
                automotive.terminology(), 
                automotive.industry());
       // Output: "Automotive: ASIL C (Automotive)"
       
       println!("Industrial: {} ({})", 
                industrial.terminology(), 
                industrial.industry());
       // Output: "Industrial: SIL 3 (Industrial)"
   }

Cross-Domain System Integration
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_safety::SafetyIntegrityLevel;

   struct SystemComponent {
       name: String,
       safety_level: SafetyIntegrityLevel,
   }

   fn integrate_mixed_criticality_system() {
       let components = vec![
           SystemComponent {
               name: "Automotive ECU".to_string(),
               safety_level: SafetyIntegrityLevel::ASIL_D,
           },
           SystemComponent {
               name: "Medical Device".to_string(),
               safety_level: SafetyIntegrityLevel::MEDICAL_C,
           },
           SystemComponent {
               name: "Industrial Controller".to_string(),
               safety_level: SafetyIntegrityLevel::SIL_4,
           },
           SystemComponent {
               name: "Railway Signaling".to_string(),
               safety_level: SafetyIntegrityLevel::RAIL_SIL_4,
           },
       ];
       
       // All components have maximum safety requirements (severity 1000)
       // They can all safely interface with each other
       for component in &components {
           assert_eq!(component.safety_level.numeric_severity(), 1000);
           println!("{}: {} - severity {}", 
                    component.name,
                    component.safety_level.terminology(),
                    component.safety_level.numeric_severity());
       }
       
       // Verify cross-component compatibility
       for i in 0..components.len() {
           for j in 0..components.len() {
               if i != j {
                   assert!(components[i].safety_level
                          .can_handle(&components[j].safety_level));
               }
           }
       }
   }

Compile-Time Safety Verification
--------------------------------

Using Safety Classifications in Functions
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_safety::{safety_classified, SafetyIntegrityLevel, static_safety_assert};

   // Function requires ASIL B or higher safety level
   #[safety_classified(SafetyIntegrityLevel::ASIL_B)]
   fn automotive_brake_control() {
       // Critical automotive function implementation
   }

   // Function requires Medical Class B or higher
   #[safety_classified(SafetyIntegrityLevel::MEDICAL_B)]
   fn medical_device_control() {
       // Medical device control implementation
   }

   // Function requires SIL 3 or higher
   #[safety_classified(SafetyIntegrityLevel::SIL_3)]
   fn industrial_safety_function() {
       // Industrial safety function implementation
   }

   fn system_integration() {
       // Define system-wide safety level
       const SYSTEM_SAFETY_LEVEL: SafetyIntegrityLevel = SafetyIntegrityLevel::ASIL_D;
       
       // Verify at compile time that system level can handle all function requirements
       static_safety_assert!(SYSTEM_SAFETY_LEVEL, SafetyIntegrityLevel::ASIL_B);
       static_safety_assert!(SYSTEM_SAFETY_LEVEL, SafetyIntegrityLevel::MEDICAL_B);
       static_safety_assert!(SYSTEM_SAFETY_LEVEL, SafetyIntegrityLevel::SIL_3);
       
       // These function calls are now statically verified to be safe
       automotive_brake_control();
       medical_device_control();
       industrial_safety_function();
   }

Safety Level Hierarchies
~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_safety::SafetyIntegrityLevel;

   fn demonstrate_safety_hierarchies() {
       // Automotive hierarchy
       let automotive_levels = vec![
           SafetyIntegrityLevel::ASIL_QM,  // 0
           SafetyIntegrityLevel::ASIL_A,   // 250
           SafetyIntegrityLevel::ASIL_B,   // 500
           SafetyIntegrityLevel::ASIL_C,   // 750
           SafetyIntegrityLevel::ASIL_D,   // 1000
       ];
       
       // Verify hierarchy ordering
       for i in 0..(automotive_levels.len() - 1) {
           let lower = &automotive_levels[i];
           let higher = &automotive_levels[i + 1];
           
           assert!(higher.can_handle(lower));
           assert!(!lower.can_handle(higher));
           assert!(higher.numeric_severity() >= lower.numeric_severity());
       }
       
       // Cross-standard equivalencies
       assert!(SafetyIntegrityLevel::ASIL_B.can_handle(&SafetyIntegrityLevel::SIL_2));
       assert!(SafetyIntegrityLevel::ASIL_C.can_handle(&SafetyIntegrityLevel::SIL_3));
       assert!(SafetyIntegrityLevel::ASIL_D.can_handle(&SafetyIntegrityLevel::SIL_4));
   }

Advanced Usage Patterns
-----------------------

Dynamic Safety Context
~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_safety::{SafetyIntegrityLevel, SafetyContext, VerificationLevel};

   struct SafeOperationContext {
       required_level: SafetyIntegrityLevel,
       current_level: SafetyIntegrityLevel,
       verification_level: VerificationLevel,
   }

   impl SafeOperationContext {
       fn new(required: SafetyIntegrityLevel, current: SafetyIntegrityLevel) -> Result<Self, String> {
           if !current.can_handle(&required) {
               return Err(format!(
                   "Insufficient safety level: {} required, {} available",
                   required.terminology(),
                   current.terminology()
               ));
           }
           
           let verification_level = match current.numeric_severity() {
               0..=249 => VerificationLevel::Basic,
               250..=499 => VerificationLevel::Standard,
               500..=749 => VerificationLevel::Enhanced,
               750..=1000 => VerificationLevel::Full,
               _ => VerificationLevel::Full,
           };
           
           Ok(Self {
               required_level: required,
               current_level: current,
               verification_level,
           })
       }
       
       fn execute_operation<F>(&self, operation: F) -> Result<(), String>
       where
           F: FnOnce() -> Result<(), String>,
       {
           // Additional verification based on safety level
           match self.verification_level {
               VerificationLevel::Full => {
                   // Pre-operation checks
                   self.pre_operation_verification()?;
                   let result = operation();
                   // Post-operation checks
                   self.post_operation_verification()?;
                   result
               }
               _ => operation(),
           }
       }
       
       fn pre_operation_verification(&self) -> Result<(), String> {
           // Implement pre-operation safety checks
           Ok(())
       }
       
       fn post_operation_verification(&self) -> Result<(), String> {
           // Implement post-operation safety checks
           Ok(())
       }
   }

   fn usage_example() -> Result<(), String> {
       let context = SafeOperationContext::new(
           SafetyIntegrityLevel::ASIL_C,
           SafetyIntegrityLevel::ASIL_D,
       )?;
       
       context.execute_operation(|| {
           // Critical operation implementation
           println!("Executing safety-critical operation");
           Ok(())
       })
   }

Agricultural Safety Example
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_safety::SafetyIntegrityLevel;

   struct AgriculturalMachine {
       name: String,
       safety_level: SafetyIntegrityLevel,
   }

   impl AgriculturalMachine {
       fn can_operate_with(&self, other: &AgriculturalMachine) -> bool {
           // Both machines must be able to handle each other's safety requirements
           self.safety_level.can_handle(&other.safety_level) &&
           other.safety_level.can_handle(&self.safety_level)
       }
   }

   fn agricultural_fleet_management() {
       let machines = vec![
           AgriculturalMachine {
               name: "Harvester".to_string(),
               safety_level: SafetyIntegrityLevel::AGPL_C,  // 550
           },
           AgriculturalMachine {
               name: "Tractor".to_string(),
               safety_level: SafetyIntegrityLevel::AGPL_B,  // 300
           },
           AgriculturalMachine {
               name: "Sprayer".to_string(),
               safety_level: SafetyIntegrityLevel::AGPL_D,  // 775
           },
       ];
       
       // Check which machines can operate together
       for i in 0..machines.len() {
           for j in (i+1)..machines.len() {
               let machine1 = &machines[i];
               let machine2 = &machines[j];
               
               if machine1.can_operate_with(machine2) {
                   println!("{} and {} can operate together safely", 
                            machine1.name, machine2.name);
               } else {
                   println!("{} and {} require additional safety measures", 
                            machine1.name, machine2.name);
               }
           }
       }
   }

Multi-Standard Validation
-------------------------

Standards Compliance Checking
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. code-block:: rust

   use kiln_safety::{SafetyIntegrityLevel, SafetyStandard};

   struct ComplianceChecker {
       required_standards: Vec<(SafetyStandard, SafetyIntegrityLevel)>,
   }

   impl ComplianceChecker {
       fn new() -> Self {
           Self {
               required_standards: Vec::new(),
           }
       }
       
       fn add_requirement(&mut self, standard: SafetyStandard, level: SafetyIntegrityLevel) {
           self.required_standards.push((standard, level));
       }
       
       fn check_compliance(&self, system_level: SafetyIntegrityLevel) -> Vec<String> {
           let mut violations = Vec::new();
           
           for (standard, required_level) in &self.required_standards {
               if !system_level.can_handle(required_level) {
                   violations.push(format!(
                       "Insufficient safety level for {}: {} required, system provides {}",
                       standard.name(),
                       required_level.terminology(),
                       system_level.terminology()
                   ));
               }
           }
           
           violations
       }
   }

   fn multi_standard_compliance_example() {
       let mut checker = ComplianceChecker::new();
       
       // Add requirements from different standards
       checker.add_requirement(SafetyStandard::ISO26262, SafetyIntegrityLevel::ASIL_C);
       checker.add_requirement(SafetyStandard::IEC61508, SafetyIntegrityLevel::SIL_2);
       checker.add_requirement(SafetyStandard::IEC62304, SafetyIntegrityLevel::MEDICAL_B);
       
       // Check system compliance
       let system_level = SafetyIntegrityLevel::ASIL_D;  // Highest automotive level
       let violations = checker.check_compliance(system_level);
       
       if violations.is_empty() {
           println!("System {} complies with all requirements", 
                    system_level.terminology());
       } else {
           for violation in violations {
               println!("Compliance violation: {}", violation);
           }
       }
   }

See Also
--------

- :doc:`../safety/safety_classification` - Complete safety classification documentation
- :doc:`../safety/mechanisms` - Safety mechanisms implementation
- :doc:`../architecture/safety` - Safety architecture overview