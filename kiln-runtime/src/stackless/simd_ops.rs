//! SIMD (V128) operation helper functions
//!
//! Pure computation helpers for SIMD operations. These functions operate on
//! `[u8; 16]` byte arrays in little-endian format, matching the WebAssembly
//! V128 value representation.

// ============================================================
// NaN canonicalization helpers
// ============================================================
// WebAssembly spec requires all NaN results to be canonical quiet NaN.
// Canonical NaN: f32 = 0x7FC00000, f64 = 0x7FF8000000000000

/// Canonical quiet NaN for f32 (positive, quiet, no payload)
const CANONICAL_NAN_F32: u32 = 0x7FC0_0000;

/// Canonical quiet NaN for f64 (positive, quiet, no payload)
const CANONICAL_NAN_F64: u64 = 0x7FF8_0000_0000_0000;

/// If the f32 value is NaN, replace with canonical quiet NaN.
#[inline]
pub fn canonicalize_f32(v: f32) -> f32 {
    if v.is_nan() {
        f32::from_bits(CANONICAL_NAN_F32)
    } else {
        v
    }
}

/// If the f64 value is NaN, replace with canonical quiet NaN.
#[inline]
pub fn canonicalize_f64(v: f64) -> f64 {
    if v.is_nan() {
        f64::from_bits(CANONICAL_NAN_F64)
    } else {
        v
    }
}

// ============================================================
// Lane accessor helpers
// ============================================================

#[inline]
pub fn get_i8(v: &[u8; 16], lane: usize) -> i8 {
    v[lane] as i8
}

#[inline]
pub fn set_i8(v: &mut [u8; 16], lane: usize, val: i8) {
    v[lane] = val as u8;
}

#[inline]
pub fn get_u8(v: &[u8; 16], lane: usize) -> u8 {
    v[lane]
}

#[inline]
pub fn set_u8(v: &mut [u8; 16], lane: usize, val: u8) {
    v[lane] = val;
}

#[inline]
pub fn get_i16(v: &[u8; 16], lane: usize) -> i16 {
    i16::from_le_bytes([v[lane * 2], v[lane * 2 + 1]])
}

#[inline]
pub fn set_i16(v: &mut [u8; 16], lane: usize, val: i16) {
    let b = val.to_le_bytes();
    v[lane * 2] = b[0];
    v[lane * 2 + 1] = b[1];
}

#[inline]
pub fn get_u16(v: &[u8; 16], lane: usize) -> u16 {
    u16::from_le_bytes([v[lane * 2], v[lane * 2 + 1]])
}

#[inline]
pub fn set_u16(v: &mut [u8; 16], lane: usize, val: u16) {
    let b = val.to_le_bytes();
    v[lane * 2] = b[0];
    v[lane * 2 + 1] = b[1];
}

#[inline]
pub fn get_i32(v: &[u8; 16], lane: usize) -> i32 {
    let o = lane * 4;
    i32::from_le_bytes([v[o], v[o + 1], v[o + 2], v[o + 3]])
}

#[inline]
pub fn set_i32(v: &mut [u8; 16], lane: usize, val: i32) {
    let o = lane * 4;
    let b = val.to_le_bytes();
    v[o] = b[0];
    v[o + 1] = b[1];
    v[o + 2] = b[2];
    v[o + 3] = b[3];
}

#[inline]
pub fn get_u32(v: &[u8; 16], lane: usize) -> u32 {
    let o = lane * 4;
    u32::from_le_bytes([v[o], v[o + 1], v[o + 2], v[o + 3]])
}

#[inline]
pub fn set_u32(v: &mut [u8; 16], lane: usize, val: u32) {
    let o = lane * 4;
    let b = val.to_le_bytes();
    v[o] = b[0];
    v[o + 1] = b[1];
    v[o + 2] = b[2];
    v[o + 3] = b[3];
}

#[inline]
pub fn get_i64(v: &[u8; 16], lane: usize) -> i64 {
    let o = lane * 8;
    let mut b = [0u8; 8];
    b.copy_from_slice(&v[o..o + 8]);
    i64::from_le_bytes(b)
}

#[inline]
pub fn set_i64(v: &mut [u8; 16], lane: usize, val: i64) {
    let o = lane * 8;
    v[o..o + 8].copy_from_slice(&val.to_le_bytes());
}

#[inline]
pub fn get_u64(v: &[u8; 16], lane: usize) -> u64 {
    let o = lane * 8;
    let mut b = [0u8; 8];
    b.copy_from_slice(&v[o..o + 8]);
    u64::from_le_bytes(b)
}

#[inline]
pub fn set_u64(v: &mut [u8; 16], lane: usize, val: u64) {
    let o = lane * 8;
    v[o..o + 8].copy_from_slice(&val.to_le_bytes());
}

#[inline]
pub fn get_f32(v: &[u8; 16], lane: usize) -> f32 {
    let o = lane * 4;
    f32::from_le_bytes([v[o], v[o + 1], v[o + 2], v[o + 3]])
}

