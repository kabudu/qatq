use std::path::PathBuf;

use qatq::geometry::{
    GeometryError, ProfilePolicy, parse_partition, profile_capture, verify_bundle,
    write_profile_bundle,
};

fn main() {
    if let Err(error) = run(std::env::args().skip(1).collect()) {
        eprintln!("qatq-kv-geometry: {error}");
        std::process::exit(2);
    }
}

fn run(args: Vec<String>) -> Result<(), GeometryError> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err(usage());
    };
    match command {
        "profile" => profile(&args[1..]),
        "verify" => verify(&args[1..]),
        "--help" | "-h" | "help" => {
            println!("{}", help());
            Ok(())
        }
        other => Err(GeometryError::Invalid(format!(
            "unsupported command {other}\n\n{}",
            help()
        ))),
    }
}

fn profile(args: &[String]) -> Result<(), GeometryError> {
    let mut capture = None;
    let mut metadata = None;
    let mut output = None;
    let mut policy = ProfilePolicy::default();
    let mut index = 0;
    while index < args.len() {
        let flag = &args[index];
        if flag == "--help" || flag == "-h" {
            println!("{}", help());
            return Ok(());
        }
        let value = args
            .get(index + 1)
            .ok_or_else(|| GeometryError::Invalid(format!("missing value for {flag}")))?;
        match flag.as_str() {
            "--capture" => capture = Some(PathBuf::from(value)),
            "--metadata" => metadata = Some(PathBuf::from(value)),
            "--output" => output = Some(PathBuf::from(value)),
            "--partition" => policy.partition = parse_partition(value)?,
            "--normalization" if value == "unit-l2" => {}
            "--normalization" => {
                return Err(GeometryError::Invalid(format!(
                    "unsupported normalization {value}"
                )));
            }
            "--seed" => policy.seed = number(flag, value)?,
            "--max-pairs" => policy.max_pairs = number(flag, value)?,
            "--exact-vector-threshold" => policy.exact_vector_threshold = number(flag, value)?,
            "--max-capture-bytes" => policy.max_capture_bytes = number(flag, value)?,
            "--max-vectors" => policy.max_vectors = number(flag, value)?,
            "--max-scalar-values" => policy.max_scalar_values = number(flag, value)?,
            "--max-dimension" => policy.max_dimension = number(flag, value)?,
            "--max-spectral-dimension" => policy.max_spectral_dimension = number(flag, value)?,
            "--chunk-tokens" => policy.chunk_tokens = number(flag, value)?,
            "--block-vectors" => policy.block_vectors = number(flag, value)?,
            "--near-duplicate-cosine" => policy.near_duplicate_cosine = number(flag, value)?,
            "--thresholds" => {
                policy.thresholds = value
                    .split(',')
                    .map(|item| number(flag, item))
                    .collect::<Result<Vec<_>, _>>()?;
            }
            _ => return Err(GeometryError::Invalid(format!("unsupported option {flag}"))),
        }
        index += 2;
    }
    let capture = capture.ok_or_else(|| GeometryError::Invalid("--capture is required".into()))?;
    let metadata =
        metadata.ok_or_else(|| GeometryError::Invalid("--metadata is required".into()))?;
    let output = output.ok_or_else(|| GeometryError::Invalid("--output is required".into()))?;
    let (manifest, geometry, sampling, metrics) = profile_capture(&capture, &metadata, policy)?;
    write_profile_bundle(&output, &manifest, &geometry, &sampling, &metrics)?;
    println!("{}", output.display());
    Ok(())
}

fn verify(args: &[String]) -> Result<(), GeometryError> {
    if args.len() != 1 {
        return Err(GeometryError::Invalid(
            "verify requires exactly one profile bundle path".into(),
        ));
    }
    verify_bundle(&PathBuf::from(&args[0]))?;
    println!("verified {}", args[0]);
    Ok(())
}

fn number<T>(flag: &str, value: &str) -> Result<T, GeometryError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| GeometryError::Invalid(format!("invalid value for {flag}: {error}")))
}

fn usage() -> GeometryError {
    GeometryError::Invalid(help().into())
}

fn help() -> &'static str {
    "Usage:\n  qatq-kv-geometry profile --capture capture.kv --metadata capture.json --partition layer-head-token --normalization unit-l2 --seed 42 --max-pairs 1000000 --output results/run-name\n  qatq-kv-geometry verify results/run-name\n\nThe profiler emits bounded observations only. It does not derive capacity requirements or emit Capacity Oracle verdicts."
}
