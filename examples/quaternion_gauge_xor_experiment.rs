use std::{
    env, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use qatq::{TensorDType, decode_qatq_exact_tensor_le, try_encode_qatq_exact_tensor_le};

const LEVEL: i32 = 3;
const GAUGE_COUNT: usize = 8;
const GAUGE_METADATA_BYTES: usize = 8;
const ITERATIONS: usize = 51;
const SAMPLE_QUATERNIONS: usize = 1024;
const GATE_PERCENT: usize = 95;
const GAUGE_BLOCK_QUATERNIONS: usize = 256;
const ORIENTATION_SAMPLES_PER_BLOCK: usize = 4;

#[derive(Clone)]
struct Dataset {
    name: String,
    bytes: Vec<u8>,
}

struct GaugePayload {
    value_count: usize,
    orientation_bytes: Vec<u8>,
    residual_bytes: Vec<u8>,
}

struct ResultRow {
    name: String,
    raw_bytes: usize,
    qatq_bytes: usize,
    gauge_candidate_bytes: usize,
    selected: bool,
    qatq_encode_ns_per_value: f64,
    gauge_encode_ns_per_value: f64,
    qatq_decode_ns_per_value: f64,
    gauge_decode_ns_per_value: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut datasets = synthetic_datasets();
    let args: Vec<String> = env::args().skip(1).collect();
    if !args.len().is_multiple_of(2) {
        return Err(
            "usage: quaternion_gauge_xor_experiment [--input label:path] [--input-dir label:path]"
                .into(),
        );
    }
    for pair in args.chunks_exact(2) {
        let (label, path) = parse_label_path(&pair[1])?;
        match pair[0].as_str() {
            "--input" => datasets.push(Dataset {
                name: label,
                bytes: fs::read(path)?,
            }),
            "--input-dir" => datasets.push(load_directory(&label, &path)?),
            option => return Err(format!("unknown option {option}").into()),
        }
    }

    let mut rows = Vec::new();
    for dataset in &datasets {
        rows.push(evaluate(dataset)?);
    }
    print_rows(&rows);
    Ok(())
}

fn evaluate(dataset: &Dataset) -> Result<ResultRow, Box<dyn std::error::Error>> {
    if !dataset.bytes.len().is_multiple_of(2) {
        return Err(format!("{} has an odd byte length", dataset.name).into());
    }
    let value_count = dataset.bytes.len() / 2;
    let qatq = try_encode_qatq_exact_tensor_le(&dataset.bytes, TensorDType::BF16)?;
    let qatq_decoded = decode_qatq_exact_tensor_le(&qatq)?;
    if qatq_decoded.bytes_le != dataset.bytes {
        return Err(format!("{} failed QATQ exact round trip", dataset.name).into());
    }

    let gauge = encode_gauge(&dataset.bytes)?;
    let gauge_decoded = decode_gauge(&gauge)?;
    if gauge_decoded != dataset.bytes {
        return Err(format!("{} failed gauge exact round trip", dataset.name).into());
    }
    let gauge_candidate_bytes = gauge_size(&gauge);
    let selected = gauge_sample_gate(&dataset.bytes)?
        && gauge_candidate_bytes.saturating_mul(100) <= qatq.len().saturating_mul(GATE_PERCENT);

    let qatq_encode_ns = median_timing(|| {
        try_encode_qatq_exact_tensor_le(&dataset.bytes, TensorDType::BF16)
            .expect("QATQ exact encode")
    });
    let gauge_encode_ns =
        median_timing(|| encode_gauge_decision(&dataset.bytes).expect("gauge encode decision"));
    let qatq_decode_ns =
        median_timing(|| decode_qatq_exact_tensor_le(&qatq).expect("QATQ exact decode"));
    let gauge_decode_ns = median_timing(|| decode_gauge(&gauge).expect("gauge exact decode"));

    Ok(ResultRow {
        name: dataset.name.clone(),
        raw_bytes: dataset.bytes.len(),
        qatq_bytes: qatq.len(),
        gauge_candidate_bytes,
        selected,
        qatq_encode_ns_per_value: qatq_encode_ns / value_count as f64,
        gauge_encode_ns_per_value: gauge_encode_ns / value_count as f64,
        qatq_decode_ns_per_value: qatq_decode_ns / value_count as f64,
        gauge_decode_ns_per_value: gauge_decode_ns / value_count as f64,
    })
}

fn encode_gauge_decision(bytes: &[u8]) -> Result<Option<GaugePayload>, Box<dyn std::error::Error>> {
    if !gauge_sample_gate(bytes)? {
        return Ok(None);
    }
    let gauge = encode_gauge(bytes)?;
    let qatq = try_encode_qatq_exact_tensor_le(bytes, TensorDType::BF16)?;
    Ok(
        (gauge_size(&gauge).saturating_mul(100) <= qatq.len().saturating_mul(GATE_PERCENT))
            .then_some(gauge),
    )
}

fn encode_gauge(bytes: &[u8]) -> Result<GaugePayload, Box<dyn std::error::Error>> {
    let words = le_bytes_to_words(bytes)?;
    let quaternion_count = words.len().div_ceil(4);
    let block_count = quaternion_count.div_ceil(GAUGE_BLOCK_QUATERNIONS);
    let mut orientations = Vec::with_capacity(block_count);
    let mut residual_words = Vec::with_capacity(quaternion_count * 4);
    let mut previous = [0_u16; 4];

    for gauge_block_start in (0..quaternion_count).step_by(GAUGE_BLOCK_QUATERNIONS) {
        let gauge_block_end = (gauge_block_start + GAUGE_BLOCK_QUATERNIONS).min(quaternion_count);
        let orientation = best_block_orientation(&words, gauge_block_start, gauge_block_end);
        orientations.push(orientation);
        for quaternion_index in gauge_block_start..gauge_block_end {
            let current = quaternion_at(&words, quaternion_index);
            let aligned = apply_gauge(orientation, current);
            residual_words.extend_from_slice(&xor_quaternion(aligned, previous));
            previous = current;
        }
    }

    let packed_orientations = pack_orientations(&orientations);
    let orientation_bytes = zstd::bulk::compress(&packed_orientations, LEVEL)?;
    let residual_planes = byte_plane_words(&residual_words);
    let residual_bytes = zstd::bulk::compress(&residual_planes, LEVEL)?;
    Ok(GaugePayload {
        value_count: words.len(),
        orientation_bytes,
        residual_bytes,
    })
}

fn decode_gauge(payload: &GaugePayload) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let quaternion_count = payload.value_count.div_ceil(4);
    let block_count = quaternion_count.div_ceil(GAUGE_BLOCK_QUATERNIONS);
    let packed_len = block_count.saturating_mul(3).div_ceil(8);
    let packed_orientations = zstd::bulk::decompress(&payload.orientation_bytes, packed_len)?;
    let orientations = unpack_orientations(&packed_orientations, block_count)?;
    let residual_word_count = quaternion_count
        .checked_mul(4)
        .ok_or("residual word count overflow")?;
    let residual_plane_bytes = residual_word_count
        .checked_mul(2)
        .ok_or("residual length overflow")?;
    let residual_planes = zstd::bulk::decompress(&payload.residual_bytes, residual_plane_bytes)?;
    let residual_words = words_from_byte_planes(&residual_planes, residual_word_count)?;

    let mut decoded = Vec::with_capacity(residual_word_count);
    let mut previous = [0_u16; 4];
    for (quaternion_index, residual) in residual_words.chunks_exact(4).enumerate() {
        let orientation = orientations[quaternion_index / GAUGE_BLOCK_QUATERNIONS];
        let mut aligned = [0_u16; 4];
        for index in 0..4 {
            aligned[index] = residual[index] ^ previous[index];
        }
        let original = apply_gauge(inverse_orientation(orientation), aligned);
        decoded.extend_from_slice(&original);
        previous = original;
    }
    decoded.truncate(payload.value_count);
    Ok(words_to_le_bytes(&decoded))
}