#[inline]
pub fn set_f32(v: &mut [u8; 16], lane: usize, val: f32) {
    let o = lane * 4;
    let b = val.to_le_bytes();
    v[o] = b[0];
    v[o + 1] = b[1];
    v[o + 2] = b[2];
    v[o + 3] = b[3];
}

#[inline]
pub fn get_f64(v: &[u8; 16], lane: usize) -> f64 {
    let o = lane * 8;
    let mut b = [0u8; 8];
    b.copy_from_slice(&v[o..o + 8]);
    f64::from_le_bytes(b)
}

#[inline]
pub fn set_f64(v: &mut [u8; 16], lane: usize, val: f64) {
    let o = lane * 8;
    v[o..o + 8].copy_from_slice(&val.to_le_bytes());
}

// ============================================================
// Splat operations
// ============================================================

#[inline]
pub fn splat_i8x16(val: u8) -> [u8; 16] {
    [val; 16]
}

#[inline]
pub fn splat_i16x8(val: u16) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, val);
    }
    r
}

#[inline]
pub fn splat_i32x4(val: u32) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, val);
    }
    r
}

#[inline]
pub fn splat_i64x2(val: u64) -> [u8; 16] {
    let mut r = [0u8; 16];
    set_u64(&mut r, 0, val);
    set_u64(&mut r, 1, val);
    r
}

#[inline]
pub fn splat_f32x4(val: f32) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_f32(&mut r, i, val);
    }
    r
}

#[inline]
pub fn splat_f64x2(val: f64) -> [u8; 16] {
    let mut r = [0u8; 16];
    set_f64(&mut r, 0, val);
    set_f64(&mut r, 1, val);
    r
}

// ============================================================
// Bitwise operations
// ============================================================

#[inline]
pub fn v128_not(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = !v[i];
    }
    r
}

#[inline]
pub fn v128_and(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i] & b[i];
    }
    r
}

#[inline]
pub fn v128_andnot(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i] & !b[i];
    }
    r
}

#[inline]
pub fn v128_or(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i] | b[i];
    }
    r
}

#[inline]
pub fn v128_xor(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i] ^ b[i];
    }
    r
}

#[inline]
pub fn v128_bitselect(v1: &[u8; 16], v2: &[u8; 16], c: &[u8; 16]) -> [u8; 16] {
    // result = (v1 & c) | (v2 & ~c)
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = (v1[i] & c[i]) | (v2[i] & !c[i]);
    }
    r
}

#[inline]
pub fn v128_any_true(v: &[u8; 16]) -> bool {
    v.iter().any(|&b| b != 0)
}

// ============================================================
// i8x16 operations
// ============================================================

#[inline]
pub fn i8x16_swizzle(a: &[u8; 16], s: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        let idx = s[i] as usize;
        r[i] = if idx < 16 { a[idx] } else { 0 };
    }
    r
}

#[inline]
pub fn i8x16_shuffle(a: &[u8; 16], b: &[u8; 16], lanes: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    let combined: [u8; 32] = {
        let mut c = [0u8; 32];
        c[..16].copy_from_slice(a);
        c[16..].copy_from_slice(b);
        c
    };
    for i in 0..16 {
        let idx = lanes[i] as usize;
        r[i] = if idx < 32 { combined[idx] } else { 0 };
    }
    r
}

#[inline]
pub fn i8x16_abs(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = (v[i] as i8).wrapping_abs() as u8;
    }
    r
}

#[inline]
pub fn i8x16_neg(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = (v[i] as i8).wrapping_neg() as u8;
    }
    r
}

#[inline]
pub fn i8x16_popcnt(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = v[i].count_ones() as u8;
    }
    r
}

#[inline]
pub fn i8x16_all_true(v: &[u8; 16]) -> bool {
    v.iter().all(|&b| b != 0)
}

#[inline]
pub fn i8x16_bitmask(v: &[u8; 16]) -> u32 {
    let mut mask = 0u32;
    for i in 0..16 {
        if (v[i] as i8) < 0 {
            mask |= 1 << i;
        }
    }
    mask
}

#[inline]
pub fn i8x16_add(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i].wrapping_add(b[i]);
    }
    r
}

#[inline]
pub fn i8x16_sub(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i].wrapping_sub(b[i]);
    }
    r
}

#[inline]
pub fn i8x16_add_sat_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = (a[i] as i8).saturating_add(b[i] as i8) as u8;
    }
    r
}

#[inline]
pub fn i8x16_add_sat_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i].saturating_add(b[i]);
    }
    r
}

#[inline]
pub fn i8x16_sub_sat_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = (a[i] as i8).saturating_sub(b[i] as i8) as u8;
    }
    r
}

#[inline]
pub fn i8x16_sub_sat_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i].saturating_sub(b[i]);
    }
    r
}

