use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use qatq::{
    TensorDType, decode_qatq_exact_tensor_le, qatq_exact_strategy, try_encode_qatq_exact_tensor_le,
};

const DEFAULT_ITERS: usize = 5;

fn main() {
    if let Err(error) = run() {
        eprintln!("qatq-kv-bench: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let mut inputs = Vec::new();
    let mut output = None;
    let mut iters = DEFAULT_ITERS;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--input" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--input requires label:dtype:path".to_string())?;
                inputs.push(parse_input(value)?);
                index += 2;
            }
            "--dir" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--dir requires a path".to_string())?;
                inputs.extend(scan_dir(Path::new(value))?);
                index += 2;
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.get(index + 1)
                        .ok_or_else(|| "--output requires a path".to_string())?,
                ));
                index += 2;
            }
            "--iters" => {
                iters = args
                    .get(index + 1)
                    .ok_or_else(|| "--iters requires a value".to_string())?
                    .parse::<usize>()
                    .map_err(|error| format!("invalid --iters: {error}"))?;
                if iters == 0 {
                    return Err("--iters must be greater than zero".to_string());
                }
                index += 2;
            }
            "-h" | "--help" => {
                print_usage();
                return Ok(());
            }
            other => return Err(format!("unknown option {other}")),
        }
    }
    if inputs.is_empty() {
        print_usage();
        return Err("at least one --input or --dir is required".to_string());
    }

    let mut rows = Vec::with_capacity(inputs.len());
    for input in &inputs {
        rows.push(bench_input(input, iters)?);
    }
    let report = render_report(&rows, iters);
    if let Some(output) = output {
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::write(&output, report.as_bytes())
            .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    } else {
        print!("{report}");
    }
    Ok(())
}

#[derive(Clone)]
struct Input {
    label: String,
    dtype: TensorDType,
    path: PathBuf,
}

struct BenchRow {
    label: String,
    dtype: TensorDType,
    path: PathBuf,
    values: usize,
    raw_bytes: usize,
    qatq_bytes: usize,
    qatq_strategy: String,
    zstd_bytes: usize,
    lz4_bytes: usize,
    qatq_encode_ns_per_value: f64,
    qatq_decode_ns_per_value: f64,
    zstd_encode_ns_per_value: f64,
    zstd_decode_ns_per_value: f64,
    lz4_encode_ns_per_value: f64,
    lz4_decode_ns_per_value: f64,
}

fn bench_input(input: &Input, iters: usize) -> Result<BenchRow, String> {
    let bytes = fs::read(&input.path)
        .map_err(|error| format!("failed to read {}: {error}", input.path.display()))?;
    let width = input.dtype.element_width();
    if bytes.len() % width != 0 {
        return Err(format!(
            "{} byte length {} is not divisible by {} for {}",
            input.path.display(),
            bytes.len(),
            width,
            input.dtype.as_str()
        ));
    }
    let values = bytes.len() / width;
    let qatq = try_encode_qatq_exact_tensor_le(&bytes, input.dtype)
        .map_err(|error| format!("QATQ encode failed for {}: {error}", input.label))?;
    let decoded = decode_qatq_exact_tensor_le(&qatq)
        .map_err(|error| format!("QATQ decode failed for {}: {error}", input.label))?;
    if decoded.dtype != input.dtype || decoded.bytes_le != bytes {
        return Err(format!("QATQ exact round trip failed for {}", input.label));
    }
    let zstd = zstd::bulk::compress(&bytes, 3)
        .map_err(|error| format!("zstd encode failed for {}: {error}", input.label))?;
    let zstd_decoded = zstd::bulk::decompress(&zstd, bytes.len())
        .map_err(|error| format!("zstd decode failed for {}: {error}", input.label))?;
    if zstd_decoded != bytes {
        return Err(format!("zstd exact round trip failed for {}", input.label));
    }
    let lz4 = lz4_flex::compress_prepend_size(&bytes);
    let lz4_decoded = lz4_flex::decompress_size_prepended(&lz4)
        .map_err(|error| format!("lz4 decode failed for {}: {error}", input.label))?;
    if lz4_decoded != bytes {
        return Err(format!("lz4 exact round trip failed for {}", input.label));
    }

    let qatq_encode = time_repeated(iters, || {
        try_encode_qatq_exact_tensor_le(&bytes, input.dtype).expect("QATQ encode")
    });
    let qatq_decode = time_repeated(iters, || {
        decode_qatq_exact_tensor_le(&qatq).expect("QATQ decode")
    });
    let zstd_encode = time_repeated(iters, || {
        zstd::bulk::compress(&bytes, 3).expect("zstd encode")
    });
    let zstd_decode = time_repeated(iters, || {
        zstd::bulk::decompress(&zstd, bytes.len()).expect("zstd decode")
    });
    let lz4_encode = time_repeated(iters, || lz4_flex::compress_prepend_size(&bytes));
    let lz4_decode = time_repeated(iters, || {
        lz4_flex::decompress_size_prepended(&lz4).expect("lz4 decode")
    });

    Ok(BenchRow {
        label: input.label.clone(),
        dtype: input.dtype,
        path: input.path.clone(),
        values,
        raw_bytes: bytes.len(),
        qatq_bytes: qatq.len(),
        qatq_strategy: qatq_exact_strategy(&qatq)
            .map(|strategy| strategy.as_str().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        zstd_bytes: zstd.len(),
        lz4_bytes: lz4.len(),
        qatq_encode_ns_per_value: ns_per_value(qatq_encode, iters, values),
        qatq_decode_ns_per_value: ns_per_value(qatq_decode, iters, values),
        zstd_encode_ns_per_value: ns_per_value(zstd_encode, iters, values),
        zstd_decode_ns_per_value: ns_per_value(zstd_decode, iters, values),
        lz4_encode_ns_per_value: ns_per_value(lz4_encode, iters, values),
        lz4_decode_ns_per_value: ns_per_value(lz4_decode, iters, values),
    })
}

fn time_repeated<T>(iters: usize, mut f: impl FnMut() -> T) -> Duration {
    let start = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(f());
    }
    start.elapsed()
}

