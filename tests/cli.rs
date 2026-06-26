use std::{
    fs,
    process::{Command, Stdio},
};

use qatq::{MAX_VALUES_PER_PAYLOAD, live_vram_page_checksum};

const LIVE_VRAM_TEST_SEAL_KEY_HEX: &str =
    "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

#[test]
fn cli_encodes_and_decodes_turboquant_q4_with_seed() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-turboquant-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let encoded = dir.join(format!("{stem}.qatq"));
    let decoded = dir.join(format!("{stem}.decoded.f32le"));
    let values = [0.25_f32, -0.5, 1.0, 2.0, -3.5, 0.125];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write input");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode")
        .arg("--mode")
        .arg("turboquant-q4")
        .arg("--seed")
        .arg("0x51415451")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode");
    assert!(encode_status.success());

    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(decode_status.success());

    let decoded_bytes = fs::read(&decoded).expect("read decoded");
    assert_eq!(decoded_bytes.len(), values.len() * 4);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_encodes_and_decodes_phase1_q4_with_seed() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let encoded = dir.join(format!("{stem}.qatq"));
    let decoded = dir.join(format!("{stem}.decoded.f32le"));
    let values = [0.25_f32, -0.5, 1.0, 2.0, -3.5, 0.125];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write input");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode")
        .arg("--mode")
        .arg("phase1-q4")
        .arg("--seed")
        .arg("0x51415451")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode");
    assert!(encode_status.success());

    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(decode_status.success());

    let decoded_bytes = fs::read(&decoded).expect("read decoded");
    assert_eq!(decoded_bytes.len(), values.len() * 4);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_encodes_and_decodes_qatq_exact_with_seed_exactly() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-exact-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let encoded = dir.join(format!("{stem}.qatq"));
    let decoded = dir.join(format!("{stem}.decoded.f32le"));
    let values = [
        0.25_f32,
        -0.0,
        f32::INFINITY,
        f32::from_bits(0x7fc0_1234),
        -3.5,
        0.125,
    ];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, &input_bytes).expect("write input");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode")
        .arg("--mode")
        .arg("qatq-exact")
        .arg("--seed")
        .arg("0x51415451")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode");
    assert!(encode_status.success());

    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(decode_status.success());

    let decoded_bytes = fs::read(&decoded).expect("read decoded");
    assert_eq!(decoded_bytes, input_bytes);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_encodes_and_decodes_qatq_exact_bf16_native_bytes() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-exact-bf16-{}", std::process::id());
    let input = dir.join(format!("{stem}.bf16le"));
    let encoded = dir.join(format!("{stem}.qatq"));
    let decoded = dir.join(format!("{stem}.decoded.bf16le"));
    let mut input_bytes = Vec::new();
    for bits in [
        0x0000_u16, 0x8000, 0x3f80, 0xbf80, 0x7f80, 0xff80, 0x7fc1, 0x3eab, 0x3eab, 0x3eab,
    ] {
        input_bytes.extend_from_slice(&bits.to_le_bytes());
    }
    fs::write(&input, &input_bytes).expect("write input");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode")
        .arg("--mode")
        .arg("qatq-exact")
        .arg("--dtype")
        .arg("bf16")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode");
    assert!(encode_status.success());

    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(decode_status.success());

    assert_eq!(fs::read(&decoded).expect("read decoded"), input_bytes);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_encodes_chunked_qatq_exact_f16_native_bytes() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-chunked-f16-{}", std::process::id());
    let input = dir.join(format!("{stem}.f16le"));
    let encoded = dir.join(format!("{stem}.qatc"));
    let decoded = dir.join(format!("{stem}.decoded.f16le"));
    let mut input_bytes = Vec::new();
    for index in 0..130_u16 {
        let bits = match index % 9 {
            0 => 0x0000,
            1 => 0x8000,
            2 => 0x3c00,
            3 => 0xbc00,
            4 => 0x7c00,
            5 => 0xfc00,
            6 => 0x7e01,
            _ => 0x3000 + index,
        };
        input_bytes.extend_from_slice(&bits.to_le_bytes());
    }
    fs::write(&input, &input_bytes).expect("write input");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode-chunked")
        .arg("--max-values-per-chunk")
        .arg("31")
        .arg("--dtype")
        .arg("f16")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode-chunked");
    assert!(encode_status.success());

    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(decode_status.success());

    assert_eq!(fs::read(&decoded).expect("read decoded"), input_bytes);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_corrupt_qatq_decode_does_not_overwrite_existing_output() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-corrupt-exact-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let encoded = dir.join(format!("{stem}.qatq"));
    let decoded = dir.join(format!("{stem}.decoded.f32le"));
    let values = [0.25_f32, -0.0, f32::from_bits(0x7fc0_1234), -3.5];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, &input_bytes).expect("write input");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode")
        .arg("--mode")
        .arg("qatq-exact")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode");
    assert!(encode_status.success());

    let mut encoded_bytes = fs::read(&encoded).expect("read encoded");
    let last = encoded_bytes.last_mut().expect("encoded bytes");
    *last ^= 0x01;
    fs::write(&encoded, encoded_bytes).expect("write corrupt encoded");

    let sentinel = b"keep-existing-single-output";
    fs::write(&decoded, sentinel).expect("write sentinel output");

    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(!decode_status.success());

    let decoded_bytes = fs::read(&decoded).expect("read decoded");
    assert_eq!(decoded_bytes, sentinel);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_failed_encode_does_not_overwrite_existing_output() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-failed-encode-{}", std::process::id());
    let input = dir.join(format!("{stem}.bad"));
    let encoded = dir.join(format!("{stem}.qatq"));
    fs::write(&input, [1_u8, 2, 3]).expect("write invalid input");

    let sentinel = b"keep-existing-encoded-output";
    fs::write(&encoded, sentinel).expect("write sentinel output");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode")
        .arg("--mode")
        .arg("qatq-exact")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode");
    assert!(!encode_status.success());

    let encoded_bytes = fs::read(&encoded).expect("read encoded");
    assert_eq!(encoded_bytes, sentinel);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
}

#[test]
fn cli_oversized_single_payload_encode_fails_before_overwriting_output() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-oversized-encode-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let encoded = dir.join(format!("{stem}.qatq"));
    let file = fs::File::create(&input).expect("create sparse input");
    file.set_len(((MAX_VALUES_PER_PAYLOAD as u64) + 1) * 4)
        .expect("size sparse input");

    let sentinel = b"keep-existing-oversized-output";
    fs::write(&encoded, sentinel).expect("write sentinel output");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode")
        .arg("--mode")
        .arg("qatq-exact")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode");
    assert!(!encode_status.success());

    let encoded_bytes = fs::read(&encoded).expect("read encoded");
    assert_eq!(encoded_bytes, sentinel);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
}