#[inline]
pub fn i8x16_min_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        let va = a[i] as i8;
        let vb = b[i] as i8;
        r[i] = if va < vb { va } else { vb } as u8;
    }
    r
}

#[inline]
pub fn i8x16_min_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i].min(b[i]);
    }
    r
}

#[inline]
pub fn i8x16_max_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        let va = a[i] as i8;
        let vb = b[i] as i8;
        r[i] = if va > vb { va } else { vb } as u8;
    }
    r
}

#[inline]
pub fn i8x16_max_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = a[i].max(b[i]);
    }
    r
}

#[inline]
pub fn i8x16_avgr_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = ((a[i] as u16 + b[i] as u16 + 1) / 2) as u8;
    }
    r
}

#[inline]
pub fn i8x16_shl(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = (shift % 8) as u8;
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = v[i] << s;
    }
    r
}

#[inline]
pub fn i8x16_shr_s(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = (shift % 8) as u8;
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = ((v[i] as i8) >> s) as u8;
    }
    r
}

#[inline]
pub fn i8x16_shr_u(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = (shift % 8) as u8;
    let mut r = [0u8; 16];
    for i in 0..16 {
        r[i] = v[i] >> s;
    }
    r
}

#[inline]
pub fn i8x16_narrow_i16x8_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let va = get_i16(a, i) as i32;
        r[i] = va.max(-128).min(127) as i8 as u8;
    }
    for i in 0..8 {
        let vb = get_i16(b, i) as i32;
        r[i + 8] = vb.max(-128).min(127) as i8 as u8;
    }
    r
}

#[inline]
pub fn i8x16_narrow_i16x8_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let va = get_i16(a, i) as i32;
        r[i] = va.max(0).min(255) as u8;
    }
    for i in 0..8 {
        let vb = get_i16(b, i) as i32;
        r[i + 8] = vb.max(0).min(255) as u8;
    }
    r
}

// ============================================================
// i16x8 operations
// ============================================================

#[inline]
pub fn i16x8_abs(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_i16(&mut r, i, get_i16(v, i).wrapping_abs());
    }
    r
}

#[inline]
pub fn i16x8_neg(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_i16(&mut r, i, get_i16(v, i).wrapping_neg());
    }
    r
}

#[inline]
pub fn i16x8_all_true(v: &[u8; 16]) -> bool {
    for i in 0..8 {
        if get_u16(v, i) == 0 {
            return false;
        }
    }
    true
}

#[inline]
pub fn i16x8_bitmask(v: &[u8; 16]) -> u32 {
    let mut mask = 0u32;
    for i in 0..8 {
        if get_i16(v, i) < 0 {
            mask |= 1 << i;
        }
    }
    mask
}

#[inline]
pub fn i16x8_add(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, get_u16(a, i).wrapping_add(get_u16(b, i)));
    }
    r
}

#[inline]
pub fn i16x8_sub(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, get_u16(a, i).wrapping_sub(get_u16(b, i)));
    }
    r
}

#[inline]
pub fn i16x8_mul(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, get_u16(a, i).wrapping_mul(get_u16(b, i)));
    }
    r
}

#[inline]
pub fn i16x8_add_sat_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_i16(&mut r, i, get_i16(a, i).saturating_add(get_i16(b, i)));
    }
    r
}

#[inline]
pub fn i16x8_add_sat_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, get_u16(a, i).saturating_add(get_u16(b, i)));
    }
    r
}

#[inline]
pub fn i16x8_sub_sat_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_i16(&mut r, i, get_i16(a, i).saturating_sub(get_i16(b, i)));
    }
    r
}

#[inline]
pub fn i16x8_sub_sat_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, get_u16(a, i).saturating_sub(get_u16(b, i)));
    }
    r
}

#[inline]
pub fn i16x8_shl(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = (shift % 16) as u16;
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, get_u16(v, i) << s);
    }
    r
}

#[inline]
pub fn i16x8_shr_s(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = (shift % 16) as u16;
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_i16(&mut r, i, get_i16(v, i) >> s);
    }
    r
}

#[inline]
pub fn i16x8_shr_u(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = (shift % 16) as u16;
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, get_u16(v, i) >> s);
    }
    r
}

#[inline]
pub fn i16x8_min_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let va = get_i16(a, i);
        let vb = get_i16(b, i);
        set_i16(&mut r, i, if va < vb { va } else { vb });
    }
    r
}

#[inline]
pub fn i16x8_min_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, get_u16(a, i).min(get_u16(b, i)));
    }
    r
}

#[inline]
pub fn i16x8_max_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let va = get_i16(a, i);
        let vb = get_i16(b, i);
        set_i16(&mut r, i, if va > vb { va } else { vb });
    }
    r
}

#[inline]
pub fn i16x8_max_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, get_u16(a, i).max(get_u16(b, i)));
    }
    r
}

