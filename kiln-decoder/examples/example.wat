(module
  ;; Export a function that adds two numbers
  (func (export "add") (param i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
  )
  
  ;; Export a memory
  (memory (export "memory") 1)
)
