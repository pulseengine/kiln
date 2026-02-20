PlantUML Test
=============

Testing if PlantUML rendering works in the documentation build.

Simple Test Diagram
-------------------

.. uml::

   @startuml
   actor User
   participant "Kiln Runtime" as Kiln
   database "WASM Module" as WASM
   
   User -> Kiln: Execute module
   Kiln -> WASM: Load binary
   WASM --> Kiln: Module loaded
   Kiln -> Kiln: Validate
   Kiln -> Kiln: Instantiate
   Kiln --> User: Execution result
   @enduml

Component Test
--------------

.. uml::

   @startuml
   component "Test Component" as TC {
       component "Module A" as MA
       component "Module B" as MB
   }
   
   MA --> MB : uses
   @enduml