#[inline]
pub fn i16x8_avgr_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let sum = get_u16(a, i) as u32 + get_u16(b, i) as u32 + 1;
        set_u16(&mut r, i, (sum / 2) as u16);
    }
    r
}

#[inline]
pub fn i16x8_narrow_i32x4_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_i32(a, i) as i64;
        set_i16(&mut r, i, va.max(-32768).min(32767) as i16);
    }
    for i in 0..4 {
        let vb = get_i32(b, i) as i64;
        set_i16(&mut r, i + 4, vb.max(-32768).min(32767) as i16);
    }
    r
}

#[inline]
pub fn i16x8_narrow_i32x4_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_i32(a, i) as i64;
        set_u16(&mut r, i, va.max(0).min(65535) as u16);
    }
    for i in 0..4 {
        let vb = get_i32(b, i) as i64;
        set_u16(&mut r, i + 4, vb.max(0).min(65535) as u16);
    }
    r
}

#[inline]
pub fn i16x8_extend_low_i8x16_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_i16(&mut r, i, v[i] as i8 as i16);
    }
    r
}

#[inline]
pub fn i16x8_extend_low_i8x16_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, v[i] as u16);
    }
    r
}

#[inline]
pub fn i16x8_extend_high_i8x16_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_i16(&mut r, i, v[i + 8] as i8 as i16);
    }
    r
}

#[inline]
pub fn i16x8_extend_high_i8x16_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        set_u16(&mut r, i, v[i + 8] as u16);
    }
    r
}

#[inline]
pub fn i16x8_q15mulr_sat_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let va = get_i16(a, i) as i32;
        let vb = get_i16(b, i) as i32;
        let product = ((va * vb) + 0x4000) >> 15;
        set_i16(&mut r, i, product.max(-32768).min(32767) as i16);
    }
    r
}

#[inline]
pub fn i16x8_extadd_pairwise_i8x16_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let a = v[i * 2] as i8 as i16;
        let b = v[i * 2 + 1] as i8 as i16;
        set_i16(&mut r, i, a + b);
    }
    r
}

#[inline]
pub fn i16x8_extadd_pairwise_i8x16_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let a = v[i * 2] as u16;
        let b = v[i * 2 + 1] as u16;
        set_u16(&mut r, i, a + b);
    }
    r
}

#[inline]
pub fn i16x8_extmul_low_i8x16_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let va = a[i] as i8 as i16;
        let vb = b[i] as i8 as i16;
        set_i16(&mut r, i, va * vb);
    }
    r
}

#[inline]
pub fn i16x8_extmul_high_i8x16_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let va = a[i + 8] as i8 as i16;
        let vb = b[i + 8] as i8 as i16;
        set_i16(&mut r, i, va * vb);
    }
    r
}

#[inline]
pub fn i16x8_extmul_low_i8x16_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let va = a[i] as u16;
        let vb = b[i] as u16;
        set_u16(&mut r, i, va * vb);
    }
    r
}

#[inline]
pub fn i16x8_extmul_high_i8x16_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let va = a[i + 8] as u16;
        let vb = b[i + 8] as u16;
        set_u16(&mut r, i, va * vb);
    }
    r
}

// ============================================================
// i32x4 operations
// ============================================================

#[inline]
pub fn i32x4_abs(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_i32(&mut r, i, get_i32(v, i).wrapping_abs());
    }
    r
}

#[inline]
pub fn i32x4_neg(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_i32(&mut r, i, get_i32(v, i).wrapping_neg());
    }
    r
}

#[inline]
pub fn i32x4_all_true(v: &[u8; 16]) -> bool {
    for i in 0..4 {
        if get_u32(v, i) == 0 {
            return false;
        }
    }
    true
}

#[inline]
pub fn i32x4_bitmask(v: &[u8; 16]) -> u32 {
    let mut mask = 0u32;
    for i in 0..4 {
        if get_i32(v, i) < 0 {
            mask |= 1 << i;
        }
    }
    mask
}

#[inline]
pub fn i32x4_add(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, get_u32(a, i).wrapping_add(get_u32(b, i)));
    }
    r
}

#[inline]
pub fn i32x4_sub(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, get_u32(a, i).wrapping_sub(get_u32(b, i)));
    }
    r
}

#[inline]
pub fn i32x4_mul(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, get_u32(a, i).wrapping_mul(get_u32(b, i)));
    }
    r
}

#[inline]
pub fn i32x4_shl(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = shift % 32;
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, get_u32(v, i) << s);
    }
    r
}

#[inline]
pub fn i32x4_shr_s(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = shift % 32;
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_i32(&mut r, i, get_i32(v, i) >> s);
    }
    r
}

#[inline]
pub fn i32x4_shr_u(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = shift % 32;
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, get_u32(v, i) >> s);
    }
    r
}

#[inline]
pub fn i32x4_min_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_i32(a, i);
        let vb = get_i32(b, i);
        set_i32(&mut r, i, if va < vb { va } else { vb });
    }
    r
}