fn best_orientation(current: [u16; 4], previous: [u16; 4]) -> (u8, [u16; 4]) {
    let mut best_orientation = 0_u8;
    let mut best_residual = xor_quaternion(current, previous);
    let mut best_score = residual_score(best_residual);
    for orientation in 1..GAUGE_COUNT as u8 {
        let aligned = apply_gauge(orientation, current);
        let residual = xor_quaternion(aligned, previous);
        let score = residual_score(residual);
        if score < best_score {
            best_orientation = orientation;
            best_residual = residual;
            best_score = score;
        }
    }
    (best_orientation, best_residual)
}

fn best_block_orientation(words: &[u16], start: usize, end: usize) -> u8 {
    let count = end - start;
    let sample_count = count.min(ORIENTATION_SAMPLES_PER_BLOCK);
    let mut best_orientation = 0_u8;
    let mut best_score = u64::MAX;
    for orientation in 0..GAUGE_COUNT as u8 {
        let mut score = 0_u64;
        for sample in 0..sample_count {
            let quaternion_index = start + sample * count / sample_count;
            let current = quaternion_at(words, quaternion_index);
            let previous = if quaternion_index == 0 {
                [0_u16; 4]
            } else {
                quaternion_at(words, quaternion_index - 1)
            };
            score += u64::from(residual_score(xor_quaternion(
                apply_gauge(orientation, current),
                previous,
            )));
        }
        if score < best_score {
            best_orientation = orientation;
            best_score = score;
        }
    }
    best_orientation
}

