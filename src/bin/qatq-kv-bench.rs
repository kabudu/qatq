use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use qatq::{
    KvPageKey, KvPageKind, LiveVramEventTracePolicy, LiveVramEventTraceReport,
    LiveVramGpuAllocationGranularity, LiveVramLimits, LiveVramPageEvent, LiveVramPageEventKind,
    LiveVramPrefetchBudget, LiveVramProofGate, LiveVramSchedulerPolicy, LiveVramSchedulerState,
    LlamaCppKvExportReplayConfig, TensorDType, build_live_vram_evidence_report,
    build_live_vram_evidence_report_with_page_seals,
    compare_live_vram_segment_summary_attention_reference,
    compare_live_vram_typed_streaming_attention_reference, decode_qatq_exact_tensor_le,
    decode_tensor_le_bytes_to_f32, estimate_live_vram_residency_after_offload,
    estimate_live_vram_residency_from_runtime_allocation, evaluate_live_vram_event_trace,
    evaluate_live_vram_live_paging_proof_gate, evaluate_live_vram_prefetch_deadlines,
    evaluate_live_vram_proof_gate, live_vram_snapshots_from_llama_cpp_export_dir,
    parse_llama_cpp_kv_manifest, qatq_exact_strategy, try_encode_qatq_exact_tensor_le,
};