#[inline]
pub fn i32x4_min_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, get_u32(a, i).min(get_u32(b, i)));
    }
    r
}

#[inline]
pub fn i32x4_max_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_i32(a, i);
        let vb = get_i32(b, i);
        set_i32(&mut r, i, if va > vb { va } else { vb });
    }
    r
}

#[inline]
pub fn i32x4_max_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, get_u32(a, i).max(get_u32(b, i)));
    }
    r
}

#[inline]
pub fn i32x4_dot_i16x8_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let a0 = get_i16(a, i * 2) as i32;
        let a1 = get_i16(a, i * 2 + 1) as i32;
        let b0 = get_i16(b, i * 2) as i32;
        let b1 = get_i16(b, i * 2 + 1) as i32;
        set_i32(&mut r, i, (a0 * b0).wrapping_add(a1 * b1));
    }
    r
}

#[inline]
pub fn i32x4_extend_low_i16x8_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_i32(&mut r, i, get_i16(v, i) as i32);
    }
    r
}

#[inline]
pub fn i32x4_extend_low_i16x8_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, get_u16(v, i) as u32);
    }
    r
}

#[inline]
pub fn i32x4_extend_high_i16x8_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_i32(&mut r, i, get_i16(v, i + 4) as i32);
    }
    r
}

#[inline]
pub fn i32x4_extend_high_i16x8_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_u32(&mut r, i, get_u16(v, i + 4) as u32);
    }
    r
}

#[inline]
pub fn i32x4_extadd_pairwise_i16x8_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let a = get_i16(v, i * 2) as i32;
        let b = get_i16(v, i * 2 + 1) as i32;
        set_i32(&mut r, i, a + b);
    }
    r
}

#[inline]
pub fn i32x4_extadd_pairwise_i16x8_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let a = get_u16(v, i * 2) as u32;
        let b = get_u16(v, i * 2 + 1) as u32;
        set_u32(&mut r, i, a + b);
    }
    r
}

#[inline]
pub fn i32x4_extmul_low_i16x8_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_i16(a, i) as i32;
        let vb = get_i16(b, i) as i32;
        set_i32(&mut r, i, va * vb);
    }
    r
}

#[inline]
pub fn i32x4_extmul_high_i16x8_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_i16(a, i + 4) as i32;
        let vb = get_i16(b, i + 4) as i32;
        set_i32(&mut r, i, va * vb);
    }
    r
}

#[inline]
pub fn i32x4_extmul_low_i16x8_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_u16(a, i) as u32;
        let vb = get_u16(b, i) as u32;
        set_u32(&mut r, i, va * vb);
    }
    r
}

#[inline]
pub fn i32x4_extmul_high_i16x8_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_u16(a, i + 4) as u32;
        let vb = get_u16(b, i + 4) as u32;
        set_u32(&mut r, i, va * vb);
    }
    r
}

// ============================================================
// i64x2 operations
// ============================================================

#[inline]
pub fn i64x2_abs(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_i64(&mut r, i, get_i64(v, i).wrapping_abs());
    }
    r
}

#[inline]
pub fn i64x2_neg(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_i64(&mut r, i, get_i64(v, i).wrapping_neg());
    }
    r
}

#[inline]
pub fn i64x2_all_true(v: &[u8; 16]) -> bool {
    get_u64(v, 0) != 0 && get_u64(v, 1) != 0
}

#[inline]
pub fn i64x2_bitmask(v: &[u8; 16]) -> u32 {
    let mut mask = 0u32;
    if get_i64(v, 0) < 0 { mask |= 1; }
    if get_i64(v, 1) < 0 { mask |= 2; }
    mask
}

#[inline]
pub fn i64x2_add(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_u64(&mut r, i, get_u64(a, i).wrapping_add(get_u64(b, i)));
    }
    r
}

#[inline]
pub fn i64x2_sub(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_u64(&mut r, i, get_u64(a, i).wrapping_sub(get_u64(b, i)));
    }
    r
}

#[inline]
pub fn i64x2_mul(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_u64(&mut r, i, get_u64(a, i).wrapping_mul(get_u64(b, i)));
    }
    r
}

#[inline]
pub fn i64x2_shl(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = (shift % 64) as u64;
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_u64(&mut r, i, get_u64(v, i) << s);
    }
    r
}

#[inline]
pub fn i64x2_shr_s(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = (shift % 64) as u64;
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_i64(&mut r, i, get_i64(v, i) >> s);
    }
    r
}

#[inline]
pub fn i64x2_shr_u(v: &[u8; 16], shift: u32) -> [u8; 16] {
    let s = (shift % 64) as u64;
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_u64(&mut r, i, get_u64(v, i) >> s);
    }
    r
}

#[inline]
pub fn i64x2_extend_low_i32x4_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_i64(&mut r, i, get_i32(v, i) as i64);
    }
    r
}