#[test]
fn cli_encodes_chunked_exact_container_and_decodes_exactly() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-chunked-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let encoded = dir.join(format!("{stem}.qatc"));
    let decoded = dir.join(format!("{stem}.decoded.f32le"));
    let values: Vec<f32> = (0..130)
        .map(|index| {
            if index % 17 == 0 {
                f32::from_bits(0x7fc0_0000 | index as u32)
            } else {
                ((index as f32) * 0.041).sin()
            }
        })
        .collect();
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, &input_bytes).expect("write input");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode-chunked")
        .arg("--max-values-per-chunk")
        .arg("32")
        .arg("--seed")
        .arg("0x51415451")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode-chunked");
    assert!(encode_status.success());

    let encoded_bytes = fs::read(&encoded).expect("read encoded");
    assert_eq!(&encoded_bytes[0..4], b"QATC");
    assert_eq!(encoded_bytes[4], 2);
    assert_ne!(&encoded_bytes[24..32], &[0_u8; 8]);

    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(decode_status.success());

    let decoded_bytes = fs::read(&decoded).expect("read decoded");
    assert_eq!(decoded_bytes, input_bytes);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_failed_encode_chunked_does_not_overwrite_existing_output() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-failed-chunked-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let encoded = dir.join(format!("{stem}.qatc"));
    let values = [0.25_f32, -0.5, 1.0, 2.0];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write input");

    let sentinel = b"keep-existing-chunked-output";
    fs::write(&encoded, sentinel).expect("write sentinel output");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode-chunked")
        .arg("--max-values-per-chunk")
        .arg("0")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode-chunked");
    assert!(!encode_status.success());

    let encoded_bytes = fs::read(&encoded).expect("read encoded");
    assert_eq!(encoded_bytes, sentinel);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
}

#[test]
fn cli_corrupt_qatc_decode_does_not_overwrite_existing_output() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-corrupt-chunked-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let encoded = dir.join(format!("{stem}.qatc"));
    let decoded = dir.join(format!("{stem}.decoded.f32le"));
    let values: Vec<f32> = (0..130).map(|index| (index as f32).sin()).collect();
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, &input_bytes).expect("write input");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode-chunked")
        .arg("--max-values-per-chunk")
        .arg("32")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode-chunked");
    assert!(encode_status.success());

    let mut encoded_bytes = fs::read(&encoded).expect("read encoded");
    let last = encoded_bytes.last_mut().expect("encoded bytes");
    *last ^= 0x01;
    fs::write(&encoded, encoded_bytes).expect("write corrupt encoded");

    let sentinel = b"keep-existing-output";
    fs::write(&decoded, sentinel).expect("write sentinel output");

    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(!decode_status.success());

    let decoded_bytes = fs::read(&decoded).expect("read decoded");
    assert_eq!(decoded_bytes, sentinel);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_qatc_checksum_mismatch_does_not_overwrite_existing_output() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-corrupt-qatchash-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let encoded = dir.join(format!("{stem}.qatc"));
    let decoded = dir.join(format!("{stem}.decoded.f32le"));
    let values: Vec<f32> = (0..130).map(|index| (index as f32).cos()).collect();
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, &input_bytes).expect("write input");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let encode_status = Command::new(bin)
        .arg("encode-chunked")
        .arg("--max-values-per-chunk")
        .arg("32")
        .arg(&input)
        .arg(&encoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run encode-chunked");
    assert!(encode_status.success());

    let mut encoded_bytes = fs::read(&encoded).expect("read encoded");
    encoded_bytes[31] ^= 0x01;
    fs::write(&encoded, encoded_bytes).expect("write corrupt encoded");

    let sentinel = b"keep-existing-output";
    fs::write(&decoded, sentinel).expect("write sentinel output");

    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(!decode_status.success());

    let decoded_bytes = fs::read(&decoded).expect("read decoded");
    assert_eq!(decoded_bytes, sentinel);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_qatc_rejects_huge_chunk_count_without_overwriting_output() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-hostile-qatchunks-{}", std::process::id());
    let encoded = dir.join(format!("{stem}.qatc"));
    let decoded = dir.join(format!("{stem}.decoded.f32le"));

    let mut payload = Vec::new();
    payload.extend_from_slice(b"QATC");
    payload.push(2);
    payload.push(4);
    payload.extend_from_slice(&[0, 0]);
    payload.extend_from_slice(&1_u64.to_be_bytes());
    payload.extend_from_slice(&u32::MAX.to_be_bytes());
    payload.extend_from_slice(&[0; 4]);
    payload.extend_from_slice(&0_u64.to_be_bytes());
    fs::write(&encoded, payload).expect("write hostile container");

    let sentinel = b"keep-existing-output";
    fs::write(&decoded, sentinel).expect("write sentinel output");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(!decode_status.success());

    let decoded_bytes = fs::read(&decoded).expect("read decoded");
    assert_eq!(decoded_bytes, sentinel);

    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_qatc_rejects_huge_chunk_len_without_overwriting_output() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-hostile-qatclen-{}", std::process::id());
    let encoded = dir.join(format!("{stem}.qatc"));
    let decoded = dir.join(format!("{stem}.decoded.f32le"));

    let mut payload = Vec::new();
    payload.extend_from_slice(b"QATC");
    payload.push(2);
    payload.push(4);
    payload.extend_from_slice(&[0, 0]);
    payload.extend_from_slice(&1_u64.to_be_bytes());
    payload.extend_from_slice(&1_u32.to_be_bytes());
    payload.extend_from_slice(&[0; 4]);
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&u32::MAX.to_be_bytes());
    fs::write(&encoded, payload).expect("write hostile container");

    let sentinel = b"keep-existing-output";
    fs::write(&decoded, sentinel).expect("write sentinel output");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let decode_status = Command::new(bin)
        .arg("decode")
        .arg(&encoded)
        .arg(&decoded)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run decode");
    assert!(!decode_status.success());

    let decoded_bytes = fs::read(&decoded).expect("read decoded");
    assert_eq!(decoded_bytes, sentinel);

    let _ = fs::remove_file(encoded);
    let _ = fs::remove_file(decoded);
}

