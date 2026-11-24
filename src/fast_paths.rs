use std::cell::RefCell;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const FNV_PRIME_2: u64 = FNV_PRIME.wrapping_mul(FNV_PRIME);
const FNV_PRIME_4: u64 = FNV_PRIME_2.wrapping_mul(FNV_PRIME_2);
const FNV_PRIME_8: u64 = FNV_PRIME_4.wrapping_mul(FNV_PRIME_4);

thread_local! {
    static ZSTD_COMPRESSOR_L1: RefCell<zstd::bulk::Compressor<'static>> =
        RefCell::new(zstd::bulk::Compressor::new(1).expect("zstd level 1 compressor"));
    static ZSTD_COMPRESSOR_L3: RefCell<zstd::bulk::Compressor<'static>> =
        RefCell::new(zstd::bulk::Compressor::new(3).expect("zstd level 3 compressor"));
    static ZSTD_DECOMPRESSOR: RefCell<zstd::bulk::Decompressor<'static>> =
        RefCell::new(zstd::bulk::Decompressor::new().expect("zstd decompressor"));
}

#[inline]
pub fn zstd_compress(bytes: &[u8], level: i32) -> Result<Vec<u8>, std::io::Error> {
    let cached = match level {
        1 => ZSTD_COMPRESSOR_L1.try_with(|cell| cell.borrow_mut().compress(bytes)),
        3 => ZSTD_COMPRESSOR_L3.try_with(|cell| cell.borrow_mut().compress(bytes)),
        _ => return zstd::bulk::compress(bytes, level),
    };
    cached.unwrap_or_else(|_| zstd::bulk::compress(bytes, level))
}

/// Compresses `width` contiguous byte planes of length `plane_len` into a
/// single standard RFC 8878 Zstd frame, flushing at each plane boundary so each
/// plane receives its own Zstd block(s) and Huffman/RLE/Raw block selection.
pub fn zstd_compress_planes_flushed_bounded(
    planes_flat: &[u8],
    plane_len: usize,
    width: usize,
    level: i32,
    max_encoded_len: usize,
) -> Option<Vec<u8>> {
    use zstd::stream::raw::{Encoder, InBuffer, Operation, OutBuffer};

    if plane_len == 0 || width == 0 || max_encoded_len == 0 {
        return None;
    }
    debug_assert_eq!(planes_flat.len(), plane_len * width);
    if planes_flat.len() != plane_len * width {
        return None;
    }

    let run_with_ctx = |compressor: &mut zstd::bulk::Compressor<'static>| -> Option<Vec<u8>> {
        let mut enc = Encoder::with_context(compressor.context_mut());
        enc.set_pledged_src_size(Some(planes_flat.len() as u64))
            .ok()?;
        let init_cap = max_encoded_len
            .saturating_add(256)
            .min(planes_flat.len().saturating_add(256))
            .min(262_144);
        let mut out = Vec::with_capacity(init_cap.max(4096));

        let ensure_room = |out: &mut Vec<u8>| -> bool {
            let len = out.len();
            if len > max_encoded_len {
                return false;
            }
            if out.capacity() == len {
                let target = max_encoded_len.saturating_add(4096);
                let grow = (len.max(4096)).min(target.saturating_sub(len));
                if grow == 0 {
                    out.reserve(64);
                } else {
                    out.reserve(grow);
                }
            }
            true
        };

        for p in 0..width {
            let plane = &planes_flat[p * plane_len..(p + 1) * plane_len];
            let mut in_buf = InBuffer::around(plane);
            while in_buf.pos() < plane.len() {
                if !ensure_room(&mut out) {
                    return None;
                }
                let pos = out.len();
                let res = {
                    let mut ob = OutBuffer::around_pos(&mut out, pos);
                    enc.run(&mut in_buf, &mut ob)
                };
                if res.is_err() || out.len() > max_encoded_len {
                    return None;
                }
            }
            if p + 1 < width {
                loop {
                    if !ensure_room(&mut out) {
                        return None;
                    }
                    let pos = out.len();
                    let remaining = {
                        let mut ob = OutBuffer::around_pos(&mut out, pos);
                        enc.flush(&mut ob)
                    }
                    .ok()?;
                    if out.len() > max_encoded_len {
                        return None;
                    }
                    if remaining == 0 {
                        break;
                    }
                }
            } else {
                loop {
                    if !ensure_room(&mut out) {
                        return None;
                    }
                    let pos = out.len();
                    let remaining = {
                        let mut ob = OutBuffer::around_pos(&mut out, pos);
                        enc.finish(&mut ob, true)
                    }
                    .ok()?;
                    if out.len() > max_encoded_len {
                        return None;
                    }
                    if remaining == 0 {
                        break;
                    }
                }
            }
        }
        Some(out)
    };

    let cached = match level {
        1 => ZSTD_COMPRESSOR_L1
            .try_with(|cell| run_with_ctx(&mut cell.borrow_mut()))
            .ok(),
        3 => ZSTD_COMPRESSOR_L3
            .try_with(|cell| run_with_ctx(&mut cell.borrow_mut()))
            .ok(),
        _ => None,
    };
    cached.unwrap_or_else(|| run_with_ctx(&mut zstd::bulk::Compressor::new(level).ok()?))
}

