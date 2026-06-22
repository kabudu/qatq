use std::{
    fs,
    process::{Command, Stdio},
};

#[test]
fn benchmark_accepts_external_f32le_fixture() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let output = dir.join(format!("{stem}.md"));
    let values = [0.25_f32, -0.5, 1.0, 2.0, -3.5, 0.125, 0.75, -1.25];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--output")
        .arg(&output)
        .arg("--input")
        .arg(format!("tiny:{}", input.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run benchmark");
    assert!(status.success());

    let report = fs::read_to_string(&output).expect("read benchmark report");
    assert!(report.contains("fixture"));
    assert!(report.contains("tiny"));
    assert!(report.contains("zstd-raw-f32le"));
    assert!(report.contains("lz4-raw-f32le"));
    assert!(report.contains("turboquant-q4"));
    assert!(report.contains("phase1-q4"));
    assert!(report.contains("phase2 strategy"));
    assert!(report.contains("bit-delta"));
    assert!(report.contains("delta-xor-byte-plane-rle"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output);
}

#[test]
fn benchmark_accepts_manifest_and_writes_paper_summary() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-manifest-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let manifest = dir.join(format!("{stem}.manifest"));
    let output = dir.join(format!("{stem}.md"));
    let paper_output = dir.join(format!("{stem}.paper.md"));
    let quality_output = dir.join(format!("{stem}.quality.md"));
    let task_quality_output = dir.join(format!("{stem}.task-quality.md"));
    let values = [
        0.125_f32, -0.25, 0.5, -1.0, 2.0, -4.0, 0.75, 1.25, 0.375, -0.625, 1.5, -2.5, 3.0, -3.25,
        0.875, -1.125, -0.125, 0.25, -0.5, 1.0, -2.0, 4.0, -0.75, -1.25, -0.375, 0.625, -1.5, 2.5,
        -3.0, 3.25, -0.875, 1.125,
    ];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");
    fs::write(
        &manifest,
        format!(
            "[fixture]\nname = tiny-kv\ngroup = runtime-kv\npath = \"{}\"\nshape = \"[1, 8]\"\nnotes = \"test fixture\"\n",
            input.file_name().unwrap().to_string_lossy()
        ),
    )
    .expect("write manifest");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--output")
        .arg(&output)
        .arg("--paper-output")
        .arg(&paper_output)
        .arg("--quality-output")
        .arg(&quality_output)
        .arg("--task-quality-output")
        .arg(&task_quality_output)
        .arg("--no-synthetic")
        .arg("--manifest")
        .arg(&manifest)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run benchmark");
    assert!(status.success());

    let report = fs::read_to_string(&output).expect("read benchmark report");
    assert!(report.contains("runtime-kv"));
    assert!(report.contains("tiny-kv"));

    let paper = fs::read_to_string(&paper_output).expect("read paper report");
    assert!(paper.contains("# Paper Tables"));
    assert!(paper.contains("Base TurboQuant Reference Versus Quaternion Overlay"));
    assert!(paper.contains("runtime-kv"));

    let quality = fs::read_to_string(&quality_output).expect("read quality report");
    assert!(quality.contains("# QATQ Quality Experiments"));
    assert!(quality.contains("turboquant-q4"));
    assert!(quality.contains("phase1-q4"));
    assert!(quality.contains("Inner-Product Error Summary"));
    assert!(quality.contains("runtime-kv"));

    let task_quality = fs::read_to_string(&task_quality_output).expect("read task report");
    assert!(task_quality.contains("# QATQ Task Quality Experiments"));
    assert!(task_quality.contains("Retrieval Top-1 Agreement"));
    assert!(task_quality.contains("phase2-lossless"));
    assert!(task_quality.contains("100.00%"));
    assert!(task_quality.contains("runtime-kv"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(manifest);
    let _ = fs::remove_file(output);
    let _ = fs::remove_file(paper_output);
    let _ = fs::remove_file(quality_output);
    let _ = fs::remove_file(task_quality_output);
}

#[test]
fn benchmark_failed_input_does_not_overwrite_existing_reports() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-failed-input-{}", std::process::id());
    let missing = dir.join(format!("{stem}.missing.f32le"));
    let output = dir.join(format!("{stem}.md"));
    let paper_output = dir.join(format!("{stem}.paper.md"));
    let quality_output = dir.join(format!("{stem}.quality.md"));
    let task_quality_output = dir.join(format!("{stem}.task-quality.md"));
    let gate = dir.join(format!("{stem}.gate.md"));
    let output_sentinel = b"keep-existing-benchmark-report";
    let paper_sentinel = b"keep-existing-paper-report";
    let quality_sentinel = b"keep-existing-quality-report";
    let task_quality_sentinel = b"keep-existing-task-quality-report";
    let gate_sentinel = b"keep-existing-gate-report";
    fs::write(&output, output_sentinel).expect("write output sentinel");
    fs::write(&paper_output, paper_sentinel).expect("write paper sentinel");
    fs::write(&quality_output, quality_sentinel).expect("write quality sentinel");
    fs::write(&task_quality_output, task_quality_sentinel).expect("write task quality sentinel");
    fs::write(&gate, gate_sentinel).expect("write gate sentinel");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--output")
        .arg(&output)
        .arg("--paper-output")
        .arg(&paper_output)
        .arg("--quality-output")
        .arg(&quality_output)
        .arg("--task-quality-output")
        .arg(&task_quality_output)
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--input")
        .arg(format!("missing:{}", missing.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run benchmark");
    assert!(!status.success());

    assert_eq!(fs::read(&output).expect("read output"), output_sentinel);
    assert_eq!(
        fs::read(&paper_output).expect("read paper output"),
        paper_sentinel
    );
    assert_eq!(
        fs::read(&quality_output).expect("read quality output"),
        quality_sentinel
    );
    assert_eq!(
        fs::read(&task_quality_output).expect("read task quality output"),
        task_quality_sentinel
    );
    assert_eq!(fs::read(&gate).expect("read gate"), gate_sentinel);

    let _ = fs::remove_file(output);
    let _ = fs::remove_file(paper_output);
    let _ = fs::remove_file(quality_output);
    let _ = fs::remove_file(task_quality_output);
    let _ = fs::remove_file(gate);
}

#[test]
fn benchmark_malformed_f32le_input_does_not_overwrite_existing_reports() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-malformed-input-{}", std::process::id());
    let input = dir.join(format!("{stem}.bad"));
    let output = dir.join(format!("{stem}.md"));
    let paper_output = dir.join(format!("{stem}.paper.md"));
    let quality_output = dir.join(format!("{stem}.quality.md"));
    let task_quality_output = dir.join(format!("{stem}.task-quality.md"));
    let gate = dir.join(format!("{stem}.gate.md"));
    fs::write(&input, [1_u8, 2, 3]).expect("write malformed input");
    let output_sentinel = b"keep-existing-benchmark-report";
    let paper_sentinel = b"keep-existing-paper-report";
    let quality_sentinel = b"keep-existing-quality-report";
    let task_quality_sentinel = b"keep-existing-task-quality-report";
    let gate_sentinel = b"keep-existing-gate-report";
    fs::write(&output, output_sentinel).expect("write output sentinel");
    fs::write(&paper_output, paper_sentinel).expect("write paper sentinel");
    fs::write(&quality_output, quality_sentinel).expect("write quality sentinel");
    fs::write(&task_quality_output, task_quality_sentinel).expect("write task quality sentinel");
    fs::write(&gate, gate_sentinel).expect("write gate sentinel");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--output")
        .arg(&output)
        .arg("--paper-output")
        .arg(&paper_output)
        .arg("--quality-output")
        .arg(&quality_output)
        .arg("--task-quality-output")
        .arg(&task_quality_output)
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--input")
        .arg(format!("malformed:{}", input.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run benchmark");
    assert!(!status.success());

    assert_eq!(fs::read(&output).expect("read output"), output_sentinel);
    assert_eq!(
        fs::read(&paper_output).expect("read paper output"),
        paper_sentinel
    );
    assert_eq!(
        fs::read(&quality_output).expect("read quality output"),
        quality_sentinel
    );
    assert_eq!(
        fs::read(&task_quality_output).expect("read task quality output"),
        task_quality_sentinel
    );
    assert_eq!(fs::read(&gate).expect("read gate"), gate_sentinel);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output);
    let _ = fs::remove_file(paper_output);
    let _ = fs::remove_file(quality_output);
    let _ = fs::remove_file(task_quality_output);
    let _ = fs::remove_file(gate);
}

#[test]
fn benchmark_manifest_failure_after_partial_work_preserves_reports() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-partial-manifest-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let missing = dir.join(format!("{stem}.missing.f32le"));
    let manifest = dir.join(format!("{stem}.manifest"));
    let output = dir.join(format!("{stem}.md"));
    let paper_output = dir.join(format!("{stem}.paper.md"));
    let quality_output = dir.join(format!("{stem}.quality.md"));
    let gate = dir.join(format!("{stem}.gate.md"));
    let values = [0.125_f32, -0.25, 0.5, -1.0, 2.0, -4.0, 0.75, 1.25];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");
    fs::write(
        &manifest,
        format!(
            "[fixture]\nname = valid-first\ngroup = runtime-kv\npath = \"{}\"\n\n[fixture]\nname = missing-second\ngroup = runtime-kv\npath = \"{}\"\n",
            input.file_name().unwrap().to_string_lossy(),
            missing.file_name().unwrap().to_string_lossy()
        ),
    )
    .expect("write manifest");
    let output_sentinel = b"keep-existing-benchmark-report";
    let paper_sentinel = b"keep-existing-paper-report";
    let quality_sentinel = b"keep-existing-quality-report";
    let gate_sentinel = b"keep-existing-gate-report";
    fs::write(&output, output_sentinel).expect("write output sentinel");
    fs::write(&paper_output, paper_sentinel).expect("write paper sentinel");
    fs::write(&quality_output, quality_sentinel).expect("write quality sentinel");
    fs::write(&gate, gate_sentinel).expect("write gate sentinel");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--output")
        .arg(&output)
        .arg("--paper-output")
        .arg(&paper_output)
        .arg("--quality-output")
        .arg(&quality_output)
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--manifest")
        .arg(&manifest)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run benchmark");
    assert!(!status.success());

    assert_eq!(fs::read(&output).expect("read output"), output_sentinel);
    assert_eq!(
        fs::read(&paper_output).expect("read paper output"),
        paper_sentinel
    );
    assert_eq!(
        fs::read(&quality_output).expect("read quality output"),
        quality_sentinel
    );
    assert_eq!(fs::read(&gate).expect("read gate"), gate_sentinel);

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(manifest);
    let _ = fs::remove_file(output);
    let _ = fs::remove_file(paper_output);
    let _ = fs::remove_file(quality_output);
    let _ = fs::remove_file(gate);
}

#[test]
fn benchmark_gate_passes_with_loose_thresholds() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-gate-pass-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let gate = dir.join(format!("{stem}.gate.md"));
    let values = [0.0_f32, 0.25, -0.5, 1.0, 2.0, -4.0, 8.0, -16.0];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--input")
        .arg(format!("gate-pass:{}", input.display()))
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--gate-require-external")
        .arg("--max-phase2-ratio")
        .arg("10.0")
        .arg("--max-phase2-encode-us")
        .arg("1000000")
        .arg("--max-phase2-decode-us")
        .arg("1000000")
        .arg("--max-phase2-decode-ns-per-value")
        .arg("1000000")
        .arg("--max-phase2-container-ratio")
        .arg("10.0")
        .arg("--max-phase2-container-decode-us")
        .arg("1000000")
        .arg("--max-phase2-container-decode-ns-per-value")
        .arg("1000000")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run benchmark gate");
    assert!(status.success());

    let report = fs::read_to_string(&gate).expect("read gate report");
    assert!(report.contains("status: `pass`"));
    assert!(report.contains("gate-pass"));
    assert!(report.contains("phase2-lossless-container"));
    assert!(report.contains("ns/value"));
    assert!(report.contains("exact_bits=true"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(gate);
}

#[test]
fn benchmark_production_kv_gate_requires_throughput_decode_thresholds() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-production-gate-policy-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let gate = dir.join(format!("{stem}.gate.md"));
    let values = vec![0.0_f32; 128];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--input")
        .arg(format!("production-policy:{}", input.display()))
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--phase2-only")
        .arg("--gate-policy")
        .arg("production-kv")
        .arg("--max-phase2-ratio")
        .arg("10.0")
        .arg("--max-phase2-decode-us")
        .arg("1000000")
        .arg("--max-phase2-container-ratio")
        .arg("10.0")
        .arg("--max-phase2-container-decode-ns-per-value")
        .arg("1000000")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run production policy gate");
    assert!(!status.success());

    let report = fs::read_to_string(&gate).expect("read gate report");
    assert!(report.contains("status: `fail`"));
    assert!(report.contains("policy: `production-kv`"));
    assert!(report.contains("production KV readiness requires decode ns/value ceilings"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(gate);
}

#[test]
fn benchmark_competitive_compression_gate_compares_phase2_to_zstd_lz4() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-competitive-gate-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let gate = dir.join(format!("{stem}.gate.md"));
    let mut input_bytes = Vec::new();
    for index in 0..4096_u32 {
        let value = ((index as f32) * 0.03125).sin();
        let value = f32::from_bits(value.to_bits() & 0xffff_0000);
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--input")
        .arg(format!("competitive-pass:{}", input.display()))
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--phase2-only")
        .arg("--gate-policy")
        .arg("competitive-compression")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run competitive compression gate");
    assert!(status.success());

    let report = fs::read_to_string(&gate).expect("read gate report");
    assert!(report.contains("status: `pass`"));
    assert!(report.contains("policy: `competitive-compression`"));
    assert!(report.contains("competitive ratio"));
    assert!(report.contains("best(zstd"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(gate);
}

#[test]
fn benchmark_production_kv_gate_passes_with_throughput_policy() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-production-gate-pass-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let gate = dir.join(format!("{stem}.gate.md"));
    let values = vec![0.0_f32; 128];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--input")
        .arg(format!("production-pass:{}", input.display()))
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--phase2-only")
        .arg("--gate-policy")
        .arg("production-kv")
        .arg("--max-phase2-ratio")
        .arg("10.0")
        .arg("--max-phase2-decode-ns-per-value")
        .arg("1000000")
        .arg("--max-phase2-container-ratio")
        .arg("10.0")
        .arg("--max-phase2-container-decode-ns-per-value")
        .arg("1000000")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run production policy gate");
    assert!(status.success());

    let report = fs::read_to_string(&gate).expect("read gate report");
    assert!(report.contains("status: `pass`"));
    assert!(report.contains("policy: `production-kv`"));
    assert!(report.contains("production readiness for mixed-size external KV tensors"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(gate);
}

#[test]
fn benchmark_phase2_only_limits_report_to_gated_rows() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-phase2-only-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let output = dir.join(format!("{stem}.md"));
    let values = [0.0_f32, 0.25, -0.5, 1.0, 2.0, -4.0, 8.0, -16.0];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--output")
        .arg(&output)
        .arg("--input")
        .arg(format!("phase2-only:{}", input.display()))
        .arg("--no-synthetic")
        .arg("--phase2-only")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run phase2-only benchmark");
    assert!(status.success());

    let report = fs::read_to_string(&output).expect("read benchmark report");
    assert!(report.contains("benchmark mode: `phase2-only`"));
    assert!(report.contains("zstd-raw-f32le"));
    assert!(report.contains("lz4-raw-f32le"));
    assert!(report.contains("phase2-lossless"));
    assert!(report.contains("phase2-lossless-container"));
    assert!(!report.contains("| lossless-f32 |"));
    assert!(!report.contains("| phase1-q4 |"));
    assert!(!report.contains("| lossy-i4 |"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output);
}

#[test]
fn benchmark_phase2_only_paper_summary_keeps_lossless_envelope_rows() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-phase2-only-paper-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let paper = dir.join(format!("{stem}.paper.md"));
    let values = [0.0_f32, 0.25, -0.5, 1.0, 2.0, -4.0, 8.0, -16.0];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--paper-output")
        .arg(&paper)
        .arg("--input")
        .arg(format!("phase2-only-paper:{}", input.display()))
        .arg("--no-synthetic")
        .arg("--phase2-only")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run phase2-only paper benchmark");
    assert!(status.success());

    let report = fs::read_to_string(&paper).expect("read paper report");
    assert!(report.contains("## Lossless Envelope Comparison"));
    assert!(report.contains("phase2-only-paper"));
    assert!(report.contains("raw-f32"));
    assert!(report.contains("phase2-lossless ratio"));
    assert!(!report.contains("missing comparison rows"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(paper);
}

#[test]
fn benchmark_gate_treats_raw_bits_as_no_compress_bypass() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-no-compress-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let gate = dir.join(format!("{stem}.gate.md"));
    let mut input_bytes = Vec::new();
    for index in 0..128_u32 {
        let bits = index.wrapping_mul(0x9e37_79b9).rotate_left(index % 31) ^ 0x1357_2468;
        input_bytes.extend_from_slice(&f32::from_bits(bits).to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--input")
        .arg(format!("no-compress:{}", input.display()))
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--phase2-only")
        .arg("--max-phase2-ratio")
        .arg("0.01")
        .arg("--max-phase2-container-ratio")
        .arg("0.01")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run no-compress benchmark gate");
    assert!(status.success());

    let report = fs::read_to_string(&gate).expect("read gate report");
    assert!(report.contains("status: `pass`"));
    assert!(report.contains("raw-bits"));
    assert!(report.contains("no-compress bypass selected"));
    assert!(report.contains("exact_bits=true"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(gate);
}

#[test]
fn benchmark_gate_fails_with_strict_container_threshold() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-container-gate-fail-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let gate = dir.join(format!("{stem}.gate.md"));
    let values = [1.0_f32, 2.0, 3.0, 4.0];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--input")
        .arg(format!("container-gate-fail:{}", input.display()))
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--max-phase2-ratio")
        .arg("10.0")
        .arg("--max-phase2-container-ratio")
        .arg("0.01")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run benchmark gate");
    assert!(!status.success());

    let report = fs::read_to_string(&gate).expect("read gate report");
    assert!(report.contains("status: `fail`"));
    assert!(report.contains("container-gate-fail"));
    assert!(report.contains("phase2-lossless-container"));
    assert!(report.contains("ratio"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(gate);
}

#[test]
fn benchmark_gate_fails_with_strict_ratio_threshold() {
    let dir = std::env::temp_dir();
    let stem = format!("qatq-bench-gate-fail-{}", std::process::id());
    let input = dir.join(format!("{stem}.f32le"));
    let gate = dir.join(format!("{stem}.gate.md"));
    let values = vec![0.0_f32; 128];
    let mut input_bytes = Vec::new();
    for value in values {
        input_bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(&input, input_bytes).expect("write fixture");

    let bin = env!("CARGO_BIN_EXE_qatq-bench");
    let status = Command::new(bin)
        .arg("--input")
        .arg(format!("gate-fail:{}", input.display()))
        .arg("--gate-output")
        .arg(&gate)
        .arg("--no-synthetic")
        .arg("--max-phase2-ratio")
        .arg("0.01")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run benchmark gate");
    assert!(!status.success());

    let report = fs::read_to_string(&gate).expect("read gate report");
    assert!(report.contains("status: `fail`"));
    assert!(report.contains("gate-fail"));
    assert!(report.contains("ratio"));

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(gate);
}