#[test]
fn cli_fixture_add_validates_and_appends_manifest_entry() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-fixture-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let manifest = dir.join(format!("{stem}.manifest"));
    let values = [0.25_f32, -0.5, 1.0, 2.0];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, &input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let status = Command::new(bin)
        .arg("fixture")
        .arg("add")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--name")
        .arg("layer0-k")
        .arg("--path")
        .arg(&input)
        .arg("--group")
        .arg("runtime-kv")
        .arg("--shape")
        .arg("[heads=1, tokens=2, dim=2]")
        .arg("--notes")
        .arg("unit capture")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run fixture add");
    assert!(status.success());

    let manifest_text = fs::read_to_string(&manifest).expect("read manifest");
    assert!(manifest_text.contains("[fixture]"));
    assert!(manifest_text.contains("group = \"runtime-kv\""));
    assert!(manifest_text.contains("name = \"layer0-k\""));
    assert!(manifest_text.contains("shape = \"[heads=1, tokens=2, dim=2]\""));
    assert!(manifest_text.contains("notes = \"unit capture; values=4\""));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(manifest);
}

#[test]
fn cli_fixture_generate_writes_public_manifest_and_fixtures() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-generate-fixtures-{}", std::process::id());
    let fixture_dir = dir.join(format!("{stem}-fixtures"));
    let manifest = dir.join(format!("{stem}.manifest"));
    let audit = dir.join(format!("{stem}.audit.md"));

    let bin = env!("CARGO_BIN_EXE_qatq");
    let generate_status = Command::new(bin)
        .arg("fixture")
        .arg("generate")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--dir")
        .arg(&fixture_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run fixture generate");
    assert!(generate_status.success());

    let manifest_text = fs::read_to_string(&manifest).expect("read manifest");
    assert!(manifest_text.contains("group = \"qatq-public\""));
    assert!(manifest_text.contains("bf16-kv-ramp-64x8x16"));
    assert!(manifest_text.contains("f32-noisy-pass-through-64x12x16"));
    assert!(manifest_text.contains("stress-signed-zero-nan-inf"));
    assert!(fixture_dir.join("bf16-kv-ramp-64x8x16.f32le").is_file());

    let verify_status = Command::new(bin)
        .arg("fixture")
        .arg("verify")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--output")
        .arg(&audit)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run fixture verify");
    assert!(verify_status.success());

    let audit_text = fs::read_to_string(&audit).expect("read audit");
    assert!(audit_text.contains("fixtures: `4`"));
    assert!(audit_text.contains("qatq-public"));

    let _ = fs::remove_file(manifest);
    let _ = fs::remove_file(audit);
    let _ = fs::remove_dir_all(fixture_dir);
}

#[test]
fn cli_fixture_add_rejects_non_f32le_input() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-bad-fixture-{}", std::process::id());
    let input = dir.join(format!("{stem}.bin"));
    let manifest = dir.join(format!("{stem}.manifest"));
    fs::write(&input, [1_u8, 2, 3]).expect("write invalid fixture");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let status = Command::new(bin)
        .arg("fixture")
        .arg("add")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--name")
        .arg("bad")
        .arg("--path")
        .arg(&input)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run fixture add");
    assert!(!status.success());
    assert!(!manifest.exists());

    let _ = fs::remove_file(input);
}

#[test]
fn cli_failed_fixture_add_does_not_overwrite_existing_manifest() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-failed-fixture-add-{}", std::process::id());
    let input = dir.join(format!("{stem}.bin"));
    let manifest = dir.join(format!("{stem}.manifest"));
    fs::write(&input, [1_u8, 2, 3]).expect("write invalid fixture");
    let sentinel = b"keep-existing-manifest";
    fs::write(&manifest, sentinel).expect("write sentinel manifest");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let status = Command::new(bin)
        .arg("fixture")
        .arg("add")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--name")
        .arg("bad")
        .arg("--path")
        .arg(&input)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run fixture add");
    assert!(!status.success());
    assert_eq!(fs::read(&manifest).expect("read manifest"), sentinel);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(manifest);
}

#[test]
fn cli_fixture_verify_writes_audit_report() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-verify-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let manifest = dir.join(format!("{stem}.manifest"));
    let audit = dir.join(format!("{stem}.audit.md"));
    let values = [1.0_f32, -0.0, f32::from_bits(0x7fc0_1234)];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, &input_bytes).expect("write fixture");
    fs::write(
        &manifest,
        format!(
            "[fixture]\ngroup = \"runtime-kv\"\nname = \"layer0-v\"\npath = \"{}\"\nshape = \"[3]\"\nnotes = \"unit verify\"\n",
            input.display()
        ),
    )
    .expect("write manifest");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let status = Command::new(bin)
        .arg("fixture")
        .arg("verify")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--output")
        .arg(&audit)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run fixture verify");
    assert!(status.success());

    let audit_text = fs::read_to_string(&audit).expect("read audit");
    assert!(audit_text.contains("# Fixture Audit"));
    assert!(audit_text.contains("fixtures: `1`"));
    assert!(audit_text.contains("total values: `3`"));
    assert!(audit_text.contains(&format!("{:016x}", fnv1a64(&input_bytes))));
    assert!(audit_text.contains("runtime-kv"));
    assert!(audit_text.contains("layer0-v"));
    assert!(audit_text.contains("unit verify"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(manifest);
    let _ = fs::remove_file(audit);
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[test]
fn cli_fixture_verify_rejects_missing_fixture_file() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-missing-fixture-{}", std::process::id());
    let manifest = dir.join(format!("{stem}.manifest"));
    let missing = dir.join(format!("{stem}.f32le"));
    fs::write(
        &manifest,
        format!(
            "[fixture]\nname = \"missing\"\npath = \"{}\"\n",
            missing.display()
        ),
    )
    .expect("write manifest");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let status = Command::new(bin)
        .arg("fixture")
        .arg("verify")
        .arg("--manifest")
        .arg(&manifest)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run fixture verify");
    assert!(!status.success());

    let _ = fs::remove_file(manifest);
}

#[test]
fn cli_failed_fixture_verify_does_not_overwrite_existing_audit() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-failed-verify-{}", std::process::id());
    let manifest = dir.join(format!("{stem}.manifest"));
    let audit = dir.join(format!("{stem}.audit.md"));
    let missing = dir.join(format!("{stem}.f32le"));
    fs::write(
        &manifest,
        format!(
            "[fixture]\nname = \"missing\"\npath = \"{}\"\n",
            missing.display()
        ),
    )
    .expect("write manifest");
    let sentinel = b"keep-existing-audit";
    fs::write(&audit, sentinel).expect("write sentinel audit");

    let bin = env!("CARGO_BIN_EXE_qatq");
    let status = Command::new(bin)
        .arg("fixture")
        .arg("verify")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--output")
        .arg(&audit)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run fixture verify");
    assert!(!status.success());
    assert_eq!(fs::read(&audit).expect("read audit"), sentinel);

    let _ = fs::remove_file(manifest);
    let _ = fs::remove_file(audit);
}