fn ns_per_value(duration: Duration, iters: usize, values: usize) -> f64 {
    if values == 0 {
        return 0.0;
    }
    duration.as_nanos() as f64 / (iters as f64 * values as f64)
}

fn parse_input(value: &str) -> Result<Input, String> {
    let mut parts = value.splitn(3, ':');
    let label = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| "input label is required".to_string())?;
    let dtype = parts
        .next()
        .ok_or_else(|| "input dtype is required".to_string())
        .and_then(parse_dtype)?;
    let path = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| "input path is required".to_string())?;
    Ok(Input {
        label: label.to_string(),
        dtype,
        path: PathBuf::from(path),
    })
}

fn scan_dir(path: &Path) -> Result<Vec<Input>, String> {
    let entries = fs::read_dir(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut inputs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read dir entry: {error}"))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(dtype) = infer_dtype(&path) else {
            continue;
        };
        let label = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("tensor")
            .to_string();
        inputs.push(Input { label, dtype, path });
    }
    inputs.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(inputs)
}

fn infer_dtype(path: &Path) -> Option<TensorDType> {
    let name = path.file_name()?.to_str()?;
    if name.ends_with(".f32le") {
        Some(TensorDType::F32)
    } else if name.ends_with(".f16le") {
        Some(TensorDType::F16)
    } else if name.ends_with(".bf16le") {
        Some(TensorDType::BF16)
    } else {
        None
    }
}

fn parse_dtype(value: &str) -> Result<TensorDType, String> {
    match value {
        "f32" => Ok(TensorDType::F32),
        "f16" => Ok(TensorDType::F16),
        "bf16" => Ok(TensorDType::BF16),
        other => Err(format!("unsupported dtype {other}")),
    }
}

fn render_report(rows: &[BenchRow], iters: usize) -> String {
    let mut out = String::new();
    out.push_str("# llama.cpp KV Tensor Compression Report\n\n");
    out.push_str("Generated by `qatq-kv-bench` over raw typed tensor files exported by a runtime adapter.\n\n");
    out.push_str(&format!("- timing iterations: `{iters}`\n"));
    out.push_str(&format!("- tensors: `{}`\n\n", rows.len()));
    out.push_str("| tensor | dtype | values | raw bytes | QATQ bytes | QATQ ratio | QATQ strategy | zstd bytes | zstd ratio | lz4 bytes | lz4 ratio | QATQ enc ns/value | QATQ dec ns/value | zstd enc ns/value | zstd dec ns/value | lz4 enc ns/value | lz4 dec ns/value | path |\n");
    out.push_str("| --- | --- | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
    for row in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {:.4} | {} | {} | {:.4} | {} | {:.4} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} |\n",
            row.label,
            row.dtype.as_str(),
            row.values,
            row.raw_bytes,
            row.qatq_bytes,
            ratio(row.qatq_bytes, row.raw_bytes),
            row.qatq_strategy,
            row.zstd_bytes,
            ratio(row.zstd_bytes, row.raw_bytes),
            row.lz4_bytes,
            ratio(row.lz4_bytes, row.raw_bytes),
            row.qatq_encode_ns_per_value,
            row.qatq_decode_ns_per_value,
            row.zstd_encode_ns_per_value,
            row.zstd_decode_ns_per_value,
            row.lz4_encode_ns_per_value,
            row.lz4_decode_ns_per_value,
            row.path.display()
        ));
    }
    out.push_str("\n## Claim Boundary\n\n");
    out.push_str("- Supported: every row is exact; QATQ, zstd, and lz4 decode bytes are checked against the original raw typed tensor bytes.\n");
    out.push_str("- Supported: QATQ rows use native dtype-aware exact tensor encoding, not f32 widening for f16/bf16 inputs.\n");
    out.push_str("- Required input: direct raw KV tensors from a llama.cpp adapter, named with `.f16le`, `.bf16le`, or `.f32le` suffixes.\n");
    out
}

fn ratio(size: usize, raw: usize) -> f64 {
    if raw == 0 {
        0.0
    } else {
        size as f64 / raw as f64
    }
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  qatq-kv-bench --dir <kv-export-dir> [--iters N] [--output report.md]");
    eprintln!("  qatq-kv-bench --input <label>:<f32|f16|bf16>:<path> [...] [--output report.md]");
}
