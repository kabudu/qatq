use std::{
    env, fs,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use qatq::{
    decode, decode_phase2_lossless, for_each_phase2_lossless_container_payload, parse_mode,
    try_encode, try_encode_phase1_q4_with_config, try_encode_phase2_lossless_with_config,
    CodecMode, Phase1Config, MAX_VALUES_PER_PAYLOAD,
};

const QATC_MAGIC: &[u8; 4] = b"QATC";
const QATQ_VERSION: u8 = 1;
const PHASE2_LOSSLESS_MODE_ID: u8 = 4;

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
        Some("encode-chunked") => encode_chunked_command(&args[2..]),
        Some("decode") => decode_command(&args[2..]),
        Some("fixture") => fixture_command(&args[2..]),
        _ => {
            print_usage();
            Err("expected encode, encode-chunked, decode, or fixture command".to_string())
        }
    }
}

fn encode_command(args: &[String]) -> Result<(), String> {
    if args.len() != 4 && args.len() != 6 {
        print_usage();
        return Err(
            "usage: qatq encode --mode <mode> [--seed <u64>] <input.f32le> <output.qatq>"
                .to_string(),
        );
    }
    if args[0] != "--mode" {
        print_usage();
        return Err("encode requires --mode <mode>".to_string());
    }

    let mode = parse_mode(&args[1]).map_err(|error| error.to_string())?;
    let (seed, input_path, output_path) = if args.len() == 6 {
        if args[2] != "--seed" {
            print_usage();
            return Err("optional encode configuration must be --seed <u64>".to_string());
        }
        if mode != CodecMode::Phase1Q4 && mode != CodecMode::Phase2Lossless {
            return Err("--seed is only supported with phase1-q4 and phase2-lossless".to_string());
        }
        (Some(parse_seed(&args[3])?), &args[4], &args[5])
    } else {
        (None, &args[2], &args[3])
    };
    let values = read_f32le(input_path, Some(MAX_VALUES_PER_PAYLOAD))?;
    let payload = match (mode, seed) {
        (CodecMode::Phase1Q4, Some(seed)) => {
            try_encode_phase1_q4_with_config(&values, Phase1Config { seed })
                .map_err(|error| error.to_string())?
        }
        (CodecMode::Phase2Lossless, Some(seed)) => {
            try_encode_phase2_lossless_with_config(&values, Phase1Config { seed })
                .map_err(|error| error.to_string())?
        }
        _ => try_encode(&values, mode).map_err(|error| error.to_string())?,
    };
    write_bytes_atomic(output_path, &payload)
}

fn encode_chunked_command(args: &[String]) -> Result<(), String> {
    if args.len() != 4 && args.len() != 6 {
        print_usage();
        return Err(
            "usage: qatq encode-chunked --max-values-per-chunk <usize> [--seed <u64>] <input.f32le> <output.qatc>"
                .to_string(),
        );
    }
    if args[0] != "--max-values-per-chunk" {
        print_usage();
        return Err("encode-chunked requires --max-values-per-chunk <usize>".to_string());
    }

    let max_values_per_chunk = args[1]
        .parse::<usize>()
        .map_err(|error| format!("invalid --max-values-per-chunk {}: {error}", args[1]))?;
    let (seed, input_path, output_path) = if args.len() == 6 {
        if args[2] != "--seed" {
            print_usage();
            return Err("optional encode-chunked configuration must be --seed <u64>".to_string());
        }
        (parse_seed(&args[3])?, &args[4], &args[5])
    } else {
        (Phase1Config::default().seed, &args[2], &args[3])
    };

    encode_f32le_file_to_qatc_atomic(
        input_path,
        output_path,
        max_values_per_chunk,
        Phase1Config { seed },
    )
}