fn quaternion_at(words: &[u16], quaternion_index: usize) -> [u16; 4] {
    let start = quaternion_index * 4;
    let end = (start + 4).min(words.len());
    let mut quaternion = [0_u16; 4];
    quaternion[..end - start].copy_from_slice(&words[start..end]);
    quaternion
}

fn residual_score(residual: [u16; 4]) -> u32 {
    residual
        .iter()
        .map(|word| {
            let [high, low] = word.to_be_bytes();
            u32::from(high != 0) * 8 + u32::from(low != 0) * 4 + word.count_ones()
        })
        .sum()
}

fn xor_quaternion(left: [u16; 4], right: [u16; 4]) -> [u16; 4] {
    [
        left[0] ^ right[0],
        left[1] ^ right[1],
        left[2] ^ right[2],
        left[3] ^ right[3],
    ]
}

fn apply_gauge(orientation: u8, q: [u16; 4]) -> [u16; 4] {
    let [a, b, c, d] = q;
    match orientation {
        0 => [a, b, c, d],
        1 => [neg(a), neg(b), neg(c), neg(d)],
        2 => [neg(b), a, neg(d), c],
        3 => [b, neg(a), d, neg(c)],
        4 => [neg(c), d, a, neg(b)],
        5 => [c, neg(d), neg(a), b],
        6 => [neg(d), neg(c), b, a],
        7 => [d, c, neg(b), neg(a)],
        _ => unreachable!("orientation is three bits"),
    }
}

fn inverse_orientation(orientation: u8) -> u8 {
    match orientation {
        0 | 1 => orientation,
        2 => 3,
        3 => 2,
        4 => 5,
        5 => 4,
        6 => 7,
        7 => 6,
        _ => unreachable!("orientation is three bits"),
    }
}

fn neg(word: u16) -> u16 {
    word ^ 0x8000
}

fn gauge_sample_gate(bytes: &[u8]) -> Result<bool, Box<dyn std::error::Error>> {
    let words = le_bytes_to_words(bytes)?;
    if words.len() < 8 {
        return Ok(false);
    }
    let quaternion_count = words.len().div_ceil(4);
    let block_step = quaternion_count.div_ceil(SAMPLE_QUATERNIONS).max(1);
    let mut identity_score = 0_u64;
    let mut gauge_score = 0_u64;
    let mut previous = [0_u16; 4];
    let mut sampled = 0;
    for (block_index, block) in words.chunks(4).enumerate() {
        let mut current = [0_u16; 4];
        current[..block.len()].copy_from_slice(block);
        if block_index > 0 && block_index.is_multiple_of(block_step) {
            let identity = xor_quaternion(current, previous);
            let (_, gauge) = best_orientation(current, previous);
            identity_score += u64::from(residual_score(identity));
            gauge_score += u64::from(residual_score(gauge));
            sampled += 1;
            if sampled == SAMPLE_QUATERNIONS {
                break;
            }
        }
        previous = current;
    }
    Ok(sampled > 0
        && gauge_score.saturating_mul(100) <= identity_score.saturating_mul(GATE_PERCENT as u64))
}