const DEFAULT_ITERS: usize = 5;
const MAX_OUTPUT_MANIFEST_BYTES: u64 = 1 << 20;
const MAX_LIVE_VRAM_EVENT_TRACE_BYTES: u64 = 16 << 20;
const MAX_ATTENTION_TENSOR_BYTES: u64 = 64 << 20;

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
    let mut live_vram_export_dir = None;
    let mut live_vram_runtime_commit = None;
    let mut live_vram_adapter_version = None;
    let mut live_vram_model_id = None;
    let mut live_vram_current_token = 0_u64;
    let mut live_vram_hot_window_tokens = LiveVramSchedulerPolicy::default().hot_window_tokens;
    let mut live_vram_prefetch_window_tokens =
        LiveVramSchedulerPolicy::default().prefetch_window_tokens;
    let mut live_vram_next_required = LiveVramNextRequiredMode::UniformAfterHot;
    let mut live_vram_max_queued_pages = LiveVramSchedulerPolicy::default().max_queued_pages;
    let mut live_vram_max_cpu_stored_bytes =
        LiveVramSchedulerPolicy::default().max_cpu_stored_bytes;
    let mut live_vram_max_tensors = 1 << 16;
    let mut live_vram_gpu_context_bytes = None;
    let mut live_vram_allocation_granularity = LiveVramGpuAllocationGranularity::RuntimeUnknown;
    let mut live_vram_allocation_granularity_set = false;
    let mut live_vram_restore_bytes_per_token = None;
    let mut live_vram_proof_gate = false;
    let mut live_vram_runtime_reclaim_gate = false;
    let mut live_vram_live_paging_gate = false;
    let mut live_vram_event_trace = None;
    let mut live_vram_event_trace_only = false;
    let mut live_vram_event_trace_gate = false;
    let mut live_vram_min_gpu_saved_ratio = LiveVramProofGate::default().min_gpu_saved_ratio;
    let mut live_vram_aggregate_codec_gate = false;
    let mut live_vram_page_seal_key = None;
    let mut live_vram_require_page_seals = false;
    let mut compare_output_baseline = None;
    let mut compare_output_candidate = None;
    let mut compare_output_gate = false;
    let mut attention_query = None;
    let mut attention_key_pages = Vec::new();
    let mut attention_value_pages = Vec::new();
    let mut attention_head_dim = None;
    let mut attention_value_dim = None;
    let mut attention_tolerance = 1.0e-5_f32;
    let mut attention_max_peak_page_kv_ratio = None;
    let mut attention_equivalence_gate = false;
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
            "--live-vram-export-dir" => {
                live_vram_export_dir =
                    Some(PathBuf::from(args.get(index + 1).ok_or_else(|| {
                        "--live-vram-export-dir requires a path".to_string()
                    })?));
                index += 2;
            }
            "--live-vram-runtime-commit" => {
                live_vram_runtime_commit =
                    Some(required_arg(&args, index, "--live-vram-runtime-commit")?);
                index += 2;
            }
            "--live-vram-adapter-version" => {
                live_vram_adapter_version =
                    Some(required_arg(&args, index, "--live-vram-adapter-version")?);
                index += 2;
            }
            "--live-vram-model-id" => {
                live_vram_model_id = Some(required_arg(&args, index, "--live-vram-model-id")?);
                index += 2;
            }
            "--live-vram-current-token" => {
                live_vram_current_token = parse_u64_arg(&args, index, "--live-vram-current-token")?;
                index += 2;
            }
            "--live-vram-hot-window-tokens" => {
                live_vram_hot_window_tokens =
                    parse_u64_arg(&args, index, "--live-vram-hot-window-tokens")?;
                index += 2;
            }
            "--live-vram-prefetch-window-tokens" => {
                live_vram_prefetch_window_tokens =
                    parse_u64_arg(&args, index, "--live-vram-prefetch-window-tokens")?;
                index += 2;
            }
            "--live-vram-next-required" => {
                live_vram_next_required = parse_live_vram_next_required_mode(&required_arg(
                    &args,
                    index,
                    "--live-vram-next-required",
                )?)?;
                index += 2;
            }
            "--live-vram-max-cpu-stored-bytes" => {
                live_vram_max_cpu_stored_bytes =
                    parse_usize_arg(&args, index, "--live-vram-max-cpu-stored-bytes")?;
                index += 2;
            }
            "--live-vram-max-queued-pages" => {
                live_vram_max_queued_pages =
                    parse_usize_arg(&args, index, "--live-vram-max-queued-pages")?;
                index += 2;
            }
            "--live-vram-max-tensors" => {
                live_vram_max_tensors = parse_usize_arg(&args, index, "--live-vram-max-tensors")?;
                index += 2;
            }
            "--live-vram-gpu-context-bytes" => {
                live_vram_gpu_context_bytes = Some(parse_usize_arg(
                    &args,
                    index,
                    "--live-vram-gpu-context-bytes",
                )?);
                index += 2;
            }
            "--live-vram-allocation-granularity" => {
                live_vram_allocation_granularity = parse_live_vram_allocation_granularity(
                    &required_arg(&args, index, "--live-vram-allocation-granularity")?,
                )?;
                live_vram_allocation_granularity_set = true;
                index += 2;
            }
            "--live-vram-restore-bytes-per-token" => {
                live_vram_restore_bytes_per_token = Some(parse_usize_arg(
                    &args,
                    index,
                    "--live-vram-restore-bytes-per-token",
                )?);
                index += 2;
            }
            "--live-vram-proof-gate" => {
                live_vram_proof_gate = true;
                index += 1;
            }
            "--live-vram-runtime-reclaim-gate" => {
                live_vram_runtime_reclaim_gate = true;
                index += 1;
            }
            "--live-vram-live-paging-gate" => {
                live_vram_live_paging_gate = true;
                index += 1;
            }
            "--live-vram-event-trace" => {
                live_vram_event_trace = Some(PathBuf::from(required_arg(
                    &args,
                    index,
                    "--live-vram-event-trace",
                )?));
                index += 2;
            }
            "--live-vram-event-trace-only" => {
                live_vram_event_trace_only = true;
                index += 1;
            }
            "--live-vram-event-trace-gate" => {
                live_vram_event_trace_gate = true;
                index += 1;
            }
            "--live-vram-min-gpu-saved-ratio" => {
                live_vram_min_gpu_saved_ratio =
                    parse_ratio_arg(&args, index, "--live-vram-min-gpu-saved-ratio")?;
                index += 2;
            }
            "--live-vram-aggregate-codec-gate" => {
                live_vram_aggregate_codec_gate = true;
                index += 1;
            }
            "--live-vram-require-page-seals" => {
                live_vram_require_page_seals = true;
                index += 1;
            }
            "--live-vram-page-seal-key-hex" => {
                live_vram_page_seal_key = Some(parse_hex_32_arg(
                    &args,
                    index,
                    "--live-vram-page-seal-key-hex",
                )?);
                index += 2;
            }
            "--compare-output-baseline" => {
                compare_output_baseline = Some(PathBuf::from(required_arg(
                    &args,
                    index,
                    "--compare-output-baseline",
                )?));
                index += 2;
            }
            "--compare-output-candidate" => {
                compare_output_candidate = Some(PathBuf::from(required_arg(
                    &args,
                    index,
                    "--compare-output-candidate",
                )?));
                index += 2;
            }
            "--compare-output-gate" => {
                compare_output_gate = true;
                index += 1;
            }
            "--attention-query" => {
                attention_query = Some(parse_attention_tensor_arg(&required_arg(
                    &args,
                    index,
                    "--attention-query",
                )?)?);
                index += 2;
            }
            "--attention-key-page" => {
                attention_key_pages.push(parse_attention_tensor_arg(&required_arg(
                    &args,
                    index,
                    "--attention-key-page",
                )?)?);
                index += 2;
            }
            "--attention-value-page" => {
                attention_value_pages.push(parse_attention_tensor_arg(&required_arg(
                    &args,
                    index,
                    "--attention-value-page",
                )?)?);
                index += 2;
            }
            "--attention-head-dim" => {
                attention_head_dim = Some(parse_usize_arg(&args, index, "--attention-head-dim")?);
                index += 2;
            }
            "--attention-value-dim" => {
                attention_value_dim = Some(parse_usize_arg(&args, index, "--attention-value-dim")?);
                index += 2;
            }
            "--attention-tolerance" => {
                attention_tolerance = parse_f32_arg(&args, index, "--attention-tolerance")?;
                if !attention_tolerance.is_finite() || attention_tolerance < 0.0 {
                    return Err("--attention-tolerance must be finite and non-negative".to_string());
                }
                index += 2;
            }
            "--attention-max-peak-page-kv-ratio" => {
                let ratio = parse_f32_arg(&args, index, "--attention-max-peak-page-kv-ratio")?;
                if !ratio.is_finite() || ratio <= 0.0 || ratio > 1.0 {
                    return Err(
                        "--attention-max-peak-page-kv-ratio must be finite in (0, 1]".to_string(),
                    );
                }
                attention_max_peak_page_kv_ratio = Some(ratio);
                index += 2;
            }
            "--attention-equivalence-gate" => {
                attention_equivalence_gate = true;
                index += 1;
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
    let attention_mode = attention_query.is_some()
        || !attention_key_pages.is_empty()
        || !attention_value_pages.is_empty()
        || attention_head_dim.is_some()
        || attention_value_dim.is_some()
        || attention_equivalence_gate;
    if attention_mode {
        if !inputs.is_empty()
            || live_vram_export_dir.is_some()
            || compare_output_baseline.is_some()
            || compare_output_candidate.is_some()
            || compare_output_gate
        {
            return Err(
                "--attention-* options cannot be combined with --input, --dir, --live-vram-export-dir, or --compare-output-*"
                    .to_string(),
            );
        }
        let report = run_attention_equivalence_report(AttentionCliOptions {
            query: attention_query
                .ok_or_else(|| "--attention-query is required in attention mode".to_string())?,
            key_pages: attention_key_pages,
            value_pages: attention_value_pages,
            head_dim: attention_head_dim
                .ok_or_else(|| "--attention-head-dim is required in attention mode".to_string())?,
            value_dim: attention_value_dim
                .ok_or_else(|| "--attention-value-dim is required in attention mode".to_string())?,
            tolerance: attention_tolerance,
            max_peak_page_kv_ratio: attention_max_peak_page_kv_ratio,
            gate: attention_equivalence_gate,
        })?;
        write_or_print(output, &report)?;
        return Ok(());
    }
    if live_vram_event_trace_only {
        if !inputs.is_empty()
            || live_vram_export_dir.is_some()
            || compare_output_baseline.is_some()
            || compare_output_candidate.is_some()
            || compare_output_gate
            || live_vram_proof_gate
            || live_vram_runtime_reclaim_gate
            || live_vram_live_paging_gate
        {
            return Err(
                "--live-vram-event-trace-only cannot be combined with inputs, export replay, output comparison, or live-VRAM proof gates"
                    .to_string(),
            );
        }
        let trace = live_vram_event_trace.as_ref().ok_or_else(|| {
            "--live-vram-event-trace-only requires --live-vram-event-trace".to_string()
        })?;
        let events = parse_live_vram_event_trace_file(trace)?;
        let report = evaluate_live_vram_event_trace(&events, LiveVramEventTracePolicy::default());
        if live_vram_event_trace_gate && !report.passed() {
            return Err(format!(
                "live VRAM event trace gate failed: {}",
                event_trace_failure_summary(&report)
            ));
        }
        write_or_print(output, &render_event_trace_report_json(&report))?;
        return Ok(());
    }
    if live_vram_event_trace_gate {
        return Err(
            "--live-vram-event-trace-gate requires --live-vram-event-trace-only".to_string(),
        );
    }
    if compare_output_baseline.is_some() || compare_output_candidate.is_some() {
        if !inputs.is_empty() || live_vram_export_dir.is_some() {
            return Err(
                "--compare-output-baseline/--compare-output-candidate cannot be combined with --input, --dir, or --live-vram-export-dir"
                    .to_string(),
            );
        }
        let baseline = compare_output_baseline
            .ok_or_else(|| "--compare-output-baseline is required".to_string())?;
        let candidate = compare_output_candidate
            .ok_or_else(|| "--compare-output-candidate is required".to_string())?;
        let report = compare_llama_output_manifests(&baseline, &candidate)?;
        if compare_output_gate && !report.passed {
            let failures = report.failures.join("; ");
            return Err(format!(
                "llama.cpp output manifest comparison failed: {failures}"
            ));
        }
        write_or_print(output, &report.to_json())?;
        return Ok(());
    }
    if compare_output_gate {
        return Err(
            "--compare-output-gate requires --compare-output-baseline and --compare-output-candidate"
                .to_string(),
        );
    }
    if let Some(export_dir) = live_vram_export_dir {
        if !inputs.is_empty() {
            return Err(
                "--live-vram-export-dir cannot be combined with --input or --dir".to_string(),
            );
        }
        let report = run_live_vram_export_report(
            &export_dir,
            LiveVramCliOptions {
                runtime_commit: live_vram_runtime_commit.ok_or_else(|| {
                    "--live-vram-runtime-commit is required with --live-vram-export-dir".to_string()
                })?,
                adapter_version: live_vram_adapter_version.ok_or_else(|| {
                    "--live-vram-adapter-version is required with --live-vram-export-dir"
                        .to_string()
                })?,
                model_id: live_vram_model_id.ok_or_else(|| {
                    "--live-vram-model-id is required with --live-vram-export-dir".to_string()
                })?,
                current_token: live_vram_current_token,
                hot_window_tokens: live_vram_hot_window_tokens,
                prefetch_window_tokens: live_vram_prefetch_window_tokens,
                next_required: live_vram_next_required,
                max_queued_pages: live_vram_max_queued_pages,
                max_cpu_stored_bytes: live_vram_max_cpu_stored_bytes,
                max_tensors: live_vram_max_tensors,
                gpu_context_bytes: live_vram_gpu_context_bytes,
                allocation_granularity: live_vram_allocation_granularity,
                allocation_granularity_set: live_vram_allocation_granularity_set,
                restore_bytes_per_token: live_vram_restore_bytes_per_token,
                proof_gate: live_vram_proof_gate,
                runtime_reclaim_gate: live_vram_runtime_reclaim_gate,
                live_paging_gate: live_vram_live_paging_gate,
                event_trace: live_vram_event_trace,
                min_gpu_saved_ratio: live_vram_min_gpu_saved_ratio,
                aggregate_codec_gate: live_vram_aggregate_codec_gate,
                require_page_seals: live_vram_require_page_seals,
                page_seal_key: live_vram_page_seal_key,
            },
        )?;
        write_or_print(output, &report)?;
        return Ok(());
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
    write_or_print(output, &report)?;
    Ok(())
}

struct LiveVramCliOptions {
    runtime_commit: String,
    adapter_version: String,
    model_id: String,
    current_token: u64,
    hot_window_tokens: u64,
    prefetch_window_tokens: u64,
    next_required: LiveVramNextRequiredMode,
    max_queued_pages: usize,
    max_cpu_stored_bytes: usize,
    max_tensors: usize,
    gpu_context_bytes: Option<usize>,
    allocation_granularity: LiveVramGpuAllocationGranularity,
    allocation_granularity_set: bool,
    restore_bytes_per_token: Option<usize>,
    proof_gate: bool,
    runtime_reclaim_gate: bool,
    live_paging_gate: bool,
    event_trace: Option<PathBuf>,
    min_gpu_saved_ratio: f64,
    aggregate_codec_gate: bool,
    require_page_seals: bool,
    page_seal_key: Option<[u8; 32]>,
}

#[derive(Clone)]
struct AttentionTensorInput {
    dtype: TensorDType,
    path: PathBuf,
}

struct AttentionCliOptions {
    query: AttentionTensorInput,
    key_pages: Vec<AttentionTensorInput>,
    value_pages: Vec<AttentionTensorInput>,
    head_dim: usize,
    value_dim: usize,
    tolerance: f32,
    max_peak_page_kv_ratio: Option<f32>,
    gate: bool,
}

fn run_attention_equivalence_report(options: AttentionCliOptions) -> Result<String, String> {
    if options.head_dim == 0 {
        return Err("--attention-head-dim must be greater than zero".to_string());
    }
    if options.value_dim == 0 {
        return Err("--attention-value-dim must be greater than zero".to_string());
    }
    if options.key_pages.is_empty() {
        return Err("--attention-key-page is required at least once".to_string());
    }
    if options.key_pages.len() != options.value_pages.len() {
        return Err(
            "--attention-key-page and --attention-value-page counts must match".to_string(),
        );
    }
    let page_dtype = options.key_pages[0].dtype;
    if options
        .key_pages
        .iter()
        .any(|input| input.dtype != page_dtype)
    {
        return Err("all --attention-key-page entries must use the same dtype".to_string());
    }
    if options
        .value_pages
        .iter()
        .any(|input| input.dtype != page_dtype)
    {
        return Err(
            "--attention-value-page entries must use the same dtype as --attention-key-page"
                .to_string(),
        );
    }

    let query_bytes = read_limited_binary(&options.query.path, MAX_ATTENTION_TENSOR_BYTES)?;
    let query = decode_tensor_le_bytes_to_f32(&query_bytes, options.query.dtype)
        .map_err(|error| format!("failed to decode --attention-query: {error}"))?;
    if query.len() != options.head_dim {
        return Err(format!(
            "--attention-query decoded {} values but --attention-head-dim is {}",
            query.len(),
            options.head_dim
        ));
    }

    let key_pages = read_attention_pages(&options.key_pages)?;
    let value_pages = read_attention_pages(&options.value_pages)?;
    let key_refs: Vec<&[u8]> = key_pages.iter().map(Vec::as_slice).collect();
    let value_refs: Vec<&[u8]> = value_pages.iter().map(Vec::as_slice).collect();
    let report = compare_live_vram_typed_streaming_attention_reference(
        &query,
        &key_refs,
        &value_refs,
        page_dtype,
        options.head_dim,
        options.value_dim,
        options.tolerance,
    )
    .map_err(|error| format!("failed to compare typed page-bounded attention: {error}"))?;
    let key_pages_f32 =
        decode_attention_pages_for_summary(&key_pages, page_dtype).map_err(|error| {
            format!("failed to decode key pages for segment-summary attention: {error}")
        })?;
    let value_pages_f32 =
        decode_attention_pages_for_summary(&value_pages, page_dtype).map_err(|error| {
            format!("failed to decode value pages for segment-summary attention: {error}")
        })?;
    let key_f32_refs: Vec<&[f32]> = key_pages_f32.iter().map(Vec::as_slice).collect();
    let value_f32_refs: Vec<&[f32]> = value_pages_f32.iter().map(Vec::as_slice).collect();
    let segment_summary_report = compare_live_vram_segment_summary_attention_reference(
        &query,
        &key_f32_refs,
        &value_f32_refs,
        options.value_dim,
        options.tolerance,
    )
    .map_err(|error| format!("failed to compare segment-summary attention: {error}"))?;
    if options.gate && !report.passed {
        return Err(format!(
            "live VRAM attention equivalence gate failed: max_abs_error {:.9} exceeds tolerance {:.9}",
            report.max_abs_error, report.tolerance
        ));
    }
    if options.gate && !segment_summary_report.passed {
        return Err(format!(
            "live VRAM segment-summary attention gate failed: max_abs_error {:.9} exceeds tolerance {:.9}",
            segment_summary_report.max_abs_error, segment_summary_report.tolerance
        ));
    }
    if let Some(max_peak_page_kv_ratio) = options.max_peak_page_kv_ratio {
        let actual = report.streaming.peak_kv_value_ratio().unwrap_or(1.0);
        if actual > f64::from(max_peak_page_kv_ratio) {
            return Err(format!(
                "live VRAM attention peak page KV ratio gate failed: actual {:.9} exceeds max {:.9}",
                actual, max_peak_page_kv_ratio
            ));
        }
    }
    Ok(render_attention_equivalence_json(
        &report,
        &segment_summary_report,
        options.query.dtype,
        page_dtype,
    ))
}

fn read_attention_pages(inputs: &[AttentionTensorInput]) -> Result<Vec<Vec<u8>>, String> {
    inputs
        .iter()
        .map(|input| read_limited_binary(&input.path, MAX_ATTENTION_TENSOR_BYTES))
        .collect()
}

fn decode_attention_pages_for_summary(
    pages: &[Vec<u8>],
    dtype: TensorDType,
) -> Result<Vec<Vec<f32>>, qatq::QatqError> {
    pages
        .iter()
        .map(|page| decode_tensor_le_bytes_to_f32(page, dtype))
        .collect()
}

fn read_limited_binary(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to stat {}: {error}", path.display()))?;
    if metadata.len() > max_bytes {
        return Err(format!(
            "{} is too large: {} bytes > {} bytes",
            path.display(),
            metadata.len(),
            max_bytes
        ));
    }
    fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn render_attention_equivalence_json(
    report: &qatq::LiveVramStreamingAttentionEquivalenceReport,
    segment_summary_report: &qatq::LiveVramStreamingAttentionEquivalenceReport,
    query_dtype: TensorDType,
    page_dtype: TensorDType,
) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"format\": \"qatq-live-vram-attention-equivalence-v1\",\n");
    out.push_str(&format!("  \"passed\": {},\n", report.passed));
    out.push_str("  \"query_dtype\": ");
    push_json_string(&mut out, query_dtype.as_str());
    out.push_str(",\n");
    out.push_str("  \"page_dtype\": ");
    push_json_string(&mut out, page_dtype.as_str());
    out.push_str(",\n");
    out.push_str(&format!("  \"head_dim\": {},\n", report.streaming.head_dim));
    out.push_str(&format!(
        "  \"value_dim\": {},\n",
        report.streaming.value_dim
    ));
    out.push_str(&format!("  \"pages\": {},\n", report.streaming.pages));
    out.push_str(&format!("  \"tokens\": {},\n", report.streaming.tokens));
    out.push_str(&format!("  \"tolerance\": {:.9},\n", report.tolerance));
    out.push_str(&format!(
        "  \"max_abs_error\": {:.9},\n",
        report.max_abs_error
    ));
    out.push_str(&format!(
        "  \"max_relative_error\": {:.9},\n",
        report.max_relative_error
    ));
    out.push_str(&format!(
        "  \"peak_page_kv_values\": {},\n",
        report.streaming.peak_page_kv_values
    ));
    out.push_str(&format!(
        "  \"materialized_kv_values\": {},\n",
        report.streaming.materialized_kv_values
    ));
    out.push_str(&format!(
        "  \"peak_page_kv_ratio\": {:.9},\n",
        report.streaming.peak_kv_value_ratio().unwrap_or(0.0)
    ));
    out.push_str("  \"segment_summary_reduction\": \"online-page-summary\",\n");
    out.push_str(&format!(
        "  \"segment_summary_passed\": {},\n",
        segment_summary_report.passed
    ));
    out.push_str(&format!(
        "  \"segment_summary_max_abs_error\": {:.9},\n",
        segment_summary_report.max_abs_error
    ));
    out.push_str(&format!(
        "  \"segment_summary_max_relative_error\": {:.9},\n",
        segment_summary_report.max_relative_error
    ));
    out.push_str(&format!(
        "  \"segment_summary_peak_page_kv_values\": {},\n",
        segment_summary_report.streaming.peak_page_kv_values
    ));
    out.push_str(&format!(
        "  \"segment_summary_peak_page_kv_ratio\": {:.9}\n",
        segment_summary_report
            .streaming
            .peak_kv_value_ratio()
            .unwrap_or(0.0)
    ));
    out.push_str("}\n");
    out
}