fn decode_command(args: &[String]) -> Result<(), String> {
    if args.len() != 2 {
        print_usage();
        return Err("usage: qatq decode <input.qatq> <output.f32le>".to_string());
    }
    let payload =
        fs::read(&args[0]).map_err(|error| format!("failed to read {}: {error}", args[0]))?;
    if payload.starts_with(b"QATC") {
        return decode_container_to_f32le(&payload, &args[1]);
    }
    let values = decode(&payload).map_err(|error| error.to_string())?;
    write_f32le_atomic(&args[1], &values)
}

fn decode_container_to_f32le(payload: &[u8], output_path: impl AsRef<Path>) -> Result<(), String> {
    let output_path = output_path.as_ref();
    write_atomic_with(output_path, |writer| {
        let mut write_error = None;
        let result = for_each_phase2_lossless_container_payload(payload, |chunk| {
            let values = decode_phase2_lossless(chunk)?;
            write_f32le_values(writer, &values).map_err(|error| {
                write_error = Some(format!(
                    "failed to write {}: {error}",
                    output_path.display()
                ));
                qatq::QatqError::InvalidContainer
            })?;
            Ok(())
        });
        match (result, write_error) {
            (Ok(()), _) => Ok(()),
            (Err(_), Some(error)) => Err(error),
            (Err(error), None) => Err(error.to_string()),
        }
    })
}

fn fixture_command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("add") => fixture_add_command(&args[1..]),
        Some("verify") => fixture_verify_command(&args[1..]),
        _ => {
            print_usage();
            Err("usage: qatq fixture <add|verify> ...".to_string())
        }
    }
}

fn fixture_add_command(args: &[String]) -> Result<(), String> {
    let mut manifest = None;
    let mut name = None;
    let mut path = None;
    let mut group = None;
    let mut shape = None;
    let mut notes = None;
    let mut index = 0;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("{} requires a value", args[index]))?;
        match args[index].as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value)),
            "--name" => name = Some(value.clone()),
            "--path" => path = Some(PathBuf::from(value)),
            "--group" => group = Some(value.clone()),
            "--shape" => shape = Some(value.clone()),
            "--notes" => notes = Some(value.clone()),
            other => return Err(format!("unknown fixture add option {other}")),
        }
        index += 2;
    }

    let manifest = manifest.ok_or_else(|| "--manifest is required".to_string())?;
    let name = required_manifest_value(name, "--name")?;
    let fixture_path = path.ok_or_else(|| "--path is required".to_string())?;
    let byte_len = validate_f32le_file(&fixture_path)?;
    let value_count = byte_len / 4;

    if let Some(parent) = manifest
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let block = render_fixture_manifest_block(FixtureManifestEntry {
        group: group.as_deref().unwrap_or("fixture"),
        name: &name,
        path: &fixture_path,
        shape: shape.as_deref(),
        notes: notes.as_deref(),
        value_count,
    });
    let mut text = match fs::read_to_string(&manifest) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(format!("failed to read {}: {error}", manifest.display())),
    };
    text.push_str(&block);
    write_bytes_atomic(&manifest, text.as_bytes())
}

fn fixture_verify_command(args: &[String]) -> Result<(), String> {
    let mut manifest = None;
    let mut output = None;
    let mut index = 0;
    while index < args.len() {
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("{} requires a value", args[index]))?;
        match args[index].as_str() {
            "--manifest" => manifest = Some(PathBuf::from(value)),
            "--output" => output = Some(PathBuf::from(value)),
            other => return Err(format!("unknown fixture verify option {other}")),
        }
        index += 2;
    }

    let manifest = manifest.ok_or_else(|| "--manifest is required".to_string())?;
    let fixtures = read_fixture_manifest(&manifest)?;
    if fixtures.is_empty() {
        return Err(format!("{} has no fixture entries", manifest.display()));
    }
    let mut rows = Vec::with_capacity(fixtures.len());
    for fixture in fixtures {
        let (bytes, fingerprint) = fingerprint_f32le_file(&fixture.path)?;
        rows.push(FixtureAuditRow {
            group: fixture.group,
            name: fixture.name,
            path: fixture.path,
            shape: fixture.shape,
            notes: fixture.notes,
            bytes,
            values: bytes / 4,
            fingerprint,
        });
    }
    let report = render_fixture_audit_report(&manifest, &rows);
    if let Some(path) = output {
        write_bytes_atomic(&path, report.as_bytes())
    } else {
        print!("{report}");
        Ok(())
    }
}