fn pack_orientations(orientations: &[u8]) -> Vec<u8> {
    let mut packed = vec![0_u8; orientations.len().saturating_mul(3).div_ceil(8)];
    let mut bit_offset = 0;
    for orientation in orientations {
        let value = u16::from(*orientation & 0b111);
        let byte = bit_offset / 8;
        let shift = bit_offset % 8;
        packed[byte] |= (value << shift) as u8;
        if shift > 5 {
            packed[byte + 1] |= (value >> (8 - shift)) as u8;
        }
        bit_offset += 3;
    }
    packed
}

fn unpack_orientations(packed: &[u8], count: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if packed.len() != count.saturating_mul(3).div_ceil(8) {
        return Err("orientation stream length is invalid".into());
    }
    let mut orientations = Vec::with_capacity(count);
    let mut bit_offset = 0;
    for _ in 0..count {
        let byte = bit_offset / 8;
        let shift = bit_offset % 8;
        let mut value = u16::from(packed[byte]) >> shift;
        if shift > 5 {
            value |= u16::from(packed[byte + 1]) << (8 - shift);
        }
        orientations.push((value & 0b111) as u8);
        bit_offset += 3;
    }
    Ok(orientations)
}

fn byte_plane_words(words: &[u16]) -> Vec<u8> {
    let mut planes = vec![0_u8; words.len() * 2];
    for (index, word) in words.iter().enumerate() {
        let [high, low] = word.to_be_bytes();
        planes[index] = high;
        planes[words.len() + index] = low;
    }
    planes
}

fn words_from_byte_planes(
    planes: &[u8],
    count: usize,
) -> Result<Vec<u16>, Box<dyn std::error::Error>> {
    if planes.len() != count.saturating_mul(2) {
        return Err("residual byte planes have an invalid length".into());
    }
    Ok((0..count)
        .map(|index| u16::from_be_bytes([planes[index], planes[count + index]]))
        .collect())
}

fn le_bytes_to_words(bytes: &[u8]) -> Result<Vec<u16>, Box<dyn std::error::Error>> {
    if !bytes.len().is_multiple_of(2) {
        return Err("native 16-bit input has an odd byte length".into());
    }
    Ok(bytes
        .chunks_exact(2)
        .map(|word| u16::from_le_bytes([word[0], word[1]]))
        .collect())
}

fn words_to_le_bytes(words: &[u16]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

fn gauge_size(payload: &GaugePayload) -> usize {
    GAUGE_METADATA_BYTES + payload.orientation_bytes.len() + payload.residual_bytes.len()
}

fn synthetic_datasets() -> Vec<Dataset> {
    vec![
        Dataset {
            name: "gauge-orbit-runs".to_string(),
            bytes: make_gauge_orbit_fixture(),
        },
        Dataset {
            name: "smooth-native-control".to_string(),
            bytes: make_smooth_fixture(),
        },
        Dataset {
            name: "random-control".to_string(),
            bytes: make_random_fixture(),
        },
        Dataset {
            name: "all-u16-patterns".to_string(),
            bytes: words_to_le_bytes(&(0..=u16::MAX).collect::<Vec<_>>()),
        },
    ]
}

fn make_gauge_orbit_fixture() -> Vec<u8> {
    let mut words = Vec::with_capacity(65_536);
    let base = [0x3d10_u16, 0x3e20, 0xbd30, 0x3f40];
    for index in 0..16_384 {
        let drift = (index / 256) as u16;
        let canonical = [
            base[0].wrapping_add(drift),
            base[1].wrapping_add(drift),
            base[2].wrapping_add(drift),
            base[3].wrapping_add(drift),
        ];
        let orientation = ((index / 64) % GAUGE_COUNT) as u8;
        words.extend_from_slice(&apply_gauge(inverse_orientation(orientation), canonical));
    }
    words_to_le_bytes(&words)
}

fn make_smooth_fixture() -> Vec<u8> {
    let words: Vec<u16> = (0..65_536_u32)
        .map(|index| {
            0x3d00_u16
                .wrapping_add((index % 128) as u16)
                .wrapping_add((index / 4096) as u16)
        })
        .collect();
    words_to_le_bytes(&words)
}

fn make_random_fixture() -> Vec<u8> {
    let mut state = 0x5141_5451_c0de_f00d_u64;
    let words: Vec<u16> = (0..65_536)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state as u16
        })
        .collect();
    words_to_le_bytes(&words)
}

