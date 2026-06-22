use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use qatq::{
    decode, decode_phase2_lossless, decode_phase2_lossless_container,
    decode_phase2_lossless_container_with_limits, encode_phase2_lossless_container_with_config,
    phase2_lossless_strategy, restore_production_chunk,
    try_encode_phase2_lossless_exhaustive_with_config, try_encode_phase2_lossless_with_config,
    try_encode_production_chunk_with_config, Phase1Config, Phase2Strategy, QatcDecodeLimits,
};

const DEFAULT_CASES: usize = 4_096;
const EXHAUSTIVE_CASES: usize = 512;
const LARGE_VALUE_LIMIT: usize = 8_192;
const DEBUG_DECODE_NS_PER_VALUE_CEILING: f64 = 2_500.0;
const RELEASE_DECODE_NS_PER_VALUE_CEILING: f64 = 150.0;

#[test]
#[ignore = "runs thousands of deterministic KV-cache codec stress cases"]
fn phase2_kv_cache_stress_matrix() {
    let case_count = std::env::var("QATQ_KV_STRESS_CASES")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_CASES);
    assert!(
        case_count >= 1_000,
        "stress proof must cover at least 1000 cases"
    );

    let mut summary = StressSummary::default();
    let mut strategy_counts = BTreeMap::<&'static str, usize>::new();
    let mut storage_counts = BTreeMap::<&'static str, usize>::new();

    for case_index in 0..case_count {
        let spec = KvSpec::for_index(case_index);
        let values = spec.generate();
        let config = Phase1Config {
            seed: 0x5141_5451_0000_0000 ^ mix64(case_index as u64),
        };
        let raw_len = values.len() * 4;

        let encode_start = Instant::now();
        let encoded = try_encode_phase2_lossless_with_config(&values, config).unwrap();
        summary.encode_time += encode_start.elapsed();
        summary.encoded_bytes += encoded.len();
        summary.raw_bytes += raw_len;

        let strategy = phase2_lossless_strategy(&encoded).unwrap();
        *strategy_counts.entry(strategy.as_str()).or_default() += 1;

        let decode_start = Instant::now();
        let decoded = decode_phase2_lossless(&encoded).unwrap();
        summary.decode_time += decode_start.elapsed();
        assert_same_bits(&values, &decoded, case_index, "single-payload");

        let decoded_via_dispatch = decode(&encoded).unwrap();
        assert_same_bits(&values, &decoded_via_dispatch, case_index, "dispatch");

        let decision = try_encode_production_chunk_with_config(&values, config).unwrap();
        *storage_counts
            .entry(decision.metadata.storage_label())
            .or_default() += 1;
        let restored =
            restore_production_chunk(&decision.metadata, decision.stored_bytes()).unwrap();
        assert_same_bits(&values, &restored, case_index, "production-chunk");

        let chunk_size = spec.chunk_size();
        let container =
            encode_phase2_lossless_container_with_config(&values, chunk_size, config).unwrap();
        summary.container_bytes += container.len();
        let decoded_container = decode_phase2_lossless_container(&container).unwrap();
        assert_same_bits(&values, &decoded_container, case_index, "container");
        let decoded_container_via_dispatch = decode(&container).unwrap();
        assert_same_bits(
            &values,
            &decoded_container_via_dispatch,
            case_index,
            "container-dispatch",
        );

        if case_index < EXHAUSTIVE_CASES && values.len() <= 2_048 {
            let exhaustive =
                try_encode_phase2_lossless_exhaustive_with_config(&values, config).unwrap();
            let decoded_exhaustive = decode_phase2_lossless(&exhaustive).unwrap();
            assert_same_bits(&values, &decoded_exhaustive, case_index, "exhaustive");
            assert!(
                encoded.len() <= exhaustive.len(),
                "case {case_index} default encoder produced {} bytes but exhaustive produced {} bytes",
                encoded.len(),
                exhaustive.len()
            );
        }

        if case_index % 7 == 0 && encoded.len() > 40 {
            assert_mutation_rejected(&encoded, case_index, "phase2-payload");
        }
        if case_index % 11 == 0 && container.len() > 48 {
            assert_mutation_rejected(&container, case_index, "qatc-container");
        }
        if case_index % 13 == 0 && !values.is_empty() {
            let limits = QatcDecodeLimits {
                max_total_values: values.len() - 1,
                ..QatcDecodeLimits::default()
            };
            assert!(
                decode_phase2_lossless_container_with_limits(&container, limits).is_err(),
                "case {case_index} accepted a container above total-value decode limit"
            );
        }

        summary.values += values.len();
        summary.cases += 1;
        summary.max_values = summary.max_values.max(values.len());
        summary.max_encoded_bytes = summary.max_encoded_bytes.max(encoded.len());
        summary.best_ratio = summary
            .best_ratio
            .min(encoded.len() as f64 / raw_len.max(1) as f64);
        summary.worst_ratio = summary
            .worst_ratio
            .max(encoded.len() as f64 / raw_len.max(1) as f64);
    }

    assert!(
        strategy_counts.len() >= 4,
        "stress corpus did not exercise enough Phase 2 strategies: {strategy_counts:?}"
    );
    assert!(
        strategy_counts.contains_key(Phase2Strategy::QuaternionChainZstd.as_str()),
        "stress corpus never selected reversible quaternion-chain zstd"
    );
    assert!(
        strategy_counts.contains_key(Phase2Strategy::BytePlaneZstd.as_str()),
        "stress corpus never selected byte-plane zstd"
    );
    assert!(
        strategy_counts.contains_key(Phase2Strategy::RawBits.as_str()),
        "stress corpus never selected raw-bits fallback"
    );
    assert!(
        storage_counts.contains_key("qatq-phase2")
            && storage_counts.contains_key("raw-f32le-pass-through"),
        "stress corpus did not cover both production storage decisions: {storage_counts:?}"
    );

    let decode_ns_per_value = summary.decode_time.as_nanos() as f64 / summary.values.max(1) as f64;
    let ceiling = if cfg!(debug_assertions) {
        DEBUG_DECODE_NS_PER_VALUE_CEILING
    } else {
        RELEASE_DECODE_NS_PER_VALUE_CEILING
    };
    assert!(
        decode_ns_per_value <= ceiling,
        "decode throughput regressed: {decode_ns_per_value:.2}ns/value > {ceiling:.2}ns/value"
    );

    eprintln!(
        "qatq kv stress: cases={} values={} max_values={} raw_bytes={} encoded_bytes={} container_bytes={} avg_ratio={:.4} best_ratio={:.4} worst_ratio={:.4} encode_ns/value={:.2} decode_ns/value={:.2} strategies={:?} storage={:?}",
        summary.cases,
        summary.values,
        summary.max_values,
        summary.raw_bytes,
        summary.encoded_bytes,
        summary.container_bytes,
        summary.encoded_bytes as f64 / summary.raw_bytes.max(1) as f64,
        summary.best_ratio,
        summary.worst_ratio,
        summary.encode_time.as_nanos() as f64 / summary.values.max(1) as f64,
        decode_ns_per_value,
        strategy_counts,
        storage_counts
    );
}