fn validate_f32le_file(path: &Path) -> Result<usize, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to stat {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }
    let byte_len = usize::try_from(metadata.len())
        .map_err(|_| format!("{} is too large for this platform", path.display()))?;
    if byte_len % 4 != 0 {
        return Err(format!(
            "{} is not a raw f32le file: byte length {} is not divisible by 4",
            path.display(),
            byte_len
        ));
    }
    Ok(byte_len)
}

fn fingerprint_f32le_file(path: &Path) -> Result<(usize, u64), String> {
    let byte_len = validate_f32le_file(path)?;
    let file = fs::File::open(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 64 * 1024];
    let mut fingerprint = Fnv1a64::new();
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        fingerprint.update(&buffer[..read]);
    }
    Ok((byte_len, fingerprint.finish()))
}

struct ManifestFixture {
    group: String,
    name: String,
    path: PathBuf,
    shape: Option<String>,
    notes: Option<String>,
}

#[derive(Default)]
struct PartialManifestFixture {
    group: Option<String>,
    name: Option<String>,
    path: Option<PathBuf>,
    shape: Option<String>,
    notes: Option<String>,
}

impl PartialManifestFixture {
    fn set(
        &mut self,
        key: &str,
        value: String,
        manifest: &Path,
        line: usize,
    ) -> Result<(), String> {
        match key {
            "group" => self.group = Some(value),
            "name" => self.name = Some(value),
            "path" => self.path = Some(PathBuf::from(value)),
            "shape" => self.shape = Some(value),
            "notes" => self.notes = Some(value),
            other => {
                return Err(format!(
                    "{}:{line}: unsupported fixture key {other}",
                    manifest.display()
                ));
            }
        }
        Ok(())
    }

    fn finish(
        self,
        base_dir: &Path,
        manifest: &Path,
        line: usize,
    ) -> Result<ManifestFixture, String> {
        let name = self.name.ok_or_else(|| {
            format!(
                "{}:{line}: fixture is missing required name",
                manifest.display()
            )
        })?;
        let path = self.path.ok_or_else(|| {
            format!(
                "{}:{line}: fixture is missing required path",
                manifest.display()
            )
        })?;
        let path = if path.is_absolute() {
            path
        } else {
            base_dir.join(path)
        };
        Ok(ManifestFixture {
            group: self.group.unwrap_or_else(|| "fixture".to_string()),
            name,
            path,
            shape: self.shape,
            notes: self.notes,
        })
    }
}

struct FixtureAuditRow {
    group: String,
    name: String,
    path: PathBuf,
    shape: Option<String>,
    notes: Option<String>,
    bytes: usize,
    values: usize,
    fingerprint: u64,
}

fn read_fixture_manifest(path: &Path) -> Result<Vec<ManifestFixture>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read manifest {}: {error}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut fixtures = Vec::new();
    let mut current = PartialManifestFixture::default();
    let mut in_fixture = false;

    for (line_index, raw_line) in text.lines().enumerate() {
        let line_number = line_index + 1;
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line == "[fixture]" {
            if in_fixture {
                fixtures.push(current.finish(base_dir, path, line_number)?);
                current = PartialManifestFixture::default();
            }
            in_fixture = true;
            continue;
        }
        if !in_fixture {
            return Err(format!(
                "{}:{line_number}: expected [fixture] before key/value entries",
                path.display()
            ));
        }
        let (key, value) = line.split_once('=').ok_or_else(|| {
            format!(
                "{}:{line_number}: expected key = value entry",
                path.display()
            )
        })?;
        current.set(
            key.trim(),
            parse_manifest_value(value.trim()),
            path,
            line_number,
        )?;
    }
    if in_fixture {
        fixtures.push(current.finish(base_dir, path, text.lines().count())?);
    }
    Ok(fixtures)
}