#[inline]
pub fn i64x2_extend_low_i32x4_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_u64(&mut r, i, get_u32(v, i) as u64);
    }
    r
}

#[inline]
pub fn i64x2_extend_high_i32x4_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_i64(&mut r, i, get_i32(v, i + 2) as i64);
    }
    r
}

#[inline]
pub fn i64x2_extend_high_i32x4_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_u64(&mut r, i, get_u32(v, i + 2) as u64);
    }
    r
}

#[inline]
pub fn i64x2_extmul_low_i32x4_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let va = get_i32(a, i) as i64;
        let vb = get_i32(b, i) as i64;
        set_i64(&mut r, i, va * vb);
    }
    r
}

#[inline]
pub fn i64x2_extmul_high_i32x4_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let va = get_i32(a, i + 2) as i64;
        let vb = get_i32(b, i + 2) as i64;
        set_i64(&mut r, i, va * vb);
    }
    r
}

#[inline]
pub fn i64x2_extmul_low_i32x4_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let va = get_u32(a, i) as u64;
        let vb = get_u32(b, i) as u64;
        set_u64(&mut r, i, va * vb);
    }
    r
}

#[inline]
pub fn i64x2_extmul_high_i32x4_u(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let va = get_u32(a, i + 2) as u64;
        let vb = get_u32(b, i + 2) as u64;
        set_u64(&mut r, i, va * vb);
    }
    r
}

// ============================================================
// Conversion operations
// ============================================================

#[inline]
pub fn i32x4_trunc_sat_f32x4_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let f = get_f32(v, i);
        let val = if f.is_nan() {
            0i32
        } else if f >= i32::MAX as f32 {
            i32::MAX
        } else if f <= i32::MIN as f32 {
            i32::MIN
        } else {
            f as i32
        };
        set_i32(&mut r, i, val);
    }
    r
}

#[inline]
pub fn i32x4_trunc_sat_f32x4_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let f = get_f32(v, i);
        let val = if f.is_nan() || f <= -1.0 {
            0u32
        } else if f >= u32::MAX as f32 {
            u32::MAX
        } else {
            f as u32
        };
        set_u32(&mut r, i, val);
    }
    r
}

#[inline]
pub fn f32x4_convert_i32x4_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_f32(&mut r, i, get_i32(v, i) as f32);
    }
    r
}

#[inline]
pub fn f32x4_convert_i32x4_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_f32(&mut r, i, get_u32(v, i) as f32);
    }
    r
}

#[inline]
pub fn i32x4_trunc_sat_f64x2_s_zero(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let f = get_f64(v, i);
        let val = if f.is_nan() {
            0i32
        } else if f >= i32::MAX as f64 {
            i32::MAX
        } else if f <= i32::MIN as f64 {
            i32::MIN
        } else {
            f as i32
        };
        set_i32(&mut r, i, val);
    }
    r
}

#[inline]
pub fn i32x4_trunc_sat_f64x2_u_zero(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let f = get_f64(v, i);
        let val = if f.is_nan() || f <= -1.0 {
            0u32
        } else if f >= u32::MAX as f64 {
            u32::MAX
        } else {
            f as u32
        };
        set_u32(&mut r, i, val);
    }
    r
}

#[inline]
pub fn f64x2_convert_low_i32x4_s(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_f64(&mut r, i, get_i32(v, i) as f64);
    }
    r
}

#[inline]
pub fn f64x2_convert_low_i32x4_u(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_f64(&mut r, i, get_u32(v, i) as f64);
    }
    r
}

#[inline]
pub fn f32x4_demote_f64x2_zero(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_f32(&mut r, i, canonicalize_f32(get_f64(v, i) as f32));
    }
    r
}

#[inline]
pub fn f64x2_promote_low_f32x4(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_f64(&mut r, i, canonicalize_f64(get_f32(v, i) as f64));
    }
    r
}

// ============================================================
// Ceil/Floor/Trunc/Nearest for floats
// ============================================================

#[inline]
pub fn f32x4_ceil(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_f32(&mut r, i, canonicalize_f32(get_f32(v, i).ceil()));
    }
    r
}

#[inline]
pub fn f32x4_floor(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_f32(&mut r, i, canonicalize_f32(get_f32(v, i).floor()));
    }
    r
}

#[inline]
pub fn f32x4_trunc(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        set_f32(&mut r, i, canonicalize_f32(get_f32(v, i).trunc()));
    }
    r
}

#[inline]
pub fn f32x4_nearest(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let f = get_f32(v, i);
        let result = wasm_nearest_f32(f);
        set_f32(&mut r, i, canonicalize_f32(result));
    }
    r
}

