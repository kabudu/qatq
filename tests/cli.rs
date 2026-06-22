use std::{
    fs,
    process::{Command, Stdio},
};

use qatq::MAX_VALUES_PER_PAYLOAD;

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
fn cli_encodes_and_decodes_phase2_lossless_with_seed_exactly() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-phase2-{}", std::process::id());
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
        .arg("phase2-lossless")
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
fn cli_corrupt_qatq_decode_does_not_overwrite_existing_output() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-cli-corrupt-phase2-{}", std::process::id());
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
        .arg("phase2-lossless")
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
        .arg("phase2-lossless")
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
        .arg("phase2-lossless")
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
fn cli_encodes_chunked_phase2_container_and_decodes_exactly() {
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