#[inline]
pub fn zstd_decompress(bytes: &[u8], capacity: usize) -> Result<Vec<u8>, std::io::Error> {
    // A caller's thread-local destructor may run after this cache is dropped.
    ZSTD_DECOMPRESSOR
        .try_with(|cell| cell.borrow_mut().decompress(bytes, capacity))
        .unwrap_or_else(|_| zstd::bulk::decompress(bytes, capacity))
}

#[inline]
pub fn fnv1a_update_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ (byte as u64)).wrapping_mul(FNV_PRIME)
}

#[inline]
pub fn fnv1a_update_4(mut hash: u64, b0: u8, b1: u8, b2: u8, b3: u8) -> u64 {
    hash = (hash ^ (b0 as u64)).wrapping_mul(FNV_PRIME);
    hash = (hash ^ (b1 as u64)).wrapping_mul(FNV_PRIME);
    hash = (hash ^ (b2 as u64)).wrapping_mul(FNV_PRIME);
    (hash ^ (b3 as u64)).wrapping_mul(FNV_PRIME)
}

#[inline]
pub fn fnv1a_update_2_with_2_zeros(mut hash: u64, b0: u8, b1: u8) -> u64 {
    hash = (hash ^ (b0 as u64)).wrapping_mul(FNV_PRIME);
    hash = (hash ^ (b1 as u64)).wrapping_mul(FNV_PRIME);
    hash.wrapping_mul(FNV_PRIME_2)
}

#[inline]
pub fn fnv1a_update_u32_be(hash: u64, bits: u32) -> u64 {
    if bits == 0 {
        hash.wrapping_mul(FNV_PRIME_4)
    } else if (bits & 0xffff) == 0 {
        fnv1a_update_2_with_2_zeros(hash, (bits >> 24) as u8, (bits >> 16) as u8)
    } else {
        let b = bits.to_be_bytes();
        fnv1a_update_4(hash, b[0], b[1], b[2], b[3])
    }
}

#[inline]
fn fnv1a_update_8_branchless(mut hash: u64, chunk: &[u8]) -> u64 {
    hash = fnv1a_update_4(hash, chunk[0], chunk[1], chunk[2], chunk[3]);
    fnv1a_update_4(hash, chunk[4], chunk[5], chunk[6], chunk[7])
}

pub fn checksum_bytes_fast(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    let has_zero_words = if bytes.len() >= 128 {
        let word_count = bytes.len() / 8;
        let mut zeros = false;
        for s in 0..16 {
            let off = (s * word_count / 16) * 8;
            if u64::from_ne_bytes(bytes[off..off + 8].try_into().expect("8-byte sample")) == 0 {
                zeros = true;
                break;
            }
        }
        zeros
    } else {
        false
    };

    let mut chunks = bytes.chunks_exact(8);
    if has_zero_words {
        for chunk in &mut chunks {
            if u64::from_ne_bytes(chunk.try_into().expect("8-byte chunk")) == 0 {
                hash = hash.wrapping_mul(FNV_PRIME_8);
            } else {
                hash = fnv1a_update_8_branchless(hash, chunk);
            }
        }
    } else {
        for chunk in &mut chunks {
            hash = fnv1a_update_8_branchless(hash, chunk);
        }
    }
    for &byte in chunks.remainder() {
        hash = fnv1a_update_byte(hash, byte);
    }
    hash
}