/// WebAssembly nearest (round-to-nearest-even) for f32.
#[inline]
fn wasm_nearest_f32(f: f32) -> f32 {
    if f.is_nan() || f.is_infinite() || f == 0.0 {
        return f;
    }
    let rounded = f.round();
    let result = if (f - f.floor()).abs() == 0.5 {
        if rounded % 2.0 != 0.0 {
            if f > 0.0 { rounded - 1.0 } else { rounded + 1.0 }
        } else {
            rounded
        }
    } else {
        rounded
    };
    if result == 0.0 && f.is_sign_negative() {
        -0.0_f32
    } else {
        result
    }
}

#[inline]
pub fn f64x2_ceil(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_f64(&mut r, i, canonicalize_f64(get_f64(v, i).ceil()));
    }
    r
}

#[inline]
pub fn f64x2_floor(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_f64(&mut r, i, canonicalize_f64(get_f64(v, i).floor()));
    }
    r
}

#[inline]
pub fn f64x2_trunc(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        set_f64(&mut r, i, canonicalize_f64(get_f64(v, i).trunc()));
    }
    r
}

#[inline]
pub fn f64x2_nearest(v: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let f = get_f64(v, i);
        let result = wasm_nearest_f64(f);
        set_f64(&mut r, i, canonicalize_f64(result));
    }
    r
}

/// WebAssembly nearest (round-to-nearest-even) for f64.
#[inline]
fn wasm_nearest_f64(f: f64) -> f64 {
    if f.is_nan() || f.is_infinite() || f == 0.0 {
        return f;
    }
    let rounded = f.round();
    let result = if (f - f.floor()).abs() == 0.5 {
        if rounded % 2.0 != 0.0 {
            if f > 0.0 { rounded - 1.0 } else { rounded + 1.0 }
        } else {
            rounded
        }
    } else {
        rounded
    };
    if result == 0.0 && f.is_sign_negative() {
        -0.0_f64
    } else {
        result
    }
}

// ============================================================
// Relaxed SIMD operations
// ============================================================

/// `i8x16.relaxed_swizzle`: like i8x16.swizzle but out-of-range indices
/// have implementation-defined behavior. We use the same behavior as
/// regular swizzle (return 0 for out-of-range).
#[inline]
pub fn i8x16_relaxed_swizzle(a: &[u8; 16], s: &[u8; 16]) -> [u8; 16] {
    i8x16_swizzle(a, s)
}

/// `i32x4.relaxed_trunc_f32x4_s`: relaxed truncation of f32x4 to i32x4 signed.
/// For NaN and out-of-range, behavior is implementation-defined.
/// We use saturating truncation (same as i32x4.trunc_sat_f32x4_s).
#[inline]
pub fn i32x4_relaxed_trunc_f32x4_s(v: &[u8; 16]) -> [u8; 16] {
    i32x4_trunc_sat_f32x4_s(v)
}

/// `i32x4.relaxed_trunc_f32x4_u`: relaxed truncation of f32x4 to i32x4 unsigned.
/// For NaN and out-of-range, behavior is implementation-defined.
/// We use saturating truncation (same as i32x4.trunc_sat_f32x4_u).
#[inline]
pub fn i32x4_relaxed_trunc_f32x4_u(v: &[u8; 16]) -> [u8; 16] {
    i32x4_trunc_sat_f32x4_u(v)
}

/// `i32x4.relaxed_trunc_f64x2_s_zero`: relaxed truncation of f64x2 to i32x4
/// signed with zero extension. For NaN and out-of-range, behavior is
/// implementation-defined. We use saturating truncation.
#[inline]
pub fn i32x4_relaxed_trunc_f64x2_s_zero(v: &[u8; 16]) -> [u8; 16] {
    i32x4_trunc_sat_f64x2_s_zero(v)
}

/// `i32x4.relaxed_trunc_f64x2_u_zero`: relaxed truncation of f64x2 to i32x4
/// unsigned with zero extension. For NaN and out-of-range, behavior is
/// implementation-defined. We use saturating truncation.
#[inline]
pub fn i32x4_relaxed_trunc_f64x2_u_zero(v: &[u8; 16]) -> [u8; 16] {
    i32x4_trunc_sat_f64x2_u_zero(v)
}

/// `f32x4.relaxed_madd(a, b, c)` = `a * b + c` per lane (fused multiply-add).
#[inline]
pub fn f32x4_relaxed_madd(a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_f32(a, i);
        let vb = get_f32(b, i);
        let vc = get_f32(c, i);
        set_f32(&mut r, i, canonicalize_f32(va.mul_add(vb, vc)));
    }
    r
}

/// `f32x4.relaxed_nmadd(a, b, c)` = `-a * b + c` per lane (negated fused multiply-add).
#[inline]
pub fn f32x4_relaxed_nmadd(a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_f32(a, i);
        let vb = get_f32(b, i);
        let vc = get_f32(c, i);
        set_f32(&mut r, i, canonicalize_f32((-va).mul_add(vb, vc)));
    }
    r
}

