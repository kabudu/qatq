#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut values = Vec::new();
    for chunk in data.chunks_exact(4).take(4096) {
        values.push(f32::from_bits(u32::from_le_bytes(
            chunk.try_into().expect("chunk size checked"),
        )));
    }
    if let Ok(encoded) = qatq::try_encode_phase2_lossless_with_config(
        &values,
        qatq::Phase1Config::default(),
    ) {
        if let Ok(decoded) = qatq::decode_phase2_lossless(&encoded) {
            assert_eq!(f32_bits(&decoded), f32_bits(&values));
        }
    }
});

fn f32_bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|value| value.to_bits()).collect()
}