pub fn checksum_f32_bits_fast(values: &[f32]) -> u64 {
    let mut hash = FNV_OFFSET;
    let has_zero_low16 = if values.len() >= 64 {
        let mut found = false;
        for s in 0..16 {
            let idx = s * values.len() / 16;
            if (values[idx].to_bits() & 0xffff) == 0 {
                found = true;
                break;
            }
        }
        found
    } else {
        false
    };

    if has_zero_low16 {
        for &value in values {
            hash = fnv1a_update_u32_be(hash, value.to_bits());
        }
    } else {
        for &value in values {
            let b = value.to_bits().to_be_bytes();
            hash = fnv1a_update_4(hash, b[0], b[1], b[2], b[3]);
        }
    }
    hash
}

/// Converts big-endian 32-bit float bytes directly into native `Vec<f32>`
/// and computes the FNV-1a checksum.
pub fn bytes_be_to_f32_and_checksum(body: &[u8]) -> (Vec<f32>, u64) {
    debug_assert_eq!(body.len() % 4, 0);
    let value_count = body.len() / 4;
    let mut values: Vec<f32> = Vec::with_capacity(value_count);
    let mut i = 0_usize;

    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    unsafe {
        use std::arch::aarch64::{vld1q_u8, vrev32q_u8, vst1q_u8};
        let src = body.as_ptr();
        let dst = values.as_mut_ptr() as *mut u8;
        while i + 4 <= value_count {
            let v = vld1q_u8(src.add(i * 4));
            vst1q_u8(dst.add(i * 4), vrev32q_u8(v));
            i += 4;
        }
        values.set_len(i);
    }

    while i < value_count {
        let off = i * 4;
        let bits = u32::from_be_bytes([body[off], body[off + 1], body[off + 2], body[off + 3]]);
        values.push(f32::from_bits(bits));
        i += 1;
    }

    let checksum = checksum_bytes_fast(body);
    (values, checksum)
}

/// Byte-swaps big-endian 16-bit elements into little-endian bytes and computes
/// the FNV-1a checksum of the little-endian bytes.
pub fn swap_be_to_u16_le_and_checksum(body: &[u8]) -> (Vec<u8>, u64) {
    debug_assert_eq!(body.len() % 2, 0);
    let len = body.len();
    let mut out = vec![0_u8; len];
    let mut i = 0_usize;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::aarch64::{vld1q_u8, vrev16q_u8, vst1q_u8};
        let src = body.as_ptr();
        let dst = out.as_mut_ptr();
        while i + 16 <= len {
            let v = vld1q_u8(src.add(i));
            vst1q_u8(dst.add(i), vrev16q_u8(v));
            i += 16;
        }
    }

    while i < len {
        out[i] = body[i + 1];
        out[i + 1] = body[i];
        i += 2;
    }

    let checksum = checksum_bytes_fast(&out);
    (out, checksum)
}

/// Transposes little-endian 16-bit elements (`[lo, hi]`) directly into two
/// contiguous big-endian planes `[plane_hi | plane_lo]`.
pub fn transpose_u16_le_to_be_planes(bytes_le: &[u8]) -> Vec<u8> {
    debug_assert_eq!(bytes_le.len() % 2, 0);
    let element_count = bytes_le.len() / 2;
    let mut planes = vec![0_u8; bytes_le.len()];
    let (p0, p1) = planes.split_at_mut(element_count);
    transpose_u16_le_to_be_plane_slices(bytes_le, p0, p1);
    planes
}

pub fn transpose_u16_le_to_be_plane_slices(bytes_le: &[u8], p0: &mut [u8], p1: &mut [u8]) {
    let n = p0.len();
    debug_assert_eq!(p1.len(), n);
    debug_assert_eq!(bytes_le.len(), n * 2);

    let mut i = 0_usize;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::aarch64::{vld2q_u8, vst1q_u8};
        let src = bytes_le.as_ptr();
        let d0 = p0.as_mut_ptr();
        let d1 = p1.as_mut_ptr();
        while i + 16 <= n {
            let pair = vld2q_u8(src.add(i * 2));
            // `bytes_le` has [lo, hi], so pair.0 = lo -> p1, pair.1 = hi -> p0.
            vst1q_u8(d0.add(i), pair.1);
            vst1q_u8(d1.add(i), pair.0);
            i += 16;
        }
    }

    while i + 4 <= n {
        let s = i * 2;
        p0[i] = bytes_le[s + 1];
        p0[i + 1] = bytes_le[s + 3];
        p0[i + 2] = bytes_le[s + 5];
        p0[i + 3] = bytes_le[s + 7];
        p1[i] = bytes_le[s];
        p1[i + 1] = bytes_le[s + 2];
        p1[i + 2] = bytes_le[s + 4];
        p1[i + 3] = bytes_le[s + 6];
        i += 4;
    }
    while i < n {
        p0[i] = bytes_le[i * 2 + 1];
        p1[i] = bytes_le[i * 2];
        i += 1;
    }
}