#[test]
fn kv_bench_live_vram_export_writes_json_evidence() {
    let root = std::env::temp_dir().join(format!("qatq-kv-bench-live-vram-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create export dir");
    let tensor = compression_positive_f16_tensor_bytes();
    fs::write(root.join("cache_k_l0_s0.f16le"), &tensor).expect("write tensor");
    fs::write(
        root.join("manifest.json"),
        r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 1024,
  "streams": 1,
  "gpu_allocation_granularity": "whole-context",
  "gpu_context_bytes": 16384,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f16le","dtype":"f16le","active_cells":1024,"embedding":8,"row_bytes":16}
  ]
}
"#,
    )
    .expect("write manifest");
    let output = root.join("evidence.json");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let status = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-gpu-context-bytes")
        .arg("16384")
        .arg("--live-vram-allocation-granularity")
        .arg("whole-context")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run qatq-kv-bench live vram mode");
    assert!(status.success());

    let json = fs::read_to_string(&output).expect("read evidence");
    assert!(json.contains("\"claim_scope\": \"experimental-evidence\""));
    assert!(json.contains("not a live VRAM reduction claim"));
    assert!(json.contains("\"adapter_contract_version\": \"qatq-live-vram-adapter-v0\""));
    assert!(json.contains("\"total_pages\": 1"));
    assert!(json.contains("\"verified_restores\": 1"));
    assert!(json.contains("\"runtime_id\": \"llama.cpp\""));
    assert!(json.contains("\"zstd_bytes\""));
    assert!(json.contains("\"lz4_bytes\""));
    assert!(json.contains("\"residency_estimate\": {"));
    assert!(json.contains("\"allocation_granularity\": \"whole-context\""));
    assert!(json.contains("\"gpu_context_bytes_before\": 16384"));
    assert!(json.contains("\"logical_offloaded_raw_bytes\": 16384"));
    assert!(json.contains("\"reclaimable_gpu_bytes\": 0"));
    assert!(json.contains("\"restore_deadline_report\": {"));
    assert!(json.contains("\"evaluated_pages\": 1"));
    assert!(json.contains("\"prefetch_misses\": 1"));
    assert!(json.contains("\"worst_deficit_bytes\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_live_vram_page_end_next_required_keeps_hot_token_page_resident() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-live-vram-page-end-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create export dir");
    let page = compression_positive_f16_tensor_bytes();
    fs::write(root.join("cache_k_l0_s0_t0_1024.f16le"), &page).expect("write first page");
    fs::write(root.join("cache_k_l0_s0_t1024_2048.f16le"), &page).expect("write second page");
    fs::write(
        root.join("manifest.json"),
        r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 2048,
  "streams": 1,
  "gpu_allocation_granularity": "whole-tensor",
  "gpu_context_bytes": 32768,
  "total_context_bytes": 32768,
  "gpu_resident_tensors": 2,
  "total_tensors": 2,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t0_1024.f16le","dtype":"f16le","token_start":0,"token_end":1024,"active_cells":1024,"embedding":8,"row_bytes":16},
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0_t1024_2048.f16le","dtype":"f16le","token_start":1024,"token_end":2048,"active_cells":1024,"embedding":8,"row_bytes":16}
  ]
}
"#,
    )
    .expect("write manifest");

    let uniform_output = root.join("uniform-evidence.json");
    let capped_output = root.join("capped-evidence.json");
    let page_end_output = root.join("page-end-evidence.json");
    let page_end_prefetch_output = root.join("page-end-prefetch-evidence.json");
    let cold_after_hot_output = root.join("cold-after-hot-evidence.json");
    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let common_args = [
        "--live-vram-export-dir",
        root.to_str().expect("temp path is utf8"),
        "--live-vram-runtime-commit",
        "7992aa7c8",
        "--live-vram-adapter-version",
        "qatq-kv-export-7992aa7c8",
        "--live-vram-model-id",
        "test-model.gguf:sha256:abc123",
        "--live-vram-current-token",
        "0",
        "--live-vram-hot-window-tokens",
        "1024",
        "--live-vram-restore-bytes-per-token",
        "1048576",
    ];

    let status = Command::new(bin)
        .args(common_args)
        .arg("--output")
        .arg(&uniform_output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run uniform next-required live vram evidence");
    assert!(status.success());
    let json = fs::read_to_string(&uniform_output).expect("read uniform evidence");
    assert!(json.contains("\"offloaded_pages\": 2"));
    assert!(json.contains("\"resident_pages\": 0"));

    let status = Command::new(bin)
        .args(common_args)
        .arg("--live-vram-max-queued-pages")
        .arg("1")
        .arg("--output")
        .arg(&capped_output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run capped queued-pages live vram evidence");
    assert!(status.success());
    let json = fs::read_to_string(&capped_output).expect("read capped evidence");
    assert!(json.contains("\"offloaded_pages\": 1"));
    assert!(json.contains("\"resident_pages\": 1"));
    assert!(json.contains("\"keep_reason\": \"queue-full\""));

    let status = Command::new(bin)
        .args(common_args)
        .arg("--live-vram-next-required")
        .arg("page-end")
        .arg("--output")
        .arg(&page_end_output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run page-end next-required live vram evidence");
    assert!(status.success());
    let json = fs::read_to_string(&page_end_output).expect("read page-end evidence");
    assert!(json.contains("\"offloaded_pages\": 1"));
    assert!(json.contains("\"resident_pages\": 1"));

    let status = Command::new(bin)
        .args(common_args)
        .arg("--live-vram-next-required")
        .arg("page-end")
        .arg("--live-vram-prefetch-window-tokens")
        .arg("1024")
        .arg("--output")
        .arg(&page_end_prefetch_output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run page-end prefetch-window live vram evidence");
    assert!(status.success());
    let json =
        fs::read_to_string(&page_end_prefetch_output).expect("read page-end prefetch evidence");
    assert!(json.contains("\"offloaded_pages\": 0"));
    assert!(json.contains("\"resident_pages\": 2"));
    assert!(json.contains("\"keep_reason\": \"inside-prefetch-window\""));

    let status = Command::new(bin)
        .args(common_args)
        .arg("--live-vram-current-token")
        .arg("2048")
        .arg("--live-vram-next-required")
        .arg("cold-after-hot")
        .arg("--output")
        .arg(&cold_after_hot_output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run cold-after-hot next-required live vram evidence");
    assert!(status.success());
    let json = fs::read_to_string(&cold_after_hot_output).expect("read cold-after-hot evidence");
    assert!(json.contains("\"offloaded_pages\": 1"));
    assert!(json.contains("\"resident_pages\": 1"));
    assert!(json.contains("\"keep_reason\": \"inside-hot-window\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_live_vram_proof_gate_rejects_whole_context_replay() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-live-vram-proof-gate-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create export dir");
    let mut tensor = Vec::new();
    for _ in 0..32 {
        tensor.extend_from_slice(&0x3c00_u16.to_le_bytes());
    }
    fs::write(root.join("cache_k_l0_s0.f16le"), &tensor).expect("write tensor");
    fs::write(
        root.join("manifest.json"),
        r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "gpu_allocation_granularity": "whole-context",
  "gpu_context_bytes": 64,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#,
    )
    .expect("write manifest");
    let output = root.join("proof-gated-evidence.json");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let output_status = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-gpu-context-bytes")
        .arg("64")
        .arg("--live-vram-allocation-granularity")
        .arg("whole-context")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1048576")
        .arg("--live-vram-proof-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .output()
        .expect("run proof-gated qatq-kv-bench live vram mode");
    assert!(!output_status.status.success());
    let stderr = String::from_utf8_lossy(&output_status.stderr);
    assert!(stderr.contains("live VRAM proof gate failed"));
    assert!(stderr.contains("whole-context"));
    assert!(!output.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_live_vram_runtime_reclaim_gate_accepts_whole_tensor_reclaim() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-live-vram-runtime-reclaim-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create export dir");
    let mut tensor = Vec::new();
    for index in 0..4096_u32 {
        let value = ((index as f32) * 0.03125).sin();
        let value = f32::from_bits(value.to_bits() & 0xffff_0000);
        tensor.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(root.join("cache_k_l0_s0.f32le"), &tensor).expect("write tensor");
    fs::write(
        root.join("manifest.json"),
        r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 4096,
  "streams": 1,
  "gpu_allocation_granularity": "whole-tensor",
  "gpu_context_bytes": 16384,
  "total_context_bytes": 32768,
  "gpu_resident_tensors": 1,
  "total_tensors": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f32le","dtype":"f32le","active_cells":4096,"embedding":1,"row_bytes":4}
  ]
}
"#,
    )
    .expect("write manifest");
    let output = root.join("runtime-reclaim-evidence.json");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let status = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1048576")
        .arg("--live-vram-runtime-reclaim-gate")
        .arg("--live-vram-min-gpu-saved-ratio")
        .arg("0.25")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run runtime-reclaim-gated qatq-kv-bench live vram mode");
    assert!(status.success());

    let json = fs::read_to_string(&output).expect("read evidence");
    assert!(json.contains("\"claim_scope\": \"runtime-kv-allocation-reclaim\""));
    assert!(json.contains("not token-page live paging"));
    assert!(json.contains("\"allocation_granularity\": \"whole-tensor\""));
    assert!(json.contains("\"gpu_context_bytes_before\": 32768"));
    assert!(json.contains("\"gpu_context_bytes_after\": 16384"));
    assert!(json.contains("\"reclaimable_gpu_bytes\": 16384"));
    assert!(json.contains("\"verified_restores\": 1"));
    assert!(json.contains("\"qatq_beats_best_general_codec_pages\": 1"));

    let _ = fs::remove_file(&output);
    let missing_key = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1048576")
        .arg("--live-vram-runtime-reclaim-gate")
        .arg("--live-vram-require-page-seals")
        .arg("--live-vram-min-gpu-saved-ratio")
        .arg("0.25")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .output()
        .expect("run sealed runtime-reclaim gate without seal key");
    assert!(!missing_key.status.success());
    let stderr = String::from_utf8_lossy(&missing_key.stderr);
    assert!(stderr.contains("--live-vram-require-page-seals"));
    assert!(stderr.contains("--live-vram-page-seal-key-hex"));
    assert!(!output.exists());

    let sealed_status = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1048576")
        .arg("--live-vram-runtime-reclaim-gate")
        .arg("--live-vram-page-seal-key-hex")
        .arg(LIVE_VRAM_TEST_SEAL_KEY_HEX)
        .arg("--live-vram-require-page-seals")
        .arg("--live-vram-min-gpu-saved-ratio")
        .arg("0.25")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run sealed runtime-reclaim gate");
    assert!(sealed_status.success());
    let sealed_json = fs::read_to_string(&output).expect("read sealed evidence");
    assert!(sealed_json.contains("\"sealed_pages\": 1"));
    assert!(sealed_json.contains("\"metadata_seal\": {\"version\": 1, \"tag\": \""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_live_vram_live_paging_gate_accepts_safe_event_trace() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-live-vram-live-paging-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create export dir");
    let tensor = compression_positive_f32_tensor_bytes();
    let checksum = live_vram_page_checksum(&tensor);
    fs::write(root.join("cache_k_l0_s0.f32le"), &tensor).expect("write tensor");
    fs::write(
        root.join("manifest.json"),
        r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 4096,
  "streams": 1,
  "live_page_residency_granularity": "per-page",
  "gpu_allocation_granularity": "per-page",
  "gpu_context_bytes": 16384,
  "total_context_bytes": 32768,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f32le","dtype":"f32le","active_cells":4096,"embedding":1,"row_bytes":4}
  ]
}
"#,
    )
    .expect("write manifest");
    let trace = root.join("trace.json");
    fs::write(&trace, live_vram_trace_json(checksum, false)).expect("write trace");
    let output = root.join("live-paging-evidence.json");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let missing_key = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1048576")
        .arg("--live-vram-event-trace")
        .arg(&trace)
        .arg("--live-vram-live-paging-gate")
        .arg("--live-vram-min-gpu-saved-ratio")
        .arg("0.25")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .output()
        .expect("run live-paging gate without seal key");
    assert!(!missing_key.status.success());
    let stderr = String::from_utf8_lossy(&missing_key.stderr);
    assert!(stderr.contains("--live-vram-page-seal-key-hex"));
    assert!(!output.exists());

    let status = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1048576")
        .arg("--live-vram-event-trace")
        .arg(&trace)
        .arg("--live-vram-live-paging-gate")
        .arg("--live-vram-page-seal-key-hex")
        .arg(LIVE_VRAM_TEST_SEAL_KEY_HEX)
        .arg("--live-vram-min-gpu-saved-ratio")
        .arg("0.25")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run live-paging-gated qatq-kv-bench live vram mode");
    assert!(status.success());

    let json = fs::read_to_string(&output).expect("read evidence");
    assert!(json.contains("\"claim_scope\": \"token-page-live-paging\""));
    assert!(json.contains("Strict live-paging evidence"));
    assert!(json.contains("\"allocation_granularity\": \"per-page\""));
    assert!(json.contains("\"gpu_context_bytes_before\": 32768"));
    assert!(json.contains("\"gpu_context_bytes_after\": 16384"));
    assert!(json.contains("\"event_trace_report\": {"));
    assert!(json.contains("\"passed\": true"));
    assert!(json.contains("\"attention_uses\": 1"));
    assert!(json.contains("\"offloaded_pages_at_end\": 0"));
    assert!(json.contains("\"sealed_pages\": 1"));
    assert!(json.contains("\"metadata_seal\": {\"version\": 1, \"tag\": \""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_live_vram_live_paging_gate_requires_logical_page_residency_manifest() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-live-vram-live-paging-missing-residency-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create export dir");
    let tensor = compression_positive_f32_tensor_bytes();
    let checksum = live_vram_page_checksum(&tensor);
    fs::write(root.join("cache_k_l0_s0.f32le"), &tensor).expect("write tensor");
    fs::write(
        root.join("manifest.json"),
        r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 4096,
  "streams": 1,
  "gpu_allocation_granularity": "per-page",
  "gpu_context_bytes": 16384,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f32le","dtype":"f32le","active_cells":4096,"embedding":1,"row_bytes":4}
  ]
}
"#,
    )
    .expect("write manifest");
    let trace = root.join("trace.json");
    fs::write(&trace, live_vram_trace_json(checksum, false)).expect("write trace");
    let output = root.join("live-paging-evidence.json");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let output_status = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1048576")
        .arg("--live-vram-event-trace")
        .arg(&trace)
        .arg("--live-vram-live-paging-gate")
        .arg("--live-vram-page-seal-key-hex")
        .arg(LIVE_VRAM_TEST_SEAL_KEY_HEX)
        .arg("--live-vram-min-gpu-saved-ratio")
        .arg("0.25")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .output()
        .expect("run live-paging-gated qatq-kv-bench live vram mode");
    assert!(!output_status.status.success());
    let stderr = String::from_utf8_lossy(&output_status.stderr);
    assert!(stderr.contains("live_page_residency_granularity"));
    assert!(!output.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_live_vram_event_trace_only_accepts_jsonl_and_gates_failures() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-live-vram-event-trace-only-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create trace dir");
    let tensor = compression_positive_f32_tensor_bytes();
    let checksum = live_vram_page_checksum(&tensor);
    let trace = root.join("attention-events.jsonl");
    fs::write(&trace, live_vram_trace_jsonl(checksum, false)).expect("write jsonl trace");
    let output = root.join("trace-report.json");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let status = Command::new(bin)
        .arg("--live-vram-event-trace-only")
        .arg("--live-vram-event-trace")
        .arg(&trace)
        .arg("--live-vram-event-trace-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run trace-only gate");
    assert!(status.success());

    let json = fs::read_to_string(&output).expect("read trace report");
    assert!(json.contains("\"format\": \"qatq-live-vram-event-trace-report-v1\""));
    assert!(json.contains("\"passed\": true"));
    assert!(json.contains("\"attention_uses\": 1"));

    fs::write(&trace, live_vram_trace_jsonl(checksum, true)).expect("write failing jsonl trace");
    let failed = Command::new(bin)
        .arg("--live-vram-event-trace-only")
        .arg("--live-vram-event-trace")
        .arg(&trace)
        .arg("--live-vram-event-trace-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run failing trace-only gate");
    assert!(!failed.success());
    assert!(json.contains("\"offloaded_pages_at_end\": 0"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_live_vram_event_trace_only_gates_explicit_cancellation_stages() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-live-vram-event-trace-cancel-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create trace dir");
    let tensor = compression_positive_f32_tensor_bytes();
    let checksum = live_vram_page_checksum(&tensor);
    let trace = root.join("cancellation-events.jsonl");
    let output = root.join("trace-report.json");
    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");

    fs::write(
        &trace,
        [
            live_vram_trace_event_json_line(0, "snapshot", checksum, true),
            live_vram_trace_event_json_line(1, "cancelled-before-runtime-commit", checksum, false),
            live_vram_trace_event_json_line(2, "attention-use", checksum, false),
        ]
        .join("\n")
            + "\n",
    )
    .expect("write before-commit cancellation trace");
    let before = Command::new(bin)
        .arg("--live-vram-event-trace-only")
        .arg("--live-vram-event-trace")
        .arg(&trace)
        .arg("--live-vram-event-trace-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run before-commit cancellation trace");
    assert!(before.success());

    fs::write(
        &trace,
        [
            live_vram_trace_event_json_line(0, "snapshot", checksum, true),
            live_vram_trace_event_json_line(1, "offload-committed", checksum, true),
            live_vram_trace_event_json_line(2, "cancelled-after-runtime-commit", checksum, true),
            live_vram_trace_event_json_line(3, "attention-use", checksum, false),
        ]
        .join("\n")
            + "\n",
    )
    .expect("write after-commit cancellation trace");
    let after = Command::new(bin)
        .arg("--live-vram-event-trace-only")
        .arg("--live-vram-event-trace")
        .arg(&trace)
        .arg("--live-vram-event-trace-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run after-commit cancellation trace");
    assert!(after.success());

    fs::write(
        &trace,
        [
            live_vram_trace_event_json_line(0, "snapshot", checksum, true),
            live_vram_trace_event_json_line(1, "offload-committed", checksum, true),
            live_vram_trace_event_json_line(2, "cancelled-after-runtime-commit", checksum, false),
            live_vram_trace_event_json_line(3, "attention-use", checksum, false),
        ]
        .join("\n")
            + "\n",
    )
    .expect("write missing-checksum cancellation trace");
    let missing_checksum = Command::new(bin)
        .arg("--live-vram-event-trace-only")
        .arg("--live-vram-event-trace")
        .arg(&trace)
        .arg("--live-vram-event-trace-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run missing-checksum cancellation trace");
    assert!(!missing_checksum.success());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_live_vram_live_paging_gate_rejects_attention_before_restore() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-live-vram-live-paging-fail-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create export dir");
    let tensor = compression_positive_f32_tensor_bytes();
    let checksum = live_vram_page_checksum(&tensor);
    fs::write(root.join("cache_k_l0_s0.f32le"), &tensor).expect("write tensor");
    fs::write(
        root.join("manifest.json"),
        r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 4096,
  "streams": 1,
  "live_page_residency_granularity": "per-page",
  "gpu_allocation_granularity": "per-page",
  "gpu_context_bytes": 16384,
  "total_context_bytes": 32768,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f32le","dtype":"f32le","active_cells":4096,"embedding":1,"row_bytes":4}
  ]
}
"#,
    )
    .expect("write manifest");
    let trace = root.join("trace.json");
    fs::write(&trace, live_vram_trace_json(checksum, true)).expect("write trace");
    let output = root.join("live-paging-evidence.json");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let output_status = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1048576")
        .arg("--live-vram-event-trace")
        .arg(&trace)
        .arg("--live-vram-live-paging-gate")
        .arg("--live-vram-page-seal-key-hex")
        .arg(LIVE_VRAM_TEST_SEAL_KEY_HEX)
        .arg("--live-vram-min-gpu-saved-ratio")
        .arg("0.25")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .output()
        .expect("run live-paging-gated qatq-kv-bench live vram mode");
    assert!(!output_status.status.success());
    let stderr = String::from_utf8_lossy(&output_status.stderr);
    assert!(stderr.contains("live VRAM live-paging gate failed"));
    assert!(stderr.contains("offloaded page"));
    assert!(!output.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_attention_equivalence_accepts_typed_f16_pages() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-attention-equivalence-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create attention dir");
    let query = root.join("query.f32le");
    let key0 = root.join("key0.f16le");
    let key1 = root.join("key1.f16le");
    let value0 = root.join("value0.f16le");
    let value1 = root.join("value1.f16le");
    let output = root.join("attention-evidence.json");

    fs::write(&query, f32_tensor_bytes(&[1.0, 0.0])).expect("write query");
    fs::write(&key0, f16_tensor_bytes(&[0x3c00, 0x0000, 0x0000, 0x3c00])).expect("write key0");
    fs::write(&key1, f16_tensor_bytes(&[0x3c00, 0x3c00])).expect("write key1");
    fs::write(&value0, f16_tensor_bytes(&[0x3c00, 0x0000, 0x0000, 0x3c00])).expect("write value0");
    fs::write(&value1, f16_tensor_bytes(&[0x3c00, 0x3c00])).expect("write value1");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let status = Command::new(bin)
        .arg("--attention-query")
        .arg(format!("f32:{}", query.display()))
        .arg("--attention-key-page")
        .arg(format!("f16:{}", key0.display()))
        .arg("--attention-key-page")
        .arg(format!("f16:{}", key1.display()))
        .arg("--attention-value-page")
        .arg(format!("f16:{}", value0.display()))
        .arg("--attention-value-page")
        .arg(format!("f16:{}", value1.display()))
        .arg("--attention-head-dim")
        .arg("2")
        .arg("--attention-value-dim")
        .arg("2")
        .arg("--attention-tolerance")
        .arg("0.000001")
        .arg("--attention-equivalence-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run attention equivalence gate");
    assert!(status.success());

    let json = fs::read_to_string(&output).expect("read attention evidence");
    assert!(json.contains("\"format\": \"qatq-live-vram-attention-equivalence-v1\""));
    assert!(json.contains("\"passed\": true"));
    assert!(json.contains("\"query_dtype\": \"f32\""));
    assert!(json.contains("\"page_dtype\": \"f16\""));
    assert!(json.contains("\"pages\": 2"));
    assert!(json.contains("\"tokens\": 3"));
    assert!(json.contains("\"peak_page_kv_values\": 8"));
    assert!(json.contains("\"materialized_kv_values\": 12"));
    assert!(json.contains("\"segment_summary_reduction\": \"online-page-summary\""));
    assert!(json.contains("\"segment_summary_passed\": true"));
    assert!(json.contains("\"segment_summary_peak_page_kv_values\": 8"));
    assert!(!json.contains("materialized_output"));
    assert!(!json.contains("\"output\""));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_attention_equivalence_rejects_malformed_pages_without_overwrite() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-attention-equivalence-fail-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create attention dir");
    let query = root.join("query.f32le");
    let key = root.join("key.f16le");
    let value = root.join("value.f16le");
    let output = root.join("attention-evidence.json");

    fs::write(&query, f32_tensor_bytes(&[1.0, 0.0])).expect("write query");
    fs::write(&key, [0_u8, 1_u8, 2_u8]).expect("write malformed key");
    fs::write(&value, f16_tensor_bytes(&[0x3c00, 0x0000])).expect("write value");
    let sentinel = b"keep-existing-attention-evidence";
    fs::write(&output, sentinel).expect("write sentinel output");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let output_status = Command::new(bin)
        .arg("--attention-query")
        .arg(format!("f32:{}", query.display()))
        .arg("--attention-key-page")
        .arg(format!("f16:{}", key.display()))
        .arg("--attention-value-page")
        .arg(format!("f16:{}", value.display()))
        .arg("--attention-head-dim")
        .arg("2")
        .arg("--attention-value-dim")
        .arg("2")
        .arg("--attention-equivalence-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .output()
        .expect("run malformed attention equivalence gate");
    assert!(!output_status.status.success());
    let stderr = String::from_utf8_lossy(&output_status.stderr);
    assert!(stderr.contains("failed to compare typed page-bounded attention"));
    assert_eq!(fs::read(&output).expect("read output"), sentinel);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_compares_llama_output_manifests_for_reduced_kv_behaviour() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-output-compare-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create manifest dir");
    let baseline = root.join("baseline.json");
    let candidate = root.join("candidate.json");
    let output = root.join("comparison.json");
    fs::write(
        &baseline,
        llama_output_manifest_json(-1, 1000, &[11, 22, 33]),
    )
    .expect("write baseline");
    fs::write(
        &candidate,
        llama_output_manifest_json(16, 1500, &[11, 22, 33]),
    )
    .expect("write candidate");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let status = Command::new(bin)
        .arg("--compare-output-baseline")
        .arg(&baseline)
        .arg("--compare-output-candidate")
        .arg(&candidate)
        .arg("--compare-output-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run output comparison");
    assert!(status.success());

    let json = fs::read_to_string(&output).expect("read comparison");
    assert!(json.contains("\"format\": \"qatq-llama-cpp-output-comparison-v1\""));
    assert!(json.contains("\"passed\": true"));
    assert!(json.contains("\"qatq_kv_gpu_layers\": -1"));
    assert!(json.contains("\"qatq_kv_gpu_layers\": 16"));
    assert!(json.contains("\"generated_token_count\": 3"));
    assert!(!json.contains("generated_text\":"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn kv_bench_output_manifest_gate_rejects_token_drift() {
    let root =
        std::env::temp_dir().join(format!("qatq-kv-bench-output-drift-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create manifest dir");
    let baseline = root.join("baseline.json");
    let candidate = root.join("candidate.json");
    let output = root.join("comparison.json");
    fs::write(
        &baseline,
        llama_output_manifest_json(-1, 1000, &[11, 22, 33]),
    )
    .expect("write baseline");
    fs::write(
        &candidate,
        llama_output_manifest_json(16, 1500, &[11, 23, 33]),
    )
    .expect("write candidate");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let output_status = Command::new(bin)
        .arg("--compare-output-baseline")
        .arg(&baseline)
        .arg("--compare-output-candidate")
        .arg(&candidate)
        .arg("--compare-output-gate")
        .arg("--output")
        .arg(&output)
        .stdout(Stdio::null())
        .output()
        .expect("run output comparison");
    assert!(!output_status.status.success());
    let stderr = String::from_utf8_lossy(&output_status.stderr);
    assert!(stderr.contains("llama.cpp output manifest comparison failed"));
    assert!(stderr.contains("generated_tokens differ at index 1"));
    assert!(!output.exists());

    let _ = fs::remove_dir_all(root);
}

fn llama_output_manifest_json(qatq_kv_gpu_layers: i64, total_us: u64, tokens: &[i64]) -> String {
    let tokens = tokens
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        r#"{{
  "format": "qatq-llama-cpp-output-v1",
  "model_path_hash": 123456,
  "prompt_hash": 987654,
  "n_prompt": 29,
  "n_predict": 32,
  "n_gpu_layers": 999,
  "offload_kqv": true,
  "qatq_kv_gpu_layers": {qatq_kv_gpu_layers},
  "cache_type_k": "f16",
  "cache_type_v": "f16",
  "n_decode": 3,
  "total_us": {total_us},
  "generated_text_hash": 424242,
  "generated_text": "same generated text",
  "generated_tokens": [{tokens}]
}}
"#
    )
}

fn compression_positive_f32_tensor_bytes() -> Vec<u8> {
    let mut tensor = Vec::new();
    for index in 0..4096_u32 {
        let value = ((index as f32) * 0.03125).sin();
        let value = f32::from_bits(value.to_bits() & 0xffff_0000);
        tensor.extend_from_slice(&value.to_le_bytes());
    }
    tensor
}

fn compression_positive_f16_tensor_bytes() -> Vec<u8> {
    let mut tensor = Vec::with_capacity(16_384);
    let mut state = 0x9e37_79b9_u32;
    for index in 0..8192_u16 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223) ^ index as u32;
        let low = (state >> 16) as u16;
        let high = 0x3c00_u16.wrapping_add((index % 4) << 8);
        tensor.extend_from_slice(&(high | (low & 0x00ff)).to_le_bytes());
    }
    tensor
}

fn f32_tensor_bytes(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn f16_tensor_bytes(values: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 2);
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn live_vram_trace_json(checksum: u64, attention_before_restore: bool) -> String {
    let ordered_events = if attention_before_restore {
        format!(
            r#"{snapshot},
    {offload},
    {attention},
    {restore}"#,
            snapshot = live_vram_trace_event_json(0, "snapshot", checksum, true),
            offload = live_vram_trace_event_json(1, "offload-committed", checksum, true),
            attention = live_vram_trace_event_json(2, "attention-use", checksum, false),
            restore = live_vram_trace_event_json(3, "restore-committed", checksum, true),
        )
    } else {
        format!(
            r#"{snapshot},
    {offload},
    {restore},
    {attention}"#,
            snapshot = live_vram_trace_event_json(0, "snapshot", checksum, true),
            offload = live_vram_trace_event_json(1, "offload-committed", checksum, true),
            restore = live_vram_trace_event_json(2, "restore-committed", checksum, true),
            attention = live_vram_trace_event_json(3, "attention-use", checksum, false),
        )
    };
    format!(
        r#"{{
  "format": "qatq-live-vram-event-trace-v1",
  "events": [
    {ordered_events}
  ]
}}
"#
    )
}

fn live_vram_trace_jsonl(checksum: u64, attention_before_restore: bool) -> String {
    let events = if attention_before_restore {
        [
            live_vram_trace_event_json_line(0, "snapshot", checksum, true),
            live_vram_trace_event_json_line(1, "offload-committed", checksum, true),
            live_vram_trace_event_json_line(2, "attention-use", checksum, false),
            live_vram_trace_event_json_line(3, "restore-committed", checksum, true),
        ]
    } else {
        [
            live_vram_trace_event_json_line(0, "snapshot", checksum, true),
            live_vram_trace_event_json_line(1, "offload-committed", checksum, true),
            live_vram_trace_event_json_line(2, "restore-committed", checksum, true),
            live_vram_trace_event_json_line(3, "attention-use", checksum, false),
        ]
    };
    events.join("\n") + "\n"
}

fn live_vram_trace_event_json_line(
    token: u64,
    event: &str,
    checksum: u64,
    include_checksum: bool,
) -> String {
    live_vram_trace_event_json(token, event, checksum, include_checksum)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn live_vram_trace_event_json(
    token: u64,
    event: &str,
    checksum: u64,
    include_checksum: bool,
) -> String {
    let checksum_field = if include_checksum {
        format!(
            r#",
      "checksum": {checksum}"#
        )
    } else {
        String::new()
    };
    format!(
        r#"{{
      "token": {token},
      "event": "{event}",
      "runtime_id": "llama.cpp",
      "model_id": "test-model.gguf:sha256:abc123",
      "seq_id": "0",
      "layer_id": 0,
      "kind": "key",
      "token_start": 0,
      "token_end": 4096{checksum_field}
    }}"#
    )
}

#[test]
fn kv_bench_live_vram_proof_gate_requires_runtime_allocator_attestation() {
    let root = std::env::temp_dir().join(format!(
        "qatq-kv-bench-live-vram-proof-attest-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create export dir");
    let mut tensor = Vec::new();
    for _ in 0..32 {
        tensor.extend_from_slice(&0x3c00_u16.to_le_bytes());
    }
    fs::write(root.join("cache_k_l0_s0.f16le"), &tensor).expect("write tensor");
    fs::write(
        root.join("manifest.json"),
        r#"{
  "format": "qatq-llama-cpp-kv-v1",
  "seq_id": 0,
  "kv_size": 16,
  "streams": 1,
  "tensors": [
    {"name":"cache_k_l0","kind":"k","stream":0,"file":"cache_k_l0_s0.f16le","dtype":"f16le","active_cells":4,"embedding":8,"row_bytes":16}
  ]
}
"#,
    )
    .expect("write manifest");

    let bin = env!("CARGO_BIN_EXE_qatq-kv-bench");
    let output_status = Command::new(bin)
        .arg("--live-vram-export-dir")
        .arg(&root)
        .arg("--live-vram-runtime-commit")
        .arg("7992aa7c8")
        .arg("--live-vram-adapter-version")
        .arg("qatq-kv-export-7992aa7c8")
        .arg("--live-vram-model-id")
        .arg("test-model.gguf:sha256:abc123")
        .arg("--live-vram-hot-window-tokens")
        .arg("0")
        .arg("--live-vram-gpu-context-bytes")
        .arg("64")
        .arg("--live-vram-allocation-granularity")
        .arg("per-page")
        .arg("--live-vram-restore-bytes-per-token")
        .arg("1048576")
        .arg("--live-vram-proof-gate")
        .stdout(Stdio::null())
        .output()
        .expect("run proof-gated qatq-kv-bench live vram mode");
    assert!(!output_status.status.success());
    let stderr = String::from_utf8_lossy(&output_status.stderr);
    assert!(stderr.contains("requires runtime-attested gpu_context_bytes"));

    let _ = fs::remove_dir_all(root);
}