fn render_fixture_audit_report(manifest: &Path, rows: &[FixtureAuditRow]) -> String {
    let total_bytes: usize = rows.iter().map(|row| row.bytes).sum();
    let total_values: usize = rows.iter().map(|row| row.values).sum();
    let mut out = String::new();
    out.push_str("# Fixture Audit\n\n");
    out.push_str(&format!("- manifest: `{}`\n", manifest.display()));
    out.push_str(&format!("- fixtures: `{}`\n", rows.len()));
    out.push_str(&format!("- total values: `{total_values}`\n"));
    out.push_str(&format!("- total bytes: `{total_bytes}`\n\n"));
    out.push_str(
        "| group | name | values | bytes | fingerprint fnv1a64 | shape | notes | path |\n",
    );
    out.push_str("| --- | --- | ---: | ---: | --- | --- | --- | --- |\n");
    for row in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:016x} | {} | {} | {} |\n",
            row.group,
            row.name,
            row.values,
            row.bytes,
            row.fingerprint,
            row.shape.as_deref().unwrap_or(""),
            row.notes.as_deref().unwrap_or(""),
            row.path.display()
        ));
    }
    out
}

struct Fnv1a64 {
    hash: u64,
}

impl Fnv1a64 {
    fn new() -> Self {
        Self {
            hash: 0xcbf2_9ce4_8422_2325_u64,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.hash ^= *byte as u64;
            self.hash = self.hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn finish(self) -> u64 {
        self.hash
    }
}

fn parse_manifest_value(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|stripped| stripped.strip_suffix('"'))
        .unwrap_or(value)
        .trim()
        .to_string()
}

struct FixtureManifestEntry<'a> {
    group: &'a str,
    name: &'a str,
    path: &'a Path,
    shape: Option<&'a str>,
    notes: Option<&'a str>,
    value_count: usize,
}

fn render_fixture_manifest_block(entry: FixtureManifestEntry<'_>) -> String {
    let mut block = String::new();
    block.push_str("\n[fixture]\n");
    block.push_str(&format!(
        "group = \"{}\"\n",
        escape_manifest_value(entry.group)
    ));
    block.push_str(&format!(
        "name = \"{}\"\n",
        escape_manifest_value(entry.name)
    ));
    block.push_str(&format!(
        "path = \"{}\"\n",
        escape_manifest_value(&entry.path.display().to_string())
    ));
    if let Some(shape) = entry.shape {
        block.push_str(&format!("shape = \"{}\"\n", escape_manifest_value(shape)));
    }
    let notes = match entry.notes {
        Some(notes) if !notes.trim().is_empty() => {
            format!("{notes}; values={}", entry.value_count)
        }
        _ => format!("values={}", entry.value_count),
    };
    block.push_str(&format!("notes = \"{}\"\n", escape_manifest_value(&notes)));
    block
}

fn escape_manifest_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn required_manifest_value(value: Option<String>, name: &str) -> Result<String, String> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(format!("{name} is required")),
    }
}

