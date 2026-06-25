#![no_main]

use std::{collections::BTreeMap, fmt::Write as _};

use libfuzzer_sys::fuzz_target;
use qatq::{LiveVramGpuAllocationGranularity, parse_llama_cpp_kv_manifest};

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_llama_cpp_kv_manifest(text);
    }

    if data.is_empty() {
        return;
    }
    let mut cursor = Cursor::new(data);
    let manifest = synthetic_manifest(&mut cursor);
    if let Ok(parsed) = parse_llama_cpp_kv_manifest(&manifest) {
        assert_eq!(parsed.format, "qatq-llama-cpp-kv-v1");
        assert!(parsed.seq_id >= 0);
        assert!(parsed.kv_size > 0);
        assert!(parsed.streams > 0);
        assert!(!parsed.tensors.is_empty());
        if let Some(total_tensors) = parsed.total_tensors {
            assert_eq!(total_tensors, parsed.tensors.len());
        }
        if let (Some(gpu_context_bytes), Some(total_context_bytes)) =
            (parsed.gpu_context_bytes, parsed.total_context_bytes)
        {
            assert!(
                gpu_context_bytes <= total_context_bytes
                    || matches!(
                        parsed.gpu_allocation_granularity,
                        Some(LiveVramGpuAllocationGranularity::PerPage)
                    )
                    || parsed.gpu_page_staging_bytes.is_some()
            );
        }
        if let (Some(gpu_resident_tensors), Some(total_tensors)) =
            (parsed.gpu_resident_tensors, parsed.total_tensors)
        {
            assert!(gpu_resident_tensors <= total_tensors);
        }
        if let Some(granularity) = parsed.gpu_allocation_granularity {
            assert!(matches!(
                granularity,
                LiveVramGpuAllocationGranularity::PerPage
                    | LiveVramGpuAllocationGranularity::WholeTensor
                    | LiveVramGpuAllocationGranularity::WholeContext
                    | LiveVramGpuAllocationGranularity::RuntimeUnknown
            ));
        }
        let mut ranges = BTreeMap::<(String, qatq::KvPageKind, u32), Vec<(u64, u64)>>::new();
        for tensor in &parsed.tensors {
            assert!(!tensor.file.is_empty());
            assert!(!tensor.file.contains('/'));
            assert!(!tensor.file.contains('\\'));
            assert!(!tensor.file.contains(".."));
            assert!((tensor.stream as usize) < parsed.streams);
            let entry = ranges
                .entry((tensor.name.clone(), tensor.kind, tensor.stream))
                .or_default();
            assert!(entry.iter().all(|(start, end)| {
                tensor.token_start >= *end || tensor.token_end <= *start
            }));
            entry.push((tensor.token_start, tensor.token_end));
            assert_eq!(tensor.row_bytes, tensor.embedding * tensor.dtype.element_width());
        }
    }
});

struct Cursor<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, index: 0 }
    }

    fn next(&mut self) -> u8 {
        let value = self.data[self.index % self.data.len()];
        self.index = self.index.wrapping_add(1);
        value
    }
}