#[derive(Clone, Copy, Debug)]
struct KvSpec {
    tokens: usize,
    heads: usize,
    dim: usize,
    pattern: Pattern,
    seed: u64,
}

impl KvSpec {
    fn for_index(index: usize) -> Self {
        const TOKENS: [usize; 9] = [1, 2, 3, 4, 8, 16, 32, 64, 128];
        const HEADS: [usize; 7] = [1, 2, 3, 4, 8, 12, 16];
        const DIMS: [usize; 8] = [4, 8, 12, 16, 24, 32, 48, 64];
        let mixed = mix64(index as u64 ^ 0xa17a_5eed_5141_5451);
        let mut tokens = TOKENS[index % TOKENS.len()];
        let mut heads = HEADS[(index / TOKENS.len()) % HEADS.len()];
        let mut dim = DIMS[(index / (TOKENS.len() * HEADS.len())) % DIMS.len()];
        while tokens * heads * dim > LARGE_VALUE_LIMIT {
            if tokens > 1 {
                tokens /= 2;
            } else if heads > 1 {
                heads /= 2;
            } else {
                dim /= 2;
            }
        }
        Self {
            tokens,
            heads,
            dim,
            pattern: Pattern::for_index(index),
            seed: mixed,
        }
    }

    fn value_count(self) -> usize {
        self.tokens * self.heads * self.dim
    }

    fn chunk_size(self) -> usize {
        let base = match self.value_count() % 5 {
            0 => self.dim.max(1),
            1 => (self.heads * self.dim).max(1),
            2 => (self.tokens.max(1) * self.dim / 2).max(1),
            3 => 257.min(self.value_count().max(1)),
            _ => 1024.min(self.value_count().max(1)),
        };
        base.clamp(1, self.value_count().max(1))
    }

    fn generate(self) -> Vec<f32> {
        let mut values = Vec::with_capacity(self.value_count());
        let mut state = self.seed;
        for token in 0..self.tokens {
            for head in 0..self.heads {
                for channel in 0..self.dim {
                    state = mix64(state.wrapping_add(0x9e37_79b9_7f4a_7c15));
                    values.push(self.pattern.value(self, token, head, channel, state));
                }
            }
        }
        values
    }
}

#[derive(Clone, Copy, Debug)]
enum Pattern {
    Bf16Ramp,
    Bf16Wave,
    LowRankHeads,
    PageSparse,
    QuaternionChain,
    SignedZeroNanInf,
    DeltaBits,
    RepeatedTokens,
    RawNoise,
    HeadStripes,
}

impl Pattern {
    fn for_index(index: usize) -> Self {
        match index % 10 {
            0 => Self::Bf16Ramp,
            1 => Self::Bf16Wave,
            2 => Self::LowRankHeads,
            3 => Self::PageSparse,
            4 => Self::QuaternionChain,
            5 => Self::SignedZeroNanInf,
            6 => Self::DeltaBits,
            7 => Self::RepeatedTokens,
            8 => Self::RawNoise,
            _ => Self::HeadStripes,
        }
    }