fn load_directory(label: &str, path: &Path) -> Result<Dataset, Box<dyn std::error::Error>> {
    let mut paths: Vec<PathBuf> = fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("f16le" | "bf16le")
            )
        })
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(format!("{} has no native 16-bit tensor files", path.display()).into());
    }
    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(&fs::read(path)?);
    }
    Ok(Dataset {
        name: label.to_string(),
        bytes,
    })
}

fn parse_label_path(value: &str) -> Result<(String, PathBuf), Box<dyn std::error::Error>> {
    let (label, path) = value
        .split_once(':')
        .ok_or("input must use label:path syntax")?;
    if label.is_empty() || path.is_empty() {
        return Err("input must use non-empty label:path syntax".into());
    }
    Ok((label.to_string(), PathBuf::from(path)))
}

fn median_timing<T>(mut operation: impl FnMut() -> T) -> f64 {
    let mut samples = Vec::with_capacity(ITERATIONS);
    for _ in 0..5 {
        std::hint::black_box(operation());
    }
    for _ in 0..ITERATIONS {
        let started = Instant::now();
        std::hint::black_box(operation());
        samples.push(started.elapsed().as_nanos() as f64);
    }
    samples.sort_by(f64::total_cmp);
    samples[samples.len() / 2]
}

fn print_rows(rows: &[ResultRow]) {
    println!(
        "| dataset | raw bytes | qatq-exact bytes | gauge candidate bytes | size change | selected | qatq enc ns/value | gauge enc change | qatq dec ns/value | gauge dec change |"
    );
    println!("| --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: |");
    for row in rows {
        println!(
            "| {} | {} | {} | {} | {:+.2}% | {} | {:.3} | {:+.2}% | {:.3} | {:+.2}% |",
            row.name,
            row.raw_bytes,
            row.qatq_bytes,
            row.gauge_candidate_bytes,
            percent_change(row.gauge_candidate_bytes as f64, row.qatq_bytes as f64),
            if row.selected { "yes" } else { "no" },
            row.qatq_encode_ns_per_value,
            percent_change(row.gauge_encode_ns_per_value, row.qatq_encode_ns_per_value),
            row.qatq_decode_ns_per_value,
            percent_change(row.gauge_decode_ns_per_value, row.qatq_decode_ns_per_value),
        );
    }
}

fn percent_change(candidate: f64, baseline: f64) -> f64 {
    (candidate / baseline - 1.0) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gauge_action_is_exactly_invertible() {
        let patterns = [
            [0x0000, 0x8000, 0x7f80, 0xff80],
            [0x7fc1, 0xffc1, 0x0001, 0x8001],
            [0x3f80, 0xbf80, 0x3555, 0xb555],
        ];
        for quaternion in patterns {
            for orientation in 0..GAUGE_COUNT as u8 {
                assert_eq!(
                    apply_gauge(
                        inverse_orientation(orientation),
                        apply_gauge(orientation, quaternion)
                    ),
                    quaternion
                );
            }
        }
    }

    #[test]
    fn orientation_packing_roundtrips_every_symbol() {
        let orientations: Vec<u8> = (0..257).map(|index| (index % GAUGE_COUNT) as u8).collect();
        let packed = pack_orientations(&orientations);
        assert_eq!(
            unpack_orientations(&packed, orientations.len()).unwrap(),
            orientations
        );
    }

    #[test]
    fn gauge_payload_preserves_partial_quaternions() {
        for word_count in 1..40 {
            let words: Vec<u16> = (0..word_count)
                .map(|index| (index as u16).wrapping_mul(7919))
                .collect();
            let bytes = words_to_le_bytes(&words);
            let encoded = encode_gauge(&bytes).unwrap();
            assert_eq!(decode_gauge(&encoded).unwrap(), bytes);
        }
    }
}