fn read_f32le(path: impl AsRef<Path>, max_values: Option<usize>) -> Result<Vec<f32>, String> {
    let path = path.as_ref();
    let (byte_len, value_count) = validate_f32le_file_metadata(path)?;
    if let Some(max_values) = max_values {
        if value_count > max_values {
            return Err(format!(
                "{} contains {value_count} f32 values, exceeding the single-payload limit of {max_values}; use encode-chunked for large tensors",
                path.display()
            ));
        }
    }

    let file = fs::File::open(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0_u8; 64 * 1024];
    let mut pending = [0_u8; 4];
    let mut pending_len = 0_usize;
    let mut values = Vec::with_capacity(value_count);

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        let mut bytes = &buffer[..read];
        if pending_len > 0 {
            let needed = 4 - pending_len;
            if bytes.len() < needed {
                pending[pending_len..pending_len + bytes.len()].copy_from_slice(bytes);
                pending_len += bytes.len();
                continue;
            }
            pending[pending_len..4].copy_from_slice(&bytes[..needed]);
            values.push(f32::from_le_bytes(pending));
            bytes = &bytes[needed..];
        }
        let mut chunks = bytes.chunks_exact(4);
        for chunk in &mut chunks {
            values.push(f32::from_le_bytes(
                chunk.try_into().expect("chunk size checked"),
            ));
        }
        let remainder = chunks.remainder();
        pending[..remainder.len()].copy_from_slice(remainder);
        pending_len = remainder.len();
    }

    if pending_len != 0 {
        return Err(format!(
            "{} is not a raw f32le file: trailing partial f32",
            path.display()
        ));
    }
    debug_assert_eq!(values.len() * 4, byte_len);
    Ok(values)
}

fn validate_f32le_file_metadata(path: &Path) -> Result<(usize, usize), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to stat {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }
    let byte_len = usize::try_from(metadata.len())
        .map_err(|_| format!("{} is too large for this platform", path.display()))?;
    if byte_len % 4 != 0 {
        return Err(format!(
            "{} is not a raw f32le file: byte length {} is not divisible by 4",
            path.display(),
            byte_len
        ));
    }
    let value_count = byte_len / 4;
    Ok((byte_len, value_count))
}

fn encode_f32le_file_to_qatc_atomic(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    max_values_per_chunk: usize,
    config: Phase1Config,
) -> Result<(), String> {
    let input_path = input_path.as_ref();
    let output_path = output_path.as_ref();
    if max_values_per_chunk == 0 || max_values_per_chunk > MAX_VALUES_PER_PAYLOAD {
        return Err(format!("chunk size is invalid: {max_values_per_chunk}"));
    }
    let (_, value_count) = validate_f32le_file_metadata(input_path)?;
    let chunk_count = if value_count == 0 {
        1
    } else {
        value_count.div_ceil(max_values_per_chunk)
    };
    if chunk_count > u32::MAX as usize {
        return Err("chunked container is invalid".to_string());
    }

    let file = fs::File::open(input_path)
        .map_err(|error| format!("failed to read {}: {error}", input_path.display()))?;
    let mut reader = BufReader::new(file);
    let chunk_byte_len = max_values_per_chunk
        .checked_mul(4)
        .ok_or_else(|| format!("chunk size is invalid: {max_values_per_chunk}"))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(chunk_byte_len).map_err(|_| {
        format!(
            "failed to allocate chunk buffer for {}",
            input_path.display()
        )
    })?;
    bytes.resize(chunk_byte_len, 0);
    write_atomic_with(output_path, |writer| {
        write_qatc_header(writer, value_count, chunk_count)
            .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
        if value_count == 0 {
            let payload = try_encode_phase2_lossless_with_config(&[], config)
                .map_err(|error| error.to_string())?;
            write_qatc_chunk(writer, &payload, output_path)?;
            return Ok(());
        }

        let mut remaining_values = value_count;
        while remaining_values > 0 {
            let chunk_values = remaining_values.min(max_values_per_chunk);
            let chunk_bytes = chunk_values * 4;
            reader
                .read_exact(&mut bytes[..chunk_bytes])
                .map_err(|error| format!("failed to read {}: {error}", input_path.display()))?;
            let values = decode_f32le_chunk(&bytes[..chunk_bytes]);
            let payload = try_encode_phase2_lossless_with_config(&values, config)
                .map_err(|error| error.to_string())?;
            write_qatc_chunk(writer, &payload, output_path)?;
            remaining_values -= chunk_values;
        }
        Ok(())
    })
}