fn run_live_vram_export_report(
    export_dir: &Path,
    options: LiveVramCliOptions,
) -> Result<String, String> {
    if options.live_paging_gate && options.runtime_reclaim_gate {
        return Err(
            "--live-vram-live-paging-gate cannot be combined with --live-vram-runtime-reclaim-gate"
                .to_string(),
        );
    }
    if options.live_paging_gate && options.event_trace.is_none() {
        return Err("--live-vram-live-paging-gate requires --live-vram-event-trace".to_string());
    }
    let limits = LiveVramLimits::default();
    let manifest_text = fs::read_to_string(export_dir.join("manifest.json"))
        .map_err(|error| format!("failed to read llama.cpp live VRAM manifest: {error}"))?;
    let manifest = parse_llama_cpp_kv_manifest(&manifest_text)
        .map_err(|error| format!("failed to parse llama.cpp live VRAM manifest: {error}"))?;
    if options.live_paging_gate {
        let page_residency = manifest.live_page_residency_granularity.ok_or_else(|| {
            "--live-vram-live-paging-gate requires runtime-attested live_page_residency_granularity in manifest.json"
                .to_string()
        })?;
        if !page_residency.can_track_logical_pages() {
            return Err(format!(
                "--live-vram-live-paging-gate requires live_page_residency_granularity=per-page, got {}",
                page_residency.as_str()
            ));
        }
        if matches!(
            manifest.gpu_allocation_granularity,
            Some(LiveVramGpuAllocationGranularity::PerPage)
        ) && let (Some(staging_bytes), Some(total_context_bytes)) = (
            manifest.gpu_page_staging_bytes,
            manifest.total_context_bytes,
        ) && staging_bytes >= total_context_bytes
        {
            return Err(format!(
                "--live-vram-live-paging-gate requires page-staged GPU bytes below total KV context bytes, got {staging_bytes} >= {total_context_bytes}"
            ));
        }
    }
    let mut snapshots = live_vram_snapshots_from_llama_cpp_export_dir(
        export_dir,
        &LlamaCppKvExportReplayConfig {
            runtime_commit: options.runtime_commit,
            adapter_version: options.adapter_version,
            model_id: options.model_id,
            max_tensors: options.max_tensors,
            next_required_token: options.next_required.uniform_token(
                options.current_token,
                options.hot_window_tokens,
                options.prefetch_window_tokens,
            ),
        },
        limits,
    )
    .map_err(|error| format!("failed to load llama.cpp live VRAM export: {error}"))?;
    options.next_required.apply_per_page(
        &mut snapshots,
        options.current_token,
        options.hot_window_tokens,
        options.prefetch_window_tokens,
    );
    let policy = LiveVramSchedulerPolicy {
        hot_window_tokens: options.hot_window_tokens,
        prefetch_window_tokens: options.prefetch_window_tokens,
        max_queued_pages: options.max_queued_pages,
        max_cpu_stored_bytes: options.max_cpu_stored_bytes,
        require_qatq_beats_best_general_codec: !options.aggregate_codec_gate,
        ..LiveVramSchedulerPolicy::default()
    };
    let scheduler_state = LiveVramSchedulerState {
        current_token: options.current_token,
        queued_pages: 0,
        cpu_stored_bytes: 0,
    };
    let seal_context = live_vram_page_seal_context(export_dir);
    let report = if let Some(page_seal_key) = options.page_seal_key.as_ref() {
        build_live_vram_evidence_report_with_page_seals(
            &snapshots,
            scheduler_state,
            policy,
            limits,
            page_seal_key,
            seal_context.as_bytes(),
        )
    } else {
        if options.live_paging_gate {
            return Err(
                "--live-vram-live-paging-gate requires --live-vram-page-seal-key-hex".to_string(),
            );
        }
        if options.require_page_seals {
            return Err(
                "--live-vram-require-page-seals requires --live-vram-page-seal-key-hex".to_string(),
            );
        }
        build_live_vram_evidence_report(&snapshots, scheduler_state, policy, limits)
    }
    .map_err(|error| format!("failed to build live VRAM evidence report: {error}"))?;
    if options.require_page_seals && report.sealed_pages != report.offloaded_pages {
        return Err(format!(
            "--live-vram-require-page-seals expected every offloaded page to carry metadata seals, got {}/{}",
            report.sealed_pages, report.offloaded_pages
        ));
    }
    if options.allocation_granularity_set && options.gpu_context_bytes.is_none() {
        return Err(
            "--live-vram-allocation-granularity requires --live-vram-gpu-context-bytes".to_string(),
        );
    }
    let residency_estimate = if options.proof_gate
        || options.runtime_reclaim_gate
        || options.live_paging_gate
    {
        let gpu_context_bytes = manifest.gpu_context_bytes.ok_or_else(|| {
            "live VRAM gate requires runtime-attested gpu_context_bytes in manifest.json"
                .to_string()
        })?;
        let allocation_granularity = manifest.gpu_allocation_granularity.ok_or_else(|| {
            "live VRAM gate requires runtime-attested gpu_allocation_granularity in manifest.json"
                .to_string()
        })?;
        if options.runtime_reclaim_gate || options.live_paging_gate {
            let total_context_bytes = manifest.total_context_bytes.ok_or_else(|| {
                "--live-vram-runtime-reclaim-gate/--live-vram-live-paging-gate requires runtime-attested total_context_bytes in manifest.json"
                    .to_string()
            })?;
            Some(
                estimate_live_vram_residency_from_runtime_allocation(
                    &report,
                    total_context_bytes,
                    gpu_context_bytes,
                    allocation_granularity,
                )
                .map_err(|error| {
                    format!("failed to estimate runtime live VRAM residency: {error}")
                })?,
            )
        } else {
            Some(estimate_live_vram_residency_after_offload(
                &report,
                gpu_context_bytes,
                allocation_granularity,
            ))
        }
    } else {
        options.gpu_context_bytes.map(|gpu_context_bytes| {
            estimate_live_vram_residency_after_offload(
                &report,
                gpu_context_bytes,
                options.allocation_granularity,
            )
        })
    };
    let restore_deadline_report = options
        .restore_bytes_per_token
        .map(|restore_bytes_per_token| {
            evaluate_live_vram_prefetch_deadlines(
                &report,
                options.current_token,
                LiveVramPrefetchBudget {
                    restore_bytes_per_token,
                },
            )
        })
        .transpose()
        .map_err(|error| format!("failed to evaluate live VRAM restore deadlines: {error}"))?;

    if options.proof_gate {
        let proof = evaluate_live_vram_proof_gate(
            &report,
            residency_estimate.as_ref(),
            restore_deadline_report.as_ref(),
            LiveVramProofGate {
                min_gpu_saved_ratio: options.min_gpu_saved_ratio,
                require_aggregate_qatq_beats_best_general_codec: options.aggregate_codec_gate,
                require_all_pages_beat_best_general_codec: !options.aggregate_codec_gate,
                ..LiveVramProofGate::default()
            },
        )
        .map_err(|error| format!("failed to evaluate live VRAM proof gate: {error}"))?;
        if !proof.passed() {
            let failures = format_failure_messages(proof.failures.iter());
            return Err(format!("live VRAM proof gate failed: {failures}"));
        }
    }
    if options.runtime_reclaim_gate {
        let proof = evaluate_live_vram_proof_gate(
            &report,
            residency_estimate.as_ref(),
            restore_deadline_report.as_ref(),
            LiveVramProofGate {
                min_gpu_saved_ratio: options.min_gpu_saved_ratio,
                require_page_granular_reclaim: false,
                require_aggregate_qatq_beats_best_general_codec: options.aggregate_codec_gate,
                require_all_pages_beat_best_general_codec: !options.aggregate_codec_gate,
                ..LiveVramProofGate::default()
            },
        )
        .map_err(|error| format!("failed to evaluate live VRAM runtime reclaim gate: {error}"))?;
        if !proof.passed() {
            let failures = format_failure_messages(proof.failures.iter());
            return Err(format!("live VRAM runtime reclaim gate failed: {failures}"));
        }
    }
    let event_trace_events = match options.event_trace.as_ref() {
        Some(path) => Some(parse_live_vram_event_trace_file(path)?),
        None => None,
    };
    let event_trace_report = event_trace_events
        .as_ref()
        .map(|events| evaluate_live_vram_event_trace(events, LiveVramEventTracePolicy::default()));
    if options.live_paging_gate {
        let proof = evaluate_live_vram_live_paging_proof_gate(
            &report,
            residency_estimate.as_ref(),
            restore_deadline_report.as_ref(),
            event_trace_events
                .as_deref()
                .expect("validated live paging gate event trace"),
            LiveVramProofGate {
                min_gpu_saved_ratio: options.min_gpu_saved_ratio,
                require_aggregate_qatq_beats_best_general_codec: options.aggregate_codec_gate,
                require_all_pages_beat_best_general_codec: !options.aggregate_codec_gate,
                ..LiveVramProofGate::default()
            },
            LiveVramEventTracePolicy::default(),
        )
        .map_err(|error| format!("failed to evaluate live VRAM live-paging gate: {error}"))?;
        if !proof.passed() {
            let failures = format_failure_messages(
                proof
                    .proof_gate
                    .failures
                    .iter()
                    .map(ToString::to_string)
                    .chain(proof.event_trace.failures.iter().map(ToString::to_string)),
            );
            return Err(format!("live VRAM live-paging gate failed: {}", failures));
        }
    }

    Ok(report.to_json_with_runtime_estimates_and_event_trace(
        residency_estimate.as_ref(),
        restore_deadline_report.as_ref(),
        event_trace_report.as_ref(),
    ))
}

