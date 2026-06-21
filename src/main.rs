use std::{env, fs, path::Path};

use qatq::{decode, encode, parse_mode};

fn main() {
    if let Err(error) = run() {
        eprintln!("qatq: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("encode") => encode_command(&args[2..]),
        Some("decode") => decode_command(&args[2..]),
        _ => {
            print_usage();
            Err("expected encode or decode command".to_string())
        }
    }
}

fn encode_command(args: &[String]) -> Result<(), String> {
    if args.len() != 5 || args[0] != "--mode" {
        print_usage();
        return Err(
            "usage: qatq encode --mode <lossy-i4|lossless-f32> <input.f32le> <output.qatq>"
                .to_string(),
        );
    }
    let mode = parse_mode(&args[1]).map_err(|error| error.to_string())?;
    let values = read_f32le(&args[2])?;
    let payload = encode(&values, mode);
    fs::write(&args[3], payload).map_err(|error| format!("failed to write {}: {error}", args[3]))
}

fn decode_command(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        print_usage();
        return Err("usage: qatq decode <input.qatq> <output.f32le>".to_string());
    }
    let payload =
        fs::read(&args[0]).map_err(|error| format!("failed to read {}: {error}", args[0]))?;
    let values = decode(&payload).map_err(|error| error.to_string())?;
    write_f32le(&args[1], &values)
}

fn read_f32le(path: impl AsRef<Path>) -> Result<Vec<f32>, String> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if bytes.len() % 4 != 0 {
        return Err(format!(
            "{} is not a raw f32le file: byte length {} is not divisible by 4",
            path.display(),
            bytes.len()
        ));
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("chunk size checked")))
        .collect())
}

fn write_f32le(path: impl AsRef<Path>, values: &[f32]) -> Result<(), String> {
    let path = path.as_ref();
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  qatq encode --mode <lossy-i4|lossless-f32> <input.f32le> <output.qatq>");
    eprintln!("  qatq decode <input.qatq> <output.f32le>");
}