/// Transposes little-endian 16-bit elements with `word.rotate_left(1)` into
/// two big-endian byte planes `[p0 | p1]`.
#[cfg(test)]
pub fn transpose_u16_le_rot1_planes(bytes_le: &[u8], p0: &mut [u8], p1: &mut [u8]) {
    let n = p0.len();
    debug_assert_eq!(p1.len(), n);
    debug_assert_eq!(bytes_le.len(), n * 2);
    let mut i = 0_usize;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::aarch64::{vld2q_u8, vorrq_u8, vshlq_n_u8, vshrq_n_u8, vst1q_u8};
        let src = bytes_le.as_ptr();
        let d0 = p0.as_mut_ptr();
        let d1 = p1.as_mut_ptr();
        while i + 16 <= n {
            let pair = vld2q_u8(src.add(i * 2));
            let lo = pair.0;
            let hi = pair.1;
            let r0 = vorrq_u8(vshlq_n_u8::<1>(hi), vshrq_n_u8::<7>(lo));
            let r1 = vorrq_u8(vshlq_n_u8::<1>(lo), vshrq_n_u8::<7>(hi));
            vst1q_u8(d0.add(i), r0);
            vst1q_u8(d1.add(i), r1);
            i += 16;
        }
    }

    while i < n {
        let w = u16::from_le_bytes([bytes_le[i * 2], bytes_le[i * 2 + 1]]).rotate_left(1);
        let [b0, b1] = w.to_be_bytes();
        p0[i] = b0;
        p1[i] = b1;
        i += 1;
    }
}

/// Untransposes two big-endian byte planes `[p0 | p1]` directly into
/// little-endian 16-bit bytes `[lo, hi]` and computes the FNV-1a checksum.
pub fn untranspose_be_planes_to_u16_le_and_checksum(
    p0: &[u8],
    p1: &[u8],
    rot1: bool,
) -> (Vec<u8>, u64) {
    let n = p0.len();
    debug_assert_eq!(p1.len(), n);
    let mut out = vec![0_u8; n * 2];
    let mut i = 0_usize;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::aarch64::{
            uint8x16x2_t, vld1q_u8, vorrq_u8, vshlq_n_u8, vshrq_n_u8, vst2q_u8,
        };
        let s0 = p0.as_ptr();
        let s1 = p1.as_ptr();
        let dst = out.as_mut_ptr();
        while i + 16 <= n {
            let v0 = vld1q_u8(s0.add(i));
            let v1 = vld1q_u8(s1.add(i));
            let (hi, lo) = if rot1 {
                (
                    vorrq_u8(vshrq_n_u8::<1>(v0), vshlq_n_u8::<7>(v1)),
                    vorrq_u8(vshrq_n_u8::<1>(v1), vshlq_n_u8::<7>(v0)),
                )
            } else {
                (v0, v1)
            };
            vst2q_u8(dst.add(i * 2), uint8x16x2_t(lo, hi));
            i += 16;
        }
    }

    while i < n {
        if rot1 {
            let w = u16::from_be_bytes([p0[i], p1[i]]).rotate_right(1);
            let [lo, hi] = w.to_le_bytes();
            out[i * 2] = lo;
            out[i * 2 + 1] = hi;
        } else {
            out[i * 2] = p1[i];
            out[i * 2 + 1] = p0[i];
        }
        i += 1;
    }

    let checksum = checksum_bytes_fast(&out);
    (out, checksum)
}

