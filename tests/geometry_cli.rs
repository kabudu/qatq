#![cfg(feature = "geometry")]

use std::fs;
use std::process::Command;

fn fixture() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "qatq-geometry-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let capture = root.join("capture.kv");
    let metadata = root.join("capture.json");
    let bytes: Vec<u8> = [[1.0_f32, 0.0], [0.0, 1.0], [-1.0, 0.0], [1.0, 0.0]]
        .into_iter()
        .flatten()
        .flat_map(f32::to_le_bytes)
        .collect();
    fs::write(&capture, bytes).unwrap();
    fs::write(
        &metadata,
        r#"{
  "schema_version": 1,
  "capture_id": "cli-fixture",
  "model": "fixture",
  "model_family": "fixture",
  "runtime": "fixture",
  "runtime_version": "1",
  "prompt_class": "factual",
  "prompt_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
  "context_length": 4,
  "dtype": "f32",
  "tensors": [{
    "id": "k-l0",
    "offset_bytes": 0,
    "byte_length": 32,
    "layer": 0,
    "kind": "key",
    "rope_stage": "unknown",
    "token_start": 0,
    "token_count": 4,
    "heads": 1,
    "dimension": 2,
    "layout": "token_head_dimension"
  }]
}
"#,
    )
    .unwrap();
    (root, capture, metadata)
}

#[test]
fn profile_writes_and_verifies_observation_only_bundle() {
    let (root, capture, metadata) = fixture();
    let output = root.join("results");
    let binary = env!("CARGO_BIN_EXE_qatq-kv-geometry");
    let status = Command::new(binary)
        .args([
            "profile",
            "--capture",
            capture.to_str().unwrap(),
            "--metadata",
            metadata.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    for name in [
        "capture-manifest.json",
        "geometry.json",
        "summary.md",
        "sampling-plan.json",
        "metrics.json",
        "manifest.json",
    ] {
        assert!(output.join(name).is_file());
    }
    let geometry = fs::read_to_string(output.join("geometry.json")).unwrap();
    for forbidden in ["INFEASIBLE_UNDER_MODEL", "CONSTRUCTED", "UNKNOWN"] {
        assert!(!geometry.contains(forbidden));
    }
    assert!(
        Command::new(binary)
            .args(["verify", output.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn dimension_policy_publishes_refused_bundle() {
    let (root, capture, metadata) = fixture();
    let output = root.join("refused");
    let status = Command::new(env!("CARGO_BIN_EXE_qatq-kv-geometry"))
        .args([
            "profile",
            "--capture",
            capture.to_str().unwrap(),
            "--metadata",
            metadata.to_str().unwrap(),
            "--max-dimension",
            "1",
            "--output",
            output.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let geometry = fs::read_to_string(output.join("geometry.json")).unwrap();
    assert!(geometry.contains("\"status\": \"REFUSED\""));
    fs::remove_dir_all(root).unwrap();
}