    fn value(self, spec: KvSpec, token: usize, head: usize, channel: usize, state: u64) -> f32 {
        let linear = ((token * spec.heads + head) * spec.dim + channel) as u32;
        match self {
            Self::Bf16Ramp => {
                let bits = 0x3f80_0000_u32
                    .wrapping_add((token as u32) << 17)
                    .wrapping_add((head as u32) << 13)
                    .wrapping_add((channel as u32) << 9);
                f32::from_bits(bits & 0xffff_0000)
            }
            Self::Bf16Wave => {
                let wave =
                    ((token as f32 * 0.173) + (head as f32 * 0.311) + channel as f32 * 0.071).sin()
                        * 3.0;
                f32::from_bits(wave.to_bits() & 0xffff_0000)
            }
            Self::LowRankHeads => {
                let base = (token as f32 * 0.05).sin() + (channel as f32 * 0.02).cos();
                f32::from_bits((base + head as f32 * 0.000_976_562_5).to_bits() & 0xffff_8000)
            }
            Self::PageSparse => {
                if (token / 8 + head + channel / 4) % 5 == 0 {
                    0.0
                } else {
                    let sign = if (state & 1) == 0 { 1.0 } else { -1.0 };
                    f32::from_bits((sign * ((channel % 7) as f32 + 0.25)).to_bits() & 0xffff_0000)
                }
            }
            Self::QuaternionChain => {
                let lane = linear / 4;
                let component = linear % 4;
                let residual = match component {
                    0 => 0x0000_0010 + (head as u32 & 0x03),
                    1 => 0x0000_0001 + (token as u32 & 0x01),
                    2 => 0xffff_ffff,
                    _ => 0x0000_0000,
                };
                let bits = 0x3f80_0000_u32
                    .wrapping_add(lane.wrapping_mul(0x10))
                    .wrapping_add(residual);
                f32::from_bits(bits)
            }
            Self::SignedZeroNanInf => match linear % 8 {
                0 => 0.0,
                1 => -0.0,
                2 => f32::INFINITY,
                3 => f32::NEG_INFINITY,
                4 => f32::from_bits(0x7fc0_0000 | (linear & 0x000f_ffff)),
                5 => f32::from_bits(0xffc0_0000 | (linear & 0x000f_ffff)),
                6 => f32::MIN_POSITIVE,
                _ => -f32::MIN_POSITIVE,
            },
            Self::DeltaBits => {
                let bits = 0x3f00_0000_u32 ^ linear.rotate_left((head % 11) as u32);
                f32::from_bits(bits)
            }
            Self::RepeatedTokens => {
                let repeated_token = token % 4;
                let bits = 0x4000_0000_u32
                    .wrapping_add((repeated_token as u32) << 16)
                    .wrapping_add((head as u32) << 12)
                    .wrapping_add((channel as u32) << 4);
                f32::from_bits(bits & 0xffff_ff00)
            }
            Self::RawNoise => f32::from_bits((state as u32).wrapping_add(linear.rotate_left(7))),
            Self::HeadStripes => {
                let bits = if head % 2 == 0 {
                    0x3f80_0000_u32.wrapping_add((channel as u32) << 12)
                } else {
                    0xbf80_0000_u32.wrapping_sub((token as u32) << 12)
                };
                f32::from_bits(bits)
            }
        }
    }
}

#[derive(Debug)]
struct StressSummary {
    cases: usize,
    values: usize,
    raw_bytes: usize,
    encoded_bytes: usize,
    container_bytes: usize,
    max_values: usize,
    max_encoded_bytes: usize,
    best_ratio: f64,
    worst_ratio: f64,
    encode_time: Duration,
    decode_time: Duration,
}

impl Default for StressSummary {
    fn default() -> Self {
        Self {
            cases: 0,
            values: 0,
            raw_bytes: 0,
            encoded_bytes: 0,
            container_bytes: 0,
            max_values: 0,
            max_encoded_bytes: 0,
            best_ratio: f64::INFINITY,
            worst_ratio: 0.0,
            encode_time: Duration::ZERO,
            decode_time: Duration::ZERO,
        }
    }
}

fn assert_same_bits(expected: &[f32], actual: &[f32], case_index: usize, path: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "case {case_index} {path} length mismatch"
    );
    for (value_index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "case {case_index} {path} bit mismatch at value {value_index}"
        );
    }
}

fn assert_mutation_rejected(payload: &[u8], case_index: usize, path: &str) {
    let mut mutated = payload.to_vec();
    let offset = 28 + ((case_index * 17) % (mutated.len() - 28));
    mutated[offset] ^= 0x5a;
    assert!(
        decode(&mutated).is_err(),
        "case {case_index} accepted mutated {path} byte at offset {offset}"
    );
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
