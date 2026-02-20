//! SHA-256 implementation for model hashing
//!
//! This module provides a no_std compatible SHA-256 implementation
//! following FIPS 180-4 specification for secure model hashing.
//!
//! # Safety
//!
//! This implementation is designed for ASIL-B compliance:
//! - No dynamic allocation
//! - Bounded execution time
//! - Deterministic output
//! - No unsafe code

#![cfg_attr(not(feature = "std"), no_std)]

/// SHA-256 constants (first 32 bits of fractional parts of cube roots of first
/// 64 primes)
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Initial hash values (first 32 bits of fractional parts of square roots of
/// first 8 primes)
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// SHA-256 context for incremental hashing
#[derive(Debug, Clone)]
pub struct Sha256 {
    /// Current hash state
    state:      [u32; 8],
    /// Buffer for incomplete blocks
    buffer:     [u8; 64],
    /// Number of bytes in buffer
    buffer_len: usize,
    /// Total message length in bits
    bit_len:    u64,
}

impl Sha256 {
    /// Create a new SHA-256 context
    pub const fn new() -> Self {
        Self {
            state:      H0,
            buffer:     [0u8; 64],
            buffer_len: 0,
            bit_len:    0,
        }
    }

    /// Update the hash with new data
    pub fn update(&mut self, data: &[u8]) {
        let mut data_offset = 0;

        // Update bit length
        self.bit_len = self.bit_len.saturating_add((data.len() as u64) * 8);

        // If we have buffered data, fill the buffer first
        if self.buffer_len > 0 {
            let copy_len = (64 - self.buffer_len).min(data.len());
            self.buffer[self.buffer_len..self.buffer_len + copy_len]
                .copy_from_slice(&data[..copy_len]);
            self.buffer_len += copy_len;
            data_offset += copy_len;

            // Process complete block if buffer is full
            if self.buffer_len == 64 {
                let mut block = [0u8; 64];
                block.copy_from_slice(&self.buffer);
                self.process_block(&block);
                self.buffer_len = 0;
            }
        }

        // Process complete 64-byte blocks
        while data_offset + 64 <= data.len() {
            let block = &data[data_offset..data_offset + 64];
            let mut block_array = [0u8; 64];
            block_array.copy_from_slice(block);
            self.process_block(&block_array);
            data_offset += 64;
        }

        // Buffer any remaining bytes
        if data_offset < data.len() {
            let remaining = data.len() - data_offset;
            self.buffer[..remaining].copy_from_slice(&data[data_offset..]);
            self.buffer_len = remaining;
        }
    }

    /// Finalize the hash and return the result
    pub fn finalize(mut self) -> [u8; 32] {
        // Pad the message
        self.pad();

        // Convert state to bytes
        let mut result = [0u8; 32];
        for (i, &word) in self.state.iter().enumerate() {
            result[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        result
    }

    /// Process a single 512-bit block
    fn process_block(&mut self, block: &[u8; 64]) {
        // Message schedule array
        let mut w = [0u32; 64];

        // Copy block into first 16 words
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }

        // Extend the sixteen 32-bit words into sixty-four 32-bit words
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }

        // Initialize working variables
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        // Main loop
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        // Add the compressed chunk to the current hash value
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    /// Apply padding to the message
    fn pad(&mut self) {
        // Append the bit '1' to the message
        self.buffer[self.buffer_len] = 0x80;
        self.buffer_len += 1;

        // Check if we have room for the 64-bit length
        if self.buffer_len > 56 {
            // Fill rest of block with zeros and process
            self.buffer[self.buffer_len..].fill(0);
            let mut block = [0u8; 64];
            block.copy_from_slice(&self.buffer);
            self.process_block(&block);
            self.buffer.fill(0);
            self.buffer_len = 0;
        } else {
            // Fill with zeros up to length field
            self.buffer[self.buffer_len..56].fill(0);
        }

        // Append length in bits as 64-bit big-endian
        self.buffer[56..64].copy_from_slice(&self.bit_len.to_be_bytes());
        let mut final_block = [0u8; 64];
        final_block.copy_from_slice(&self.buffer);
        self.process_block(&final_block);
    }
}

/// Compute SHA-256 hash of data in one shot
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_hash() {
        let hash = sha256(b"");
        let expected = [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ];
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_abc_hash() {
        let hash = sha256(b"abc");
        let expected = [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ];
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_long_message() {
        let hash = sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
        let expected = [
            0x24, 0x8d, 0x6a, 0x61, 0xd2, 0x06, 0x38, 0xb8, 0xe5, 0xc0, 0x26, 0x93, 0x0c, 0x3e,
            0x60, 0x39, 0xa3, 0x3c, 0xe4, 0x59, 0x64, 0xff, 0x21, 0x67, 0xf6, 0xec, 0xed, 0xd4,
            0x19, 0xdb, 0x06, 0xc1,
        ];
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_exact_block_size() {
        // Test with exactly 64 bytes (one block)
        let data = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(data.len(), 64);
        let hash = sha256(data);
        // Just verify it doesn't panic and produces a hash
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn test_incremental_hashing() {
        // Test that incremental hashing produces same result
        let data = b"The quick brown fox jumps over the lazy dog";

        // One shot
        let hash1 = sha256(data);

        // Incremental
        let mut hasher = Sha256::new();
        hasher.update(&data[..10]);
        hasher.update(&data[10..20]);
        hasher.update(&data[20..]);
        let hash2 = hasher.finalize();

        assert_eq!(hash1, hash2);
    }
}