fn format_failure_messages(messages: impl IntoIterator<Item = impl ToString>) -> String {
    let mut counts = BTreeMap::<String, usize>::new();
    for message in messages {
        *counts.entry(message.to_string()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .map(|(message, count)| {
            if count == 1 {
                message
            } else {
                format!("{message} (x{count})")
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveVramNextRequiredMode {
    UniformAfterHot,
    PageEnd,
    ColdAfterHot,
}

impl LiveVramNextRequiredMode {
    fn uniform_token(
        self,
        current_token: u64,
        hot_window_tokens: u64,
        prefetch_window_tokens: u64,
    ) -> Option<u64> {
        match self {
            Self::UniformAfterHot => Some(
                current_token
                    .saturating_add(hot_window_tokens)
                    .saturating_add(prefetch_window_tokens)
                    .saturating_add(1),
            ),
            Self::PageEnd | Self::ColdAfterHot => None,
        }
    }

    fn apply_per_page(
        self,
        snapshots: &mut [qatq::KvPageSnapshot],
        current_token: u64,
        hot_window_tokens: u64,
        prefetch_window_tokens: u64,
    ) {
        if self != Self::ColdAfterHot {
            return;
        }
        let hot_start = current_token.saturating_sub(hot_window_tokens);
        let cold_next_required = current_token
            .saturating_add(hot_window_tokens)
            .saturating_add(prefetch_window_tokens)
            .saturating_add(1);
        for snapshot in snapshots {
            if snapshot.descriptor.token_end > hot_start {
                snapshot.descriptor.next_required_token = Some(snapshot.descriptor.token_end);
            } else {
                snapshot.descriptor.next_required_token = Some(cold_next_required);
            }
        }
    }
}

fn parse_live_vram_event_trace_file(path: &Path) -> Result<Vec<LiveVramPageEvent>, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to stat {}: {error}", path.display()))?;
    if metadata.len() > MAX_LIVE_VRAM_EVENT_TRACE_BYTES {
        return Err(format!(
            "{} is too large for a live VRAM event trace: {} bytes > {} bytes",
            path.display(),
            metadata.len(),
            MAX_LIVE_VRAM_EVENT_TRACE_BYTES
        ));
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse_live_vram_event_trace(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn parse_live_vram_event_trace(text: &str) -> Result<Vec<LiveVramPageEvent>, String> {
    let format = match json_string_field(text, "format") {
        Ok(format) => format,
        Err(_) => return parse_live_vram_event_trace_jsonl(text),
    };
    if format != "qatq-live-vram-event-trace-v1" {
        return Err(format!("unsupported live VRAM event trace format {format}"));
    }
    let objects = json_array_object_slices(text, "events")?;
    let mut events = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        events.push(parse_live_vram_event_trace_object(object).map_err(|error| {
            format!("invalid live VRAM event trace event at index {index}: {error}")
        })?);
    }
    Ok(events)
}

fn parse_live_vram_event_trace_jsonl(text: &str) -> Result<Vec<LiveVramPageEvent>, String> {
    let mut events = Vec::new();
    for (line_number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        events.push(parse_live_vram_event_trace_object(line).map_err(|error| {
            format!(
                "invalid live VRAM JSONL event at line {}: {error}",
                line_number + 1
            )
        })?);
    }
    if events.is_empty() {
        return Err("live VRAM JSONL event trace is empty".to_string());
    }
    Ok(events)
}

fn parse_live_vram_event_trace_object(object: &str) -> Result<LiveVramPageEvent, String> {
    Ok(LiveVramPageEvent {
        token: json_u64_field(object, "token")?,
        key: KvPageKey {
            runtime_id: json_string_field(object, "runtime_id")?,
            model_id: json_string_field(object, "model_id")?,
            seq_id: json_string_field(object, "seq_id")?,
            layer_id: parse_u32_json_field(object, "layer_id")?,
            kind: parse_kv_page_kind(&json_string_field(object, "kind")?)?,
            token_start: json_u64_field(object, "token_start")?,
            token_end: json_u64_field(object, "token_end")?,
        },
        kind: parse_live_vram_event_kind(&json_string_field(object, "event")?)?,
        checksum: json_optional_u64_field(object, "checksum")?,
    })
}

fn render_event_trace_report_json(report: &LiveVramEventTraceReport) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"format\": \"qatq-live-vram-event-trace-report-v1\",\n");
    out.push_str(&format!("  \"passed\": {},\n", report.passed()));
    out.push_str(&format!("  \"events\": {},\n", report.events));
    out.push_str(&format!("  \"snapshots\": {},\n", report.snapshots));
    out.push_str(&format!("  \"offloads\": {},\n", report.offloads));
    out.push_str(&format!("  \"restores\": {},\n", report.restores));
    out.push_str(&format!(
        "  \"attention_uses\": {},\n",
        report.attention_uses
    ));
    out.push_str(&format!("  \"cancellations\": {},\n", report.cancellations));
    out.push_str(&format!(
        "  \"peak_offloaded_pages\": {},\n",
        report.peak_offloaded_pages
    ));
    out.push_str(&format!(
        "  \"offloaded_pages_at_end\": {},\n",
        report.offloaded_pages_at_end
    ));
    out.push_str("  \"failures\": [");
    for (index, failure) in report.failures.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        push_json_string(&mut out, &failure.to_string());
    }
    out.push_str("]\n");
    out.push_str("}\n");
    out
}

fn event_trace_failure_summary(report: &LiveVramEventTraceReport) -> String {
    if report.failures.is_empty() {
        return "unknown failure".to_string();
    }
    report
        .failures
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

fn parse_u32_json_field(text: &str, key: &str) -> Result<u32, String> {
    let value = json_u64_field(text, key)?;
    value
        .try_into()
        .map_err(|_| format!("field {key} is too large for u32"))
}

fn parse_kv_page_kind(value: &str) -> Result<KvPageKind, String> {
    match value {
        "key" | "k" => Ok(KvPageKind::Key),
        "value" | "v" => Ok(KvPageKind::Value),
        other => Err(format!("unsupported live VRAM page kind {other}")),
    }
}

fn parse_live_vram_event_kind(value: &str) -> Result<LiveVramPageEventKind, String> {
    match value {
        "snapshot" => Ok(LiveVramPageEventKind::Snapshot),
        "offload-committed" => Ok(LiveVramPageEventKind::OffloadCommitted),
        "restore-committed" => Ok(LiveVramPageEventKind::RestoreCommitted),
        "attention-use" => Ok(LiveVramPageEventKind::AttentionUse),
        "cancelled" => Ok(LiveVramPageEventKind::Cancelled),
        "cancelled-before-runtime-commit" => {
            Ok(LiveVramPageEventKind::CancelledBeforeRuntimeCommit)
        }
        "cancelled-after-runtime-commit" => Ok(LiveVramPageEventKind::CancelledAfterRuntimeCommit),
        other => Err(format!("unsupported live VRAM event kind {other}")),
    }
}

#[derive(Debug)]
struct LlamaOutputManifest {
    format: String,
    model_path_hash: u64,
    prompt_hash: u64,
    n_prompt: u64,
    n_predict: u64,
    n_gpu_layers: i64,
    offload_kqv: bool,
    qatq_kv_gpu_layers: i64,
    cache_type_k: String,
    cache_type_v: String,
    n_decode: u64,
    total_us: u64,
    generated_text_hash: u64,
    generated_tokens: Vec<i64>,
}

struct OutputManifestComparison {
    passed: bool,
    failures: Vec<String>,
    baseline: LlamaOutputManifest,
    candidate: LlamaOutputManifest,
}

impl OutputManifestComparison {
    fn to_json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"format\": \"qatq-llama-cpp-output-comparison-v1\",\n");
        out.push_str(&format!("  \"passed\": {},\n", self.passed));
        out.push_str("  \"failures\": [");
        for (index, failure) in self.failures.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            push_json_string(&mut out, failure);
        }
        out.push_str("],\n");
        out.push_str("  \"baseline\": ");
        push_output_manifest_summary_json(&mut out, &self.baseline);
        out.push_str(",\n");
        out.push_str("  \"candidate\": ");
        push_output_manifest_summary_json(&mut out, &self.candidate);
        out.push('\n');
        out.push_str("}\n");
        out
    }
}

fn compare_llama_output_manifests(
    baseline_path: &Path,
    candidate_path: &Path,
) -> Result<OutputManifestComparison, String> {
    let baseline = parse_llama_output_manifest_file(baseline_path)?;
    let candidate = parse_llama_output_manifest_file(candidate_path)?;
    let mut failures = Vec::new();
    if baseline.format != "qatq-llama-cpp-output-v1" {
        failures.push(format!(
            "baseline format {} is unsupported",
            baseline.format
        ));
    }
    if candidate.format != "qatq-llama-cpp-output-v1" {
        failures.push(format!(
            "candidate format {} is unsupported",
            candidate.format
        ));
    }
    if baseline.model_path_hash != candidate.model_path_hash {
        failures.push("model_path_hash differs".to_string());
    }
    if baseline.prompt_hash != candidate.prompt_hash {
        failures.push("prompt_hash differs".to_string());
    }
    if baseline.n_prompt != candidate.n_prompt {
        failures.push("n_prompt differs".to_string());
    }
    if baseline.n_predict != candidate.n_predict {
        failures.push("n_predict differs".to_string());
    }
    if baseline.cache_type_k != candidate.cache_type_k {
        failures.push("cache_type_k differs".to_string());
    }
    if baseline.cache_type_v != candidate.cache_type_v {
        failures.push("cache_type_v differs".to_string());
    }
    if baseline.n_decode != candidate.n_decode {
        failures.push("n_decode differs".to_string());
    }
    if baseline.generated_text_hash != candidate.generated_text_hash {
        failures.push("generated_text_hash differs".to_string());
    }
    if baseline.generated_tokens != candidate.generated_tokens {
        failures.push(first_token_difference(
            &baseline.generated_tokens,
            &candidate.generated_tokens,
        ));
    }
    Ok(OutputManifestComparison {
        passed: failures.is_empty(),
        failures,
        baseline,
        candidate,
    })
}

fn first_token_difference(baseline: &[i64], candidate: &[i64]) -> String {
    let shared = baseline.len().min(candidate.len());
    for index in 0..shared {
        if baseline[index] != candidate[index] {
            return format!(
                "generated_tokens differ at index {index}: baseline={} candidate={}",
                baseline[index], candidate[index]
            );
        }
    }
    format!(
        "generated_tokens length differs: baseline={} candidate={}",
        baseline.len(),
        candidate.len()
    )
}

fn parse_llama_output_manifest_file(path: &Path) -> Result<LlamaOutputManifest, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to stat {}: {error}", path.display()))?;
    if metadata.len() > MAX_OUTPUT_MANIFEST_BYTES {
        return Err(format!(
            "{} is too large for a llama.cpp output manifest: {} bytes > {} bytes",
            path.display(),
            metadata.len(),
            MAX_OUTPUT_MANIFEST_BYTES
        ));
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse_llama_output_manifest(&text)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn parse_llama_output_manifest(text: &str) -> Result<LlamaOutputManifest, String> {
    Ok(LlamaOutputManifest {
        format: json_string_field(text, "format")?,
        model_path_hash: json_u64_field(text, "model_path_hash")?,
        prompt_hash: json_u64_field(text, "prompt_hash")?,
        n_prompt: json_u64_field(text, "n_prompt")?,
        n_predict: json_u64_field(text, "n_predict")?,
        n_gpu_layers: json_i64_field(text, "n_gpu_layers")?,
        offload_kqv: json_bool_field(text, "offload_kqv")?,
        qatq_kv_gpu_layers: json_i64_field(text, "qatq_kv_gpu_layers")?,
        cache_type_k: json_string_field(text, "cache_type_k")?,
        cache_type_v: json_string_field(text, "cache_type_v")?,
        n_decode: json_u64_field(text, "n_decode")?,
        total_us: json_u64_field(text, "total_us")?,
        generated_text_hash: json_u64_field(text, "generated_text_hash")?,
        generated_tokens: json_i64_array_field(text, "generated_tokens")?,
    })
}

fn json_field_value<'a>(text: &'a str, key: &str) -> Result<&'a str, String> {
    let needle = format!("\"{key}\"");
    let key_pos = text
        .find(&needle)
        .ok_or_else(|| format!("missing field {key}"))?;
    let after_key = &text[key_pos + needle.len()..];
    let colon_pos = after_key
        .find(':')
        .ok_or_else(|| format!("missing colon after field {key}"))?;
    Ok(after_key[colon_pos + 1..].trim_start())
}

fn json_string_field(text: &str, key: &str) -> Result<String, String> {
    let value = json_field_value(text, key)?;
    let bytes = value.as_bytes();
    if bytes.first() != Some(&b'"') {
        return Err(format!("field {key} must be a JSON string"));
    }
    let mut out = String::new();
    let mut index = 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Ok(out),
            b'\\' => {
                index += 1;
                if index >= bytes.len() {
                    return Err(format!("field {key} has an unterminated escape"));
                }
                match bytes[index] {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    other => {
                        return Err(format!(
                            "field {key} has unsupported escape \\{}",
                            other as char
                        ));
                    }
                }
            }
            byte => out.push(byte as char),
        }
        index += 1;
    }
    Err(format!("field {key} has an unterminated string"))
}

