use std::{env, fs, path::Path};

use qatq::oracle::{
    CertificateCheck, ImpossibilityCertificate, OracleOutcome, check_certificate_json, evaluate,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("qatq-oracle: {error}");
            5
        }
    };
    std::process::exit(code);
}

fn run() -> Result<i32, String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("evaluate" | "bound") => evaluate_command(&args[1..]),
        Some("construct") => {
            eprintln!("qatq-oracle: construction search is not supported in this release");
            Ok(3)
        }
        Some("check") => check_command(&args[1..]),
        Some("inspect") => inspect_command(&args[1..]),
        Some("derive-kv") => {
            eprintln!("qatq-oracle: KV derivation is not supported in this release");
            Ok(3)
        }
        _ => {
            print_usage();
            Ok(3)
        }
    }
}

fn evaluate_command(args: &[String]) -> Result<i32, String> {
    let (input, output) = match args {
        [input] => (input.as_str(), None),
        [input, flag, output] if flag == "--output" => (input.as_str(), Some(output.as_str())),
        _ => {
            print_usage();
            return Ok(3);
        }
    };
    let bytes = fs::read(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    let result = evaluate(&bytes);
    let rendered = serde_json::to_vec_pretty(&result.outcome)
        .map_err(|error| format!("failed to serialize outcome: {error}"))?;
    if let Some(output) = output {
        write_bundle(output, &result, &rendered)?;
    } else {
        println!("{}", String::from_utf8_lossy(&rendered));
    }
    Ok(result.outcome.exit_code())
}

fn check_command(args: &[String]) -> Result<i32, String> {
    let [input] = args else {
        print_usage();
        return Ok(4);
    };
    let bytes = fs::read(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    let result = check_certificate_json(&bytes, 16 * 1024 * 1024);
    println!(
        "{}",
        serde_json::to_string_pretty(&result)
            .map_err(|error| format!("failed to serialize check result: {error}"))?
    );
    Ok(if result == CertificateCheck::Valid {
        0
    } else {
        4
    })
}

fn inspect_command(args: &[String]) -> Result<i32, String> {
    let [input] = args else {
        print_usage();
        return Ok(4);
    };
    let bytes = fs::read(input).map_err(|error| format!("failed to read {input}: {error}"))?;
    let certificate: ImpossibilityCertificate =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid certificate: {error}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&certificate)
            .map_err(|error| format!("failed to serialize certificate: {error}"))?
    );
    Ok(0)
}

#[derive(Serialize)]
struct ManifestEntry {
    path: String,
    sha256: String,
    bytes: usize,
}

#[derive(Serialize)]
struct EvidenceManifest {
    schema_version: u32,
    files: Vec<ManifestEntry>,
}

fn write_bundle(
    output: &str,
    result: &qatq::oracle::EvaluationResult,
    outcome_bytes: &[u8],
) -> Result<(), String> {
    let directory = Path::new(output);
    if directory.exists() {
        return Err(format!("refusing to overwrite {}", directory.display()));
    }
    let parent = directory.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let name = directory
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "output directory must have a valid UTF-8 name".to_string())?;
    let temporary = parent.join(format!(".{name}.qatq-oracle-{}", std::process::id()));
    if temporary.exists() {
        return Err(format!(
            "temporary output path already exists: {}",
            temporary.display()
        ));
    }
    fs::create_dir(&temporary)
        .map_err(|error| format!("failed to create {}: {error}", temporary.display()))?;
    let operation = write_bundle_contents(&temporary, result, outcome_bytes).and_then(|()| {
        fs::rename(&temporary, directory).map_err(|error| {
            format!(
                "failed to publish {} atomically from {}: {error}",
                directory.display(),
                temporary.display()
            )
        })
    });
    if let Err(error) = operation {
        return match fs::remove_dir_all(&temporary) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(format!(
                "{error}; additionally failed to clean {}: {cleanup}",
                temporary.display()
            )),
        };
    }
    Ok(())
}

fn write_bundle_contents(
    temporary: &Path,
    result: &qatq::oracle::EvaluationResult,
    outcome_bytes: &[u8],
) -> Result<(), String> {
    let normalized = pretty(&result.normalized_request)?;
    let certificate = pretty(&result.certificate)?;
    let metrics = match &result.outcome {
        OracleOutcome::Unknown(report) => pretty(&report.metadata.resources)?,
        OracleOutcome::Refused(report) => pretty(&report.metadata.resources)?,
        OracleOutcome::Constructed(report) => pretty(&report.metadata.resources)?,
        OracleOutcome::InfeasibleUnderModel(report) => pretty(&report.metadata.resources)?,
    };
    let files = [
        ("request.normalized.json", normalized),
        ("outcome.json", outcome_bytes.to_vec()),
        ("certificate.json", certificate),
        ("report.md", render_report(&result.outcome).into_bytes()),
        ("metrics.json", metrics),
    ];
    let mut entries = Vec::new();
    for (name, bytes) in files {
        fs::write(temporary.join(name), &bytes)
            .map_err(|error| format!("failed to write {name}: {error}"))?;
        entries.push(ManifestEntry {
            path: name.into(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes: bytes.len(),
        });
    }
    let manifest = pretty(&EvidenceManifest {
        schema_version: 1,
        files: entries,
    })?;
    fs::write(temporary.join("manifest.json"), manifest)
        .map_err(|error| format!("failed to write manifest: {error}"))
}

fn pretty<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(value).map_err(|error| format!("serialization failed: {error}"))
}

fn render_report(outcome: &OracleOutcome) -> String {
    match outcome {
        OracleOutcome::InfeasibleUnderModel(report) => format!(
            "# QATQ Capacity Oracle report\n\nOutcome: `INFEASIBLE_UNDER_MODEL`\n\nThe required state count `{}` is strictly greater than the independently checkable finite upper bound `{}` under theorem `{:?}`.\n",
            report.certificate.required_states,
            report.certificate.claimed_upper_bound,
            report.certificate.theorem
        ),
        OracleOutcome::Unknown(report) => format!(
            "# QATQ Capacity Oracle report\n\nOutcome: `UNKNOWN`\n\n{}\n",
            report.reason
        ),
        OracleOutcome::Refused(report) => format!(
            "# QATQ Capacity Oracle report\n\nOutcome: `REFUSED`\n\n{}\n",
            report.reason
        ),
        OracleOutcome::Constructed(_) => {
            "# QATQ Capacity Oracle report\n\nOutcome: `CONSTRUCTED`\n".into()
        }
    }
}

fn print_usage() {
    eprintln!(
        "usage: qatq-oracle <evaluate|construct|bound> <request.json> [--output <directory>]\n       qatq-oracle check <certificate.json>\n       qatq-oracle inspect <certificate.json>\n       qatq-oracle derive-kv <capture> --config <config.json> --output <request.json>"
    );
}
