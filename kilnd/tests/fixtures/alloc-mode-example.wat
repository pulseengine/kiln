;; Example WebAssembly module for alloc runtime mode
;; This module demonstrates features that require heap allocation but not full std
(module
  ;; Memory for dynamic allocation (larger than no_std but limited)
  (memory (export "memory") 2) ;; 2 pages = 128KB
  
  ;; Global pointer for simple heap allocation
  (global $heap_ptr (mut i32) (i32.const 1024)) ;; Start heap at 1KB
  
  ;; Simple allocator function
  (func $alloc (export "alloc") (param $size i32) (result i32)
    (local $ptr i32)
    
    ;; Get current heap pointer
    (local.set $ptr (global.get $heap_ptr))
    
    ;; Advance heap pointer
    (global.set $heap_ptr 
      (i32.add (global.get $heap_ptr) (local.get $size)))
    
    ;; Return allocated pointer
    (local.get $ptr)
  )
  
  ;; Function that demonstrates dynamic memory usage
  (func $dynamic_array (export "dynamic_array") (param $count i32) (result i32)
    (local $array_ptr i32)
    (local $i i32)
    (local $sum i32)
    
    ;; Allocate array (4 bytes per i32)
    (local.set $array_ptr 
      (call $alloc (i32.mul (local.get $count) (i32.const 4))))
    
    ;; Initialize array with values
    (loop $init_loop
      ;; Store value at array[i] = i * 2
      (i32.store 
        (i32.add 
          (local.get $array_ptr) 
          (i32.mul (local.get $i) (i32.const 4)))
        (i32.mul (local.get $i) (i32.const 2)))
      
      ;; Increment counter
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      
      ;; Continue if not done
      (br_if $init_loop 
        (i32.lt_u (local.get $i) (local.get $count)))
    )
    
    ;; Sum all values in the array
    (local.set $i (i32.const 0))
    (loop $sum_loop
      ;; Add array[i] to sum
      (local.set $sum 
        (i32.add 
          (local.get $sum)
          (i32.load 
            (i32.add 
              (local.get $array_ptr) 
              (i32.mul (local.get $i) (i32.const 4))))))
      
      ;; Increment counter
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      
      ;; Continue if not done
      (br_if $sum_loop 
        (i32.lt_u (local.get $i) (local.get $count)))
    )
    
    (local.get $sum)
  )
  
  ;; Function to test memory limits (should work in alloc mode)
  (func $memory_test (export "memory_test") (result i32)
    (local $ptr i32)
    
    ;; Try to allocate a moderately large block (32KB)
    (local.set $ptr (call $alloc (i32.const 32768)))
    
    ;; Write pattern to verify allocation worked
    (i32.store (local.get $ptr) (i32.const 0xDEADBEEF))
    
    ;; Read back and verify
    (i32.eq 
      (i32.load (local.get $ptr)) 
      (i32.const 0xDEADBEEF))
  )
)