fn json_u64_field(text: &str, key: &str) -> Result<u64, String> {
    let raw = json_number_text(text, key)?;
    raw.parse()
        .map_err(|error| format!("invalid u64 field {key}: {error}"))
}

fn json_optional_u64_field(text: &str, key: &str) -> Result<Option<u64>, String> {
    match json_number_text(text, key) {
        Ok(raw) => raw
            .parse()
            .map(Some)
            .map_err(|error| format!("invalid u64 field {key}: {error}")),
        Err(error) if error == format!("missing field {key}") => Ok(None),
        Err(error) => Err(error),
    }
}

fn json_i64_field(text: &str, key: &str) -> Result<i64, String> {
    let raw = json_number_text(text, key)?;
    raw.parse()
        .map_err(|error| format!("invalid i64 field {key}: {error}"))
}

fn json_number_text<'a>(text: &'a str, key: &str) -> Result<&'a str, String> {
    let value = json_field_value(text, key)?;
    let len = value
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit() || *ch == '-')
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .ok_or_else(|| format!("field {key} must be a JSON number"))?;
    Ok(&value[..len])
}

fn json_bool_field(text: &str, key: &str) -> Result<bool, String> {
    let value = json_field_value(text, key)?;
    if value.starts_with("true") {
        Ok(true)
    } else if value.starts_with("false") {
        Ok(false)
    } else {
        Err(format!("field {key} must be a JSON boolean"))
    }
}

