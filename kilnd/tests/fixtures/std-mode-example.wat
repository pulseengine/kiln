;; Example WebAssembly module for std runtime mode
;; This module demonstrates features that require standard library support
(module
  ;; Import WASI fd_write for standard output
  (import "wasi_snapshot_preview1" "fd_write" 
    (func $fd_write (param i32 i32 i32 i32) (result i32)))
  
  ;; Memory for string data
  (memory (export "memory") 1)
  
  ;; Store "Hello from std mode!\n" at memory offset 0
  (data (i32.const 0) "Hello from std mode!\n")
  
  ;; Function to write the string to stdout
  (func $hello (export "hello") (result i32)
    ;; Set up iovec structure at offset 100
    ;; iovec.iov_base = 0 (pointer to string)
    (i32.store (i32.const 100) (i32.const 0))
    ;; iovec.iov_len = 21 (length of string)
    (i32.store (i32.const 104) (i32.const 21))
    
    ;; Call fd_write(stdout=1, iovec=100, iovec_count=1, bytes_written=200)
    (call $fd_write
      (i32.const 1)   ;; stdout file descriptor
      (i32.const 100) ;; iovec array
      (i32.const 1)   ;; number of iovecs
      (i32.const 200) ;; where to store bytes written
    )
  )
  
  ;; Function that demonstrates more complex std features
  (func $complex_std_function (export "complex") (param $iterations i32) (result i32)
    (local $i i32)
    (local $sum i32)
    
    ;; Loop that would benefit from std library optimizations
    (loop $main_loop
      ;; Add current iteration to sum
      (local.set $sum 
        (i32.add (local.get $sum) (local.get $i)))
      
      ;; Increment counter
      (local.set $i 
        (i32.add (local.get $i) (i32.const 1)))
      
      ;; Continue if not done
      (br_if $main_loop 
        (i32.lt_u (local.get $i) (local.get $iterations)))
    )
    
    (local.get $sum)
  )
)