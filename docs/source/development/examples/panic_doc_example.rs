/// Example module demonstrating proper panic documentation
/// following the panic documentation guidelines.
pub mod example {
    /// This function divides two numbers but may panic.
    ///
    /// # Arguments
    ///
    /// * `a` - The dividend
    /// * `b` - The divisor
    ///
    /// # Returns
    ///
    /// The result of a / b
    ///
    /// # Panics
    ///
    /// This function will panic if `b` is zero, as division by zero
    /// is mathematically undefined.
    ///
    /// Safety impact: MEDIUM
    /// Tracking: KILNQ-500
    pub fn divide(a: i32, b: i32) -> i32 {
        a / b  // Will panic if b is 0
    }

    /// A safer version of the divide function that returns a Result instead of panicking.
    ///
    /// # Arguments
    ///
    /// * `a` - The dividend
    /// * `b` - The divisor
    ///
    /// # Returns
    ///
    /// `Ok(result)` if the division was successful, or `Err(DivisionError)` if `b` is zero
    pub fn safe_divide(a: i32, b: i32) -> Result<i32, &'static str> {
        if b == 0 {
            Err("Cannot divide by zero")
        } else {
            Ok(a / b)
        }
    }

    /// This function demonstrates multiple panic conditions.
    ///
    /// # Arguments
    ///
    /// * `values` - A slice of integers
    /// * `index` - An index to access in the slice
    ///
    /// # Returns
    ///
    /// The value at the given index, doubled
    ///
    /// # Panics
    ///
    /// This function will panic in the following cases:
    /// 1. If `index` is out of bounds for the slice
    /// 2. If the resulting doubled value overflows an i32
    ///
    /// Safety impact: HIGH
    /// Tracking: KILNQ-501
    pub fn get_and_double(values: &[i32], index: usize) -> i32 {
        let value = values[index]; // Will panic if index is out of bounds
        value.checked_mul(2).expect("Overflow when doubling value") // Will panic on overflow
    }

    /// A safer version of get_and_double that handles errors properly.
    ///
    /// # Arguments
    ///
    /// * `values` - A slice of integers
    /// * `index` - An index to access in the slice
    ///
    /// # Returns
    ///
    /// `Ok(result)` with the doubled value, or an appropriate error
    pub fn safe_get_and_double(values: &[i32], index: usize) -> Result<i32, &'static str> {
        let value = values.get(index).ok_or("Index out of bounds")?;
        value.checked_mul(2).ok_or("Overflow when doubling value")
    }
} 