fn json_i64_array_field(text: &str, key: &str) -> Result<Vec<i64>, String> {
    let value = json_field_value(text, key)?;
    let bytes = value.as_bytes();
    if bytes.first() != Some(&b'[') {
        return Err(format!("field {key} must be a JSON integer array"));
    }
    let close = value
        .find(']')
        .ok_or_else(|| format!("field {key} has an unterminated array"))?;
    let body = value[1..close].trim();
    if body.is_empty() {
        return Ok(Vec::new());
    }
    body.split(',')
        .map(|item| {
            item.trim()
                .parse()
                .map_err(|error| format!("invalid integer in field {key}: {error}"))
        })
        .collect()
}

fn json_array_object_slices<'a>(text: &'a str, key: &str) -> Result<Vec<&'a str>, String> {
    let value = json_field_value(text, key)?;
    let bytes = value.as_bytes();
    if bytes.first() != Some(&b'[') {
        return Err(format!("field {key} must be a JSON object array"));
    }
    let mut index = 1;
    let mut objects = Vec::new();
    loop {
        while matches!(bytes.get(index), Some(b' ' | b'\n' | b'\r' | b'\t' | b',')) {
            index += 1;
        }
        match bytes.get(index) {
            Some(b']') => return Ok(objects),
            Some(b'{') => {
                let start = index;
                let mut depth = 0_usize;
                let mut in_string = false;
                let mut escaped = false;
                while let Some(byte) = bytes.get(index) {
                    if in_string {
                        if escaped {
                            escaped = false;
                        } else if *byte == b'\\' {
                            escaped = true;
                        } else if *byte == b'"' {
                            in_string = false;
                        }
                    } else if *byte == b'"' {
                        in_string = true;
                    } else if *byte == b'{' {
                        depth += 1;
                    } else if *byte == b'}' {
                        depth = depth
                            .checked_sub(1)
                            .ok_or_else(|| format!("field {key} has mismatched braces"))?;
                        if depth == 0 {
                            let end = index + 1;
                            objects.push(&value[start..end]);
                            index = end;
                            break;
                        }
                    }
                    index += 1;
                }
                if depth != 0 {
                    return Err(format!("field {key} has an unterminated object"));
                }
            }
            Some(_) => return Err(format!("field {key} must contain JSON objects")),
            None => return Err(format!("field {key} has an unterminated array")),
        }
    }
}