/// Applies `rotate_left(1)` to the high 16 bits `[p0, p1]` of a 4-plane f32 layout.
#[cfg(test)]
pub fn rot1_hi16_planes(src_p0: &[u8], src_p1: &[u8], dst_p0: &mut [u8], dst_p1: &mut [u8]) {
    let n = src_p0.len();
    debug_assert_eq!(src_p1.len(), n);
    debug_assert_eq!(dst_p0.len(), n);
    debug_assert_eq!(dst_p1.len(), n);
    let mut i = 0_usize;

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::aarch64::{vld1q_u8, vorrq_u8, vshlq_n_u8, vshrq_n_u8, vst1q_u8};
        while i + 16 <= n {
            let hi = vld1q_u8(src_p0.as_ptr().add(i));
            let lo = vld1q_u8(src_p1.as_ptr().add(i));
            vst1q_u8(
                dst_p0.as_mut_ptr().add(i),
                vorrq_u8(vshlq_n_u8::<1>(hi), vshrq_n_u8::<7>(lo)),
            );
            vst1q_u8(
                dst_p1.as_mut_ptr().add(i),
                vorrq_u8(vshlq_n_u8::<1>(lo), vshrq_n_u8::<7>(hi)),
            );
            i += 16;
        }
    }

    while i < n {
        let b0 = src_p0[i];
        let b1 = src_p1[i];
        dst_p0[i] = (b0 << 1) | (b1 >> 7);
        dst_p1[i] = (b1 << 1) | (b0 >> 7);
        i += 1;
    }
}

/// Transposes `&[f32]` directly into four contiguous big-endian byte planes
/// `[p0 | p1 | p2 | p3]` and returns the FNV-1a checksum of the big-endian
/// f32 bitstream.
pub fn transpose_f32_to_be_planes_and_checksum(values: &[f32]) -> (Vec<u8>, u64) {
    let n = values.len();
    let mut planes = vec![0_u8; n * 4];
    let (p0, rest) = planes.split_at_mut(n);
    let (p1, rest) = rest.split_at_mut(n);
    let (p2, p3) = rest.split_at_mut(n);

    let mut i = 0_usize;
    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    unsafe {
        use std::arch::aarch64::{vld4q_u8, vst1q_u8};
        let src = values.as_ptr() as *const u8;
        let d0 = p0.as_mut_ptr();
        let d1 = p1.as_mut_ptr();
        let d2 = p2.as_mut_ptr();
        let d3 = p3.as_mut_ptr();
        while i + 16 <= n {
            let q = vld4q_u8(src.add(i * 4));
            vst1q_u8(d0.add(i), q.3);
            vst1q_u8(d1.add(i), q.2);
            vst1q_u8(d2.add(i), q.1);
            vst1q_u8(d3.add(i), q.0);
            i += 16;
        }
    }

    while i < n {
        let [b0, b1, b2, b3] = values[i].to_bits().to_be_bytes();
        p0[i] = b0;
        p1[i] = b1;
        p2[i] = b2;
        p3[i] = b3;
        i += 1;
    }

    let checksum = checksum_f32_bits_fast(values);
    (planes, checksum)
}

