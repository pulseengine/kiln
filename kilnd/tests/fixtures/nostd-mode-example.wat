;; Example WebAssembly module for no_std runtime mode
;; This module demonstrates minimal features for bare metal/embedded systems
(module
  ;; Minimal memory (1 page = 64KB max for no_std constraints)
  (memory (export "memory") 1)
  
  ;; Simple arithmetic function (no heap allocation)
  (func $add (export "add") (param $a i32) (param $b i32) (result i32)
    (i32.add (local.get $a) (local.get $b))
  )
  
  ;; Basic multiplication
  (func $multiply (export "multiply") (param $a i32) (param $b i32) (result i32)
    (i32.mul (local.get $a) (local.get $b))
  )
  
  ;; Stack-based computation (no dynamic allocation)
  (func $fibonacci (export "fibonacci") (param $n i32) (result i32)
    (local $a i32)
    (local $b i32)
    (local $c i32)
    (local $i i32)
    
    ;; Handle base cases
    (if (i32.le_u (local.get $n) (i32.const 1))
      (then (return (local.get $n))))
    
    ;; Initialize first two Fibonacci numbers
    (local.set $a (i32.const 0))
    (local.set $b (i32.const 1))
    (local.set $i (i32.const 2))
    
    ;; Compute Fibonacci iteratively (stack-based)
    (loop $fib_loop
      ;; c = a + b
      (local.set $c (i32.add (local.get $a) (local.get $b)))
      
      ;; Shift values: a = b, b = c
      (local.set $a (local.get $b))
      (local.set $b (local.get $c))
      
      ;; Increment counter
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      
      ;; Continue if not done
      (br_if $fib_loop 
        (i32.le_u (local.get $i) (local.get $n)))
    )
    
    (local.get $b)
  )
  
  ;; Fixed-size array operations (using linear memory but no allocation)
  (func $array_sum (export "array_sum") (param $count i32) (result i32)
    (local $i i32)
    (local $sum i32)
    
    ;; Limit array size for no_std constraints (max 64 elements)
    (if (i32.gt_u (local.get $count) (i32.const 64))
      (then (local.set $count (i32.const 64))))
    
    ;; Initialize array with values at memory offset 0
    (loop $init_loop
      ;; Store value at memory[i*4] = i + 1
      (i32.store 
        (i32.mul (local.get $i) (i32.const 4))
        (i32.add (local.get $i) (i32.const 1)))
      
      ;; Increment counter
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      
      ;; Continue if not done
      (br_if $init_loop 
        (i32.lt_u (local.get $i) (local.get $count)))
    )
    
    ;; Sum all values
    (local.set $i (i32.const 0))
    (loop $sum_loop
      ;; Add memory[i*4] to sum
      (local.set $sum 
        (i32.add 
          (local.get $sum)
          (i32.load (i32.mul (local.get $i) (i32.const 4)))))
      
      ;; Increment counter
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      
      ;; Continue if not done
      (br_if $sum_loop 
        (i32.lt_u (local.get $i) (local.get $count)))
    )
    
    (local.get $sum)
  )
  
  ;; Bit manipulation (common in embedded systems)
  (func $bit_operations (export "bit_ops") (param $value i32) (result i32)
    (local $result i32)
    
    ;; Perform various bit operations
    (local.set $result (local.get $value))
    
    ;; Set bit 0
    (local.set $result 
      (i32.or (local.get $result) (i32.const 1)))
    
    ;; Clear bit 1
    (local.set $result 
      (i32.and (local.get $result) (i32.const 0xFFFFFFFD)))
    
    ;; Toggle bit 2
    (local.set $result 
      (i32.xor (local.get $result) (i32.const 4)))
    
    ;; Rotate left by 1
    (local.set $result 
      (i32.or 
        (i32.shl (local.get $result) (i32.const 1))
        (i32.shr_u (local.get $result) (i32.const 31))))
    
    (local.get $result)
  )
)