fn synthetic_manifest(cursor: &mut Cursor<'_>) -> String {
    let seq_id = if cursor.next() & 1 == 0 {
        i64::from(cursor.next())
    } else {
        -i64::from(cursor.next())
    };
    let kv_size = cursor.next() as usize * 16;
    let streams = cursor.next() as usize % 4;
    let tensor_count = cursor.next() as usize % 10;
    let total_tensors = if cursor.next() & 1 == 0 {
        tensor_count
    } else {
        tensor_count.saturating_add(1)
    };
    let gpu_context_bytes = cursor.next() as usize * 1024;
    let total_context_bytes = if cursor.next() & 1 == 0 {
        gpu_context_bytes.saturating_add(cursor.next() as usize * 1024)
    } else {
        gpu_context_bytes.saturating_sub(cursor.next() as usize)
    };
    let gpu_resident_tensors = cursor.next() as usize % 12;
    let granularity = match cursor.next() % 5 {
        0 => "per-page",
        1 => "whole-tensor",
        2 => "whole-context",
        3 => "runtime-unknown",
        _ => "bad-granularity",
    };

    let mut manifest = String::new();
    let _ = writeln!(&mut manifest, "{{");
    let _ = writeln!(&mut manifest, "  \"format\": \"qatq-llama-cpp-kv-v1\",");
    let _ = writeln!(&mut manifest, "  \"seq_id\": {seq_id},");
    let _ = writeln!(&mut manifest, "  \"kv_size\": {kv_size},");
    let _ = writeln!(&mut manifest, "  \"streams\": {streams},");
    let _ = writeln!(
        &mut manifest,
        "  \"gpu_allocation_granularity\": \"{granularity}\","
    );
    let _ = writeln!(
        &mut manifest,
        "  \"gpu_context_bytes\": {gpu_context_bytes},"
    );
    let _ = writeln!(
        &mut manifest,
        "  \"total_context_bytes\": {total_context_bytes},"
    );
    let _ = writeln!(
        &mut manifest,
        "  \"gpu_resident_tensors\": {gpu_resident_tensors},"
    );
    let _ = writeln!(&mut manifest, "  \"total_tensors\": {total_tensors},");
    let _ = writeln!(&mut manifest, "  \"tensors\": [");
    for index in 0..tensor_count {
        let dtype = match cursor.next() % 4 {
            0 => ("f16le", 2),
            1 => ("bf16le", 2),
            2 => ("f32le", 4),
            _ => ("bad", 2),
        };
        let embedding = 1 + (cursor.next() as usize % 64);
        let row_bytes = if cursor.next() & 1 == 0 {
            embedding * dtype.1
        } else {
            embedding
        };
        let active_cells = cursor.next() as usize % 64;
        let kind = if cursor.next() & 1 == 0 { "k" } else { "v" };
        let name_index = if cursor.next() & 1 == 0 { index } else { 0 };
        let stream = if cursor.next() & 1 == 0 {
            0
        } else {
            cursor.next() as usize % 6
        };
        let token_start = match cursor.next() % 4 {
            0 => 0,
            1 => index * 16,
            2 => name_index * 4,
            _ => cursor.next() as usize % 96,
        };
        let token_end = if cursor.next() & 1 == 0 {
            token_start.saturating_add(active_cells)
        } else {
            token_start.saturating_add(active_cells.saturating_add(cursor.next() as usize % 4))
        };
        let file = match cursor.next() % 5 {
            0 => format!("cache_{kind}_l{name_index}_s{stream}_t{token_start}_{token_end}.le"),
            1 => format!("../cache_{kind}_l{name_index}_s{stream}.f16le"),
            2 => format!("cache/{kind}_l{name_index}_s{stream}.f16le"),
            3 => format!("cache_{kind}_l{name_index}_s{stream}_t{token_start}_{token_end}.f16le"),
            _ => format!("cache_{kind}_l{}_s{stream}.f16le", index),
        };
        let _ = write!(
            &mut manifest,
            "    {{\"name\":\"cache_{kind}_l{name_index}\",\"kind\":\"{kind}\",\"stream\":{stream},\"file\":\"{file}\",\"dtype\":\"{}\",\"token_start\":{token_start},\"token_end\":{token_end},\"active_cells\":{active_cells},\"embedding\":{embedding},\"row_bytes\":{row_bytes},\"transposed\":{}}}",
            dtype.0,
            if cursor.next() & 1 == 0 {
                "true"
            } else {
                "false"
            }
        );
        if index + 1 != tensor_count {
            manifest.push(',');
        }
        manifest.push('\n');
    }
    let _ = writeln!(&mut manifest, "  ]");
    let _ = writeln!(&mut manifest, "}}");
    manifest
}