/// Untransposes four big-endian byte planes directly into `Vec<f32>` and
/// computes the FNV-1a checksum in a single combined pass.
pub fn untranspose_be_planes_to_f32_and_checksum(
    p0: &[u8],
    p1: &[u8],
    p2: &[u8],
    p3: &[u8],
    rot1_hi16: bool,
) -> (Vec<f32>, u64) {
    let n = p0.len();
    debug_assert_eq!(p1.len(), n);
    debug_assert_eq!(p2.len(), n);
    debug_assert_eq!(p3.len(), n);
    let mut values: Vec<f32> = Vec::with_capacity(n);
    let mut i = 0_usize;

    #[cfg(all(target_arch = "aarch64", target_endian = "little"))]
    unsafe {
        use std::arch::aarch64::{
            uint8x16x4_t, vld1q_u8, vorrq_u8, vshlq_n_u8, vshrq_n_u8, vst4q_u8,
        };
        let s0 = p0.as_ptr();
        let s1 = p1.as_ptr();
        let s2 = p2.as_ptr();
        let s3 = p3.as_ptr();
        let dst = values.as_mut_ptr() as *mut u8;
        while i + 16 <= n {
            let mut b0 = vld1q_u8(s0.add(i));
            let mut b1 = vld1q_u8(s1.add(i));
            if rot1_hi16 {
                let r0 = vorrq_u8(vshrq_n_u8::<1>(b0), vshlq_n_u8::<7>(b1));
                let r1 = vorrq_u8(vshrq_n_u8::<1>(b1), vshlq_n_u8::<7>(b0));
                b0 = r0;
                b1 = r1;
            }
            let b2 = vld1q_u8(s2.add(i));
            let b3 = vld1q_u8(s3.add(i));
            vst4q_u8(dst.add(i * 4), uint8x16x4_t(b3, b2, b1, b0));
            i += 16;
        }
        values.set_len(i);
    }

    while i < n {
        let (b0, b1) = if rot1_hi16 {
            ((p0[i] >> 1) | (p1[i] << 7), (p1[i] >> 1) | (p0[i] << 7))
        } else {
            (p0[i], p1[i])
        };
        let bits = u32::from_be_bytes([b0, b1, p2[i], p3[i]]);
        values.push(f32::from_bits(bits));
        i += 1;
    }

    let checksum = checksum_f32_bits_fast(&values);
    (values, checksum)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaneStat {
    Zero,
    Repeat(u8),
    Varying,
}

#[inline]
pub fn classify_plane(plane: &[u8]) -> PlaneStat {
    let Some(&first) = plane.first() else {
        return PlaneStat::Zero;
    };
    if plane.iter().all(|&b| b == first) {
        if first == 0 {
            PlaneStat::Zero
        } else {
            PlaneStat::Repeat(first)
        }
    } else {
        PlaneStat::Varying
    }
}

/// Fast distributed sample run-density check for contiguous bytes.
/// Returns `true` if enough zero or repeated bytes appear in the sample that
/// RLE has a realistic chance of beating raw/Zstd candidates.
pub fn sample_has_rle_potential(bytes: &[u8], threshold_percent: usize) -> bool {
    let len = bytes.len();
    if len < 512 {
        return true;
    }
    let blocks = if len <= 8_192 { 4_usize } else { 32_usize };
    const BLOCK_LEN: usize = 64;
    let mut run_bytes = 0_usize;
    let mut total = 0_usize;
    for block in 0..blocks {
        let start = block * len / blocks;
        let end = (start + BLOCK_LEN).min(len);
        let slice = &bytes[start..end];
        let mut prev = None;
        for &b in slice {
            if b == 0 || prev == Some(b) {
                run_bytes += 1;
            }
            prev = Some(b);
            total += 1;
        }
    }
    run_bytes.saturating_mul(100) >= total.saturating_mul(threshold_percent)
}

/// Fast distributed sample check for `delta-xor-byte-plane-rle` on `f32`.
pub fn sample_has_delta_xor_rle_potential(values: &[f32]) -> bool {
    let len = values.len();
    if len <= 2_048 {
        return true;
    }
    const BLOCKS: usize = 16;
    const BLOCK_LEN: usize = 32;
    let mut rle_bytes = 0_usize;
    let mut total_bytes = 0_usize;
    for block in 0..BLOCKS {
        let start = (block * len / BLOCKS).max(1);
        let end = (start + BLOCK_LEN).min(len);
        let mut prev_delta: Option<[u8; 4]> = None;
        for i in start..end {
            let delta = (values[i].to_bits() ^ values[i - 1].to_bits()).to_be_bytes();
            for (lane, &b) in delta.iter().enumerate() {
                if b == 0 || prev_delta.is_some_and(|p| p[lane] == b) {
                    rle_bytes += 1;
                }
            }
            prev_delta = Some(delta);
            total_bytes += 4;
        }
    }
    rle_bytes.saturating_mul(100) >= total_bytes.saturating_mul(30)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zstd_roundtrips(input: &[u8]) -> bool {
        [1, 3].into_iter().all(|level| {
            let Ok(bulk) = zstd_compress(input, level) else {
                return false;
            };
            let Some(flushed) =
                zstd_compress_planes_flushed_bounded(input, input.len() / 2, 2, level, 256)
            else {
                return false;
            };
            [bulk, flushed].iter().all(|frame| {
                zstd_decompress(frame, input.len()).is_ok_and(|decoded| decoded == input)
            })
        })
    }

    #[test]
    fn zstd_roundtrips_after_cached_contexts_are_destroyed() {
        struct EncodeDecodeOnDrop(std::sync::mpsc::Sender<bool>);

        const PLANES: [u8; 128] = [42; 128];

        impl Drop for EncodeDecodeOnDrop {
            fn drop(&mut self) {
                let _ = self.0.send(zstd_roundtrips(&PLANES));
            }
        }

        thread_local! {
            static GUARD: RefCell<Option<EncodeDecodeOnDrop>> = const { RefCell::new(None) };
        }

        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // Initialize the guard first so its destructor runs after the Zstd caches.
            GUARD.with(|guard| *guard.borrow_mut() = Some(EncodeDecodeOnDrop(send)));
            assert!(zstd_roundtrips(&PLANES));
        })
        .join()
        .unwrap();
        assert_eq!(receive.recv(), Ok(true));
    }
}