fn push_output_manifest_summary_json(out: &mut String, manifest: &LlamaOutputManifest) {
    out.push_str("{\n");
    out.push_str(&format!(
        "    \"model_path_hash\": {},\n",
        manifest.model_path_hash
    ));
    out.push_str(&format!("    \"prompt_hash\": {},\n", manifest.prompt_hash));
    out.push_str(&format!("    \"n_prompt\": {},\n", manifest.n_prompt));
    out.push_str(&format!("    \"n_predict\": {},\n", manifest.n_predict));
    out.push_str(&format!(
        "    \"n_gpu_layers\": {},\n",
        manifest.n_gpu_layers
    ));
    out.push_str(&format!("    \"offload_kqv\": {},\n", manifest.offload_kqv));
    out.push_str(&format!(
        "    \"qatq_kv_gpu_layers\": {},\n",
        manifest.qatq_kv_gpu_layers
    ));
    out.push_str("    \"cache_type_k\": ");
    push_json_string(out, &manifest.cache_type_k);
    out.push_str(",\n");
    out.push_str("    \"cache_type_v\": ");
    push_json_string(out, &manifest.cache_type_v);
    out.push_str(",\n");
    out.push_str(&format!("    \"n_decode\": {},\n", manifest.n_decode));
    out.push_str(&format!("    \"total_us\": {},\n", manifest.total_us));
    out.push_str(&format!(
        "    \"generated_text_hash\": {},\n",
        manifest.generated_text_hash
    ));
    out.push_str(&format!(
        "    \"generated_token_count\": {}\n",
        manifest.generated_tokens.len()
    ));
    out.push_str("  }");
}

fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
}

fn required_arg(args: &[String], index: usize, name: &str) -> Result<String, String> {
    args.get(index + 1)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| format!("{name} requires a value"))
}

fn parse_usize_arg(args: &[String], index: usize, name: &str) -> Result<usize, String> {
    let value = required_arg(args, index, name)?;
    value
        .parse()
        .map_err(|error| format!("invalid {name}: {error}"))
}

fn parse_u64_arg(args: &[String], index: usize, name: &str) -> Result<u64, String> {
    let value = required_arg(args, index, name)?;
    value
        .parse()
        .map_err(|error| format!("invalid {name}: {error}"))
}

fn parse_f32_arg(args: &[String], index: usize, name: &str) -> Result<f32, String> {
    let value = required_arg(args, index, name)?;
    value
        .parse()
        .map_err(|error| format!("invalid {name}: {error}"))
}

fn parse_ratio_arg(args: &[String], index: usize, name: &str) -> Result<f64, String> {
    let value = required_arg(args, index, name)?;
    let parsed = value
        .parse::<f64>()
        .map_err(|error| format!("invalid {name}: {error}"))?;
    if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
        return Err(format!("{name} must be a finite ratio between 0 and 1"));
    }
    Ok(parsed)
}

fn parse_hex_32_arg(args: &[String], index: usize, name: &str) -> Result<[u8; 32], String> {
    let value = required_arg(args, index, name)?;
    if value.len() != 64 {
        return Err(format!("{name} must be exactly 64 hex characters"));
    }
    let mut out = [0_u8; 32];
    for (slot, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_hex_nibble(pair[0])
            .ok_or_else(|| format!("{name} contains a non-hex character"))?;
        let low = decode_hex_nibble(pair[1])
            .ok_or_else(|| format!("{name} contains a non-hex character"))?;
        out[slot] = (high << 4) | low;
    }
    Ok(out)
}

fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn live_vram_page_seal_context(export_dir: &Path) -> String {
    format!("qatq-kv-bench-live-vram-evidence:{}", export_dir.display())
}

fn parse_live_vram_allocation_granularity(
    value: &str,
) -> Result<LiveVramGpuAllocationGranularity, String> {
    match value {
        "per-page" => Ok(LiveVramGpuAllocationGranularity::PerPage),
        "whole-tensor" => Ok(LiveVramGpuAllocationGranularity::WholeTensor),
        "whole-context" => Ok(LiveVramGpuAllocationGranularity::WholeContext),
        "runtime-unknown" => Ok(LiveVramGpuAllocationGranularity::RuntimeUnknown),
        other => Err(format!(
            "unsupported --live-vram-allocation-granularity {other}; expected per-page, whole-tensor, whole-context, or runtime-unknown"
        )),
    }
}

fn parse_live_vram_next_required_mode(value: &str) -> Result<LiveVramNextRequiredMode, String> {
    match value {
        "uniform-after-hot" => Ok(LiveVramNextRequiredMode::UniformAfterHot),
        "page-end" => Ok(LiveVramNextRequiredMode::PageEnd),
        "cold-after-hot" => Ok(LiveVramNextRequiredMode::ColdAfterHot),
        other => Err(format!(
            "unsupported --live-vram-next-required {other}; expected uniform-after-hot, page-end, or cold-after-hot"
        )),
    }
}

fn write_or_print(output: Option<PathBuf>, report: &str) -> Result<(), String> {
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

fn parse_attention_tensor_arg(value: &str) -> Result<AttentionTensorInput, String> {
    let mut parts = value.splitn(2, ':');
    let dtype = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| "attention tensor dtype is required".to_string())
        .and_then(parse_dtype)?;
    let path = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| "attention tensor path is required".to_string())?;
    Ok(AttentionTensorInput {
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
    eprintln!(
        "  qatq-kv-bench --live-vram-export-dir <patched-llama-export-dir> --live-vram-runtime-commit <commit> --live-vram-adapter-version <version> --live-vram-model-id <id> [--live-vram-next-required uniform-after-hot|page-end|cold-after-hot] [--live-vram-hot-window-tokens N --live-vram-prefetch-window-tokens N] [--live-vram-max-queued-pages N] [--live-vram-gpu-context-bytes N --live-vram-allocation-granularity per-page|whole-tensor|whole-context|runtime-unknown] [--live-vram-restore-bytes-per-token N] [--live-vram-event-trace trace.json] [--live-vram-runtime-reclaim-gate|--live-vram-proof-gate|--live-vram-live-paging-gate --live-vram-page-seal-key-hex <64-hex> --live-vram-require-page-seals --live-vram-min-gpu-saved-ratio R] [--live-vram-aggregate-codec-gate] [--output evidence.json]"
    );
    eprintln!(
        "  qatq-kv-bench --live-vram-event-trace-only --live-vram-event-trace <trace.json> [--live-vram-event-trace-gate] [--output report.json]"
    );
    eprintln!(
        "  qatq-kv-bench --compare-output-baseline <manifest.json> --compare-output-candidate <manifest.json> [--compare-output-gate] [--output comparison.json]"
    );
    eprintln!(
        "  qatq-kv-bench --attention-query <f32|f16|bf16>:<path> --attention-key-page <f32|f16|bf16>:<path> [...] --attention-value-page <f32|f16|bf16>:<path> [...] --attention-head-dim N --attention-value-dim N [--attention-tolerance F] [--attention-max-peak-page-kv-ratio R] [--attention-equivalence-gate] [--output evidence.json]"
    );
}