fn decode_f32le_chunk(bytes: &[u8]) -> Vec<f32> {
    debug_assert_eq!(bytes.len() % 4, 0);
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("chunk size checked")))
        .collect()
}

fn write_qatc_header(
    writer: &mut impl Write,
    total_values: usize,
    chunk_count: usize,
) -> std::io::Result<()> {
    writer.write_all(QATC_MAGIC)?;
    writer.write_all(&[QATQ_VERSION, PHASE2_LOSSLESS_MODE_ID, 0, 0])?;
    writer.write_all(&(total_values as u64).to_be_bytes())?;
    writer.write_all(&(chunk_count as u32).to_be_bytes())?;
    writer.write_all(&[0, 0, 0, 0])?;
    Ok(())
}

fn write_qatc_chunk(
    writer: &mut impl Write,
    payload: &[u8],
    output_path: &Path,
) -> Result<(), String> {
    if payload.len() > u32::MAX as usize {
        return Err("chunked container is invalid".to_string());
    }
    writer
        .write_all(&(payload.len() as u32).to_be_bytes())
        .and_then(|_| writer.write_all(payload))
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))
}

fn write_f32le_atomic(path: impl AsRef<Path>, values: &[f32]) -> Result<(), String> {
    let path = path.as_ref();
    write_atomic_with(path, |writer| {
        write_f32le_values(writer, values)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))
    })
}

fn write_bytes_atomic(path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), String> {
    let path = path.as_ref();
    write_atomic_with(path, |writer| {
        writer
            .write_all(bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))
    })
}

fn write_atomic_with(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<fs::File>) -> Result<(), String>,
) -> Result<(), String> {
    let temp_path = temp_output_path(path);
    let result = (|| {
        let file = fs::File::create(&temp_path)
            .map_err(|error| format!("failed to create {}: {error}", temp_path.display()))?;
        let mut writer = BufWriter::new(file);
        write(&mut writer)?;
        writer
            .flush()
            .map_err(|error| format!("failed to flush {}: {error}", temp_path.display()))?;
        fs::rename(&temp_path, path).map_err(|error| {
            format!(
                "failed to move {} to {}: {error}",
                temp_path.display(),
                path.display()
            )
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn write_f32le_values(writer: &mut impl Write, values: &[f32]) -> std::io::Result<()> {
    const BUFFER_VALUES: usize = 1024;
    let mut bytes = Vec::with_capacity(BUFFER_VALUES * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
        if bytes.len() == BUFFER_VALUES * 4 {
            writer.write_all(&bytes)?;
            bytes.clear();
        }
    }
    if !bytes.is_empty() {
        writer.write_all(&bytes)?;
    }
    Ok(())
}

fn temp_output_path(path: &Path) -> PathBuf {
    let mut temp = path.as_os_str().to_owned();
    temp.push(format!(".tmp.{}", std::process::id()));
    PathBuf::from(temp)
}

fn parse_seed(value: &str) -> Result<u64, String> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).map_err(|error| format!("invalid hex seed {value}: {error}"))
    } else {
        value
            .parse::<u64>()
            .map_err(|error| format!("invalid seed {value}: {error}"))
    }
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!(
        "  qatq encode --mode <lossy-i4|lossless-f32|phase1-q4|phase2-lossless> [--seed <u64>] <input.f32le> <output.qatq>"
    );
    eprintln!(
        "  qatq encode-chunked --max-values-per-chunk <usize> [--seed <u64>] <input.f32le> <output.qatc>"
    );
    eprintln!("  qatq decode <input.qatq> <output.f32le>");
    eprintln!("  qatq fixture add --manifest <path> --name <name> --path <tensor.f32le> [--group <group>] [--shape <shape>] [--notes <notes>]");
    eprintln!("  qatq fixture verify --manifest <path> [--output <audit.md>]");
}