/// `f64x2.relaxed_madd(a, b, c)` = `a * b + c` per lane (fused multiply-add).
#[inline]
pub fn f64x2_relaxed_madd(a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let va = get_f64(a, i);
        let vb = get_f64(b, i);
        let vc = get_f64(c, i);
        set_f64(&mut r, i, canonicalize_f64(va.mul_add(vb, vc)));
    }
    r
}

/// `f64x2.relaxed_nmadd(a, b, c)` = `-a * b + c` per lane (negated fused multiply-add).
#[inline]
pub fn f64x2_relaxed_nmadd(a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let va = get_f64(a, i);
        let vb = get_f64(b, i);
        let vc = get_f64(c, i);
        set_f64(&mut r, i, canonicalize_f64((-va).mul_add(vb, vc)));
    }
    r
}

/// Relaxed lane select: for each bit position, if the corresponding bit in `c`
/// is 1, select the bit from `a`, otherwise select the bit from `b`.
/// This is identical to v128.bitselect.
#[inline]
pub fn relaxed_laneselect(a: &[u8; 16], b: &[u8; 16], c: &[u8; 16]) -> [u8; 16] {
    v128_bitselect(a, b, c)
}

/// `f32x4.relaxed_min`: relaxed min where NaN handling is implementation-defined.
/// We return the second operand when the first is NaN (like x86 minps behavior).
#[inline]
pub fn f32x4_relaxed_min(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_f32(a, i);
        let vb = get_f32(b, i);
        set_f32(&mut r, i, if va < vb { va } else { vb });
    }
    r
}

/// `f32x4.relaxed_max`: relaxed max where NaN handling is implementation-defined.
/// We return the second operand when the first is NaN (like x86 maxps behavior).
#[inline]
pub fn f32x4_relaxed_max(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..4 {
        let va = get_f32(a, i);
        let vb = get_f32(b, i);
        set_f32(&mut r, i, if va > vb { va } else { vb });
    }
    r
}

/// `f64x2.relaxed_min`: relaxed min where NaN handling is implementation-defined.
#[inline]
pub fn f64x2_relaxed_min(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let va = get_f64(a, i);
        let vb = get_f64(b, i);
        set_f64(&mut r, i, if va < vb { va } else { vb });
    }
    r
}

/// `f64x2.relaxed_max`: relaxed max where NaN handling is implementation-defined.
#[inline]
pub fn f64x2_relaxed_max(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..2 {
        let va = get_f64(a, i);
        let vb = get_f64(b, i);
        set_f64(&mut r, i, if va > vb { va } else { vb });
    }
    r
}

/// `i16x8.relaxed_q15mulr_s`: relaxed Q15 fixed-point multiply with rounding.
/// Same as i16x8.q15mulr_sat_s but behavior for i16::MIN * i16::MIN is
/// implementation-defined.
#[inline]
pub fn i16x8_relaxed_q15mulr_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    i16x8_q15mulr_sat_s(a, b)
}

/// `i16x8.relaxed_dot_i8x16_i7x16_s`: dot product of signed i8 and unsigned-clamped
/// i7 (i.e., signed i8 treated as 0..127) lanes, producing i16 results.
/// Each pair of i8 lanes is multiplied and adjacent products are added.
#[inline]
pub fn i16x8_relaxed_dot_i8x16_i7x16_s(a: &[u8; 16], b: &[u8; 16]) -> [u8; 16] {
    let mut r = [0u8; 16];
    for i in 0..8 {
        let a0 = a[i * 2] as i8 as i16;
        let a1 = a[i * 2 + 1] as i8 as i16;
        let b0 = b[i * 2] as i8 as i16;
        let b1 = b[i * 2 + 1] as i8 as i16;
        set_i16(&mut r, i, (a0 * b0).wrapping_add(a1 * b1));
    }
    r
}

/// `i32x4.relaxed_dot_i8x16_i7x16_add_s`: dot product of signed i8 and i7 lanes
/// producing i32 results with accumulator addition. Groups of 4 i8 lanes are
/// multiplied pairwise with i7 lanes and accumulated into i32 lanes.
#[inline]
pub fn i32x4_relaxed_dot_i8x16_i7x16_add_s(
    a: &[u8; 16],
    b: &[u8; 16],
    c: &[u8; 16],
) -> [u8; 16] {
    // First compute i16x8 dot products
    let mut intermediate = [0i16; 8];
    for i in 0..8 {
        let a0 = a[i * 2] as i8 as i16;
        let a1 = a[i * 2 + 1] as i8 as i16;
        let b0 = b[i * 2] as i8 as i16;
        let b1 = b[i * 2 + 1] as i8 as i16;
        intermediate[i] = (a0 * b0).wrapping_add(a1 * b1);
    }
    // Then pairwise add to i32x4 and add accumulator
    let mut r = [0u8; 16];
    for i in 0..4 {
        let sum = intermediate[i * 2] as i32 + intermediate[i * 2 + 1] as i32;
        let acc = get_i32(c, i);
        set_i32(&mut r, i, sum.wrapping_add(acc));
    }
    r
}
