#![cfg(feature = "oracle")]

use std::{fs, process::Command};

use sha2::{Digest, Sha256};

#[test]
fn finite_binary_evaluate_exits_infeasible() {
    let output = Command::new(env!("CARGO_BIN_EXE_qatq-oracle"))
        .arg("evaluate")
        .arg("examples/oracle/binary-128-d48-48bit.json")
        .output()
        .expect("run qatq-oracle");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"outcome\": \"INFEASIBLE_UNDER_MODEL\""));
}

#[test]
fn check_rejects_missing_certificate_as_tool_failure() {
    let output = Command::new(env!("CARGO_BIN_EXE_qatq-oracle"))
        .arg("check")
        .arg("missing.json")
        .output()
        .expect("run qatq-oracle");
    assert_eq!(output.status.code(), Some(5));
}

#[test]
fn evidence_bundle_is_atomic_and_certificate_checks() {
    let directory =
        std::env::temp_dir().join(format!("qatq-oracle-bundle-{}-{}", std::process::id(), 1));
    if directory.exists() {
        fs::remove_dir_all(&directory).expect("remove stale test directory");
    }
    let binary = env!("CARGO_BIN_EXE_qatq-oracle");
    let output = Command::new(binary)
        .arg("bound")
        .arg("examples/oracle/binary-128-d48-48bit.json")
        .arg("--output")
        .arg(&directory)
        .output()
        .expect("run bound");
    assert_eq!(output.status.code(), Some(1));
    for name in [
        "request.normalized.json",
        "outcome.json",
        "certificate.json",
        "report.md",
        "metrics.json",
        "manifest.json",
    ] {
        assert!(directory.join(name).is_file(), "missing {name}");
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    for entry in manifest["files"].as_array().unwrap() {
        let name = entry["path"].as_str().unwrap();
        let bytes = fs::read(directory.join(name)).unwrap();
        assert_eq!(entry["bytes"].as_u64().unwrap(), bytes.len() as u64);
        assert_eq!(
            entry["sha256"].as_str().unwrap(),
            format!("{:x}", Sha256::digest(bytes))
        );
    }

    let check = Command::new(binary)
        .arg("check")
        .arg(directory.join("certificate.json"))
        .output()
        .expect("check certificate");
    assert_eq!(check.status.code(), Some(0));
    assert!(String::from_utf8(check.stdout).unwrap().contains("VALID"));

    let second = Command::new(binary)
        .arg("bound")
        .arg("examples/oracle/binary-128-d48-48bit.json")
        .arg("--output")
        .arg(&directory)
        .output()
        .expect("rerun bound");
    assert_eq!(second.status.code(), Some(5));
    fs::remove_dir_all(directory).expect("remove test directory");
}
