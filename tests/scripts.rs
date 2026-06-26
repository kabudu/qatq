use std::process::Command;

fn run_python_snippet(snippet: &str) -> std::process::Output {
    Command::new("python3")
        .arg("-c")
        .arg(snippet)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .expect("run python3")
}

#[test]
fn live_vram_matrix_applies_long_context_config_gates() {
    let output = run_python_snippet(
        r#"
import argparse, importlib.util, json, sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("matrix", "scripts/llama_cpp_live_vram_matrix.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

config = json.loads(Path("adapters/llama-cpp/live-vram-native-long-context-latency.local.example.json").read_text())
case = config["cases"][0]
args = argparse.Namespace(
    require_stable_reclaim=False,
    require_stable_qc_bytes=False,
    max_elapsed_jitter_ratio=0.0,
    deep_latency_baseline=False,
    min_token_latency_samples=0,
    max_mixed_token_p95_regression_ratio=0.0,
    max_mixed_token_p99_regression_ratio=0.0,
    min_deep_token_latency_samples=0,
    max_deep_mixed_token_p95_regression_ratio=0.0,
    max_deep_mixed_token_p99_regression_ratio=0.0,
    host_memory_pressure_mib=0,
)
module.apply_config_gates(args, config)
module.validate_gate_args(args)
assert args.require_stable_reclaim is True
assert args.require_stable_qc_bytes is True
assert args.deep_latency_baseline is True
assert args.min_token_latency_samples == 128
assert args.min_deep_token_latency_samples == 128
assert args.max_mixed_token_p95_regression_ratio == 0.15
assert args.max_mixed_token_p99_regression_ratio == 0.20
assert args.max_deep_mixed_token_p95_regression_ratio == 0.15
assert args.max_deep_mixed_token_p99_regression_ratio == 0.20
assert args.host_memory_pressure_mib == 1024
assert case["sweep_kv_gpu_layers"] == [4]
assert case["prefetch_window_tokens"] == 32
assert case["max_queued_pages"] == 32
assert case["native_page_streaming_flatten_flash"] is True
assert case["attention_page_segments_live_offloaded_only"] is True
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn llama_cpp_adapter_bootstrap_dry_run_pins_patch_audit_and_targets() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "qatq-llama.cpp"
    resolved_work_dir = str(work_dir.resolve())
    report = Path(raw_tmp) / "bootstrap.json"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_adapter_bootstrap.py",
            "--work-dir",
            str(work_dir),
            "--target",
            "llama-simple",
            "--target",
            "llama-server",
            "--jobs",
            "3",
            "--output",
            str(report),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    data = json.loads(report.read_text())
    commands = [entry["argv"] for entry in data["commands"]]
    flat = [" ".join(command) for command in commands]

    assert data["format"] == "qatq-llama-cpp-adapter-bootstrap-v1"
    assert data["dry_run"] is True
    assert data["commit"] == "7992aa7c8e21ea2eb7a5e4802da56eec7b376036"
    assert data["targets"] == ["llama-simple", "llama-server"]
    assert data["skip_build"] is False
    assert data["skip_audit"] is False
    assert commands[0][:3] == ["git", "clone", "--filter=blob:none"]
    assert any(command[:5] == ["git", "-C", resolved_work_dir, "fetch", "--depth"] for command in commands)
    assert any("git -C" in command and "checkout --detach 7992aa7c8e21ea2eb7a5e4802da56eec7b376036" in command for command in flat)
    assert any(command[:4] == ["git", "-C", resolved_work_dir, "apply"] and "--check" in command for command in commands)
    assert any(command[:4] == ["git", "-C", resolved_work_dir, "apply"] and "--check" not in command for command in commands)
    assert any(
        "llama_cpp_live_vram_adapter_audit.py" in command
        and "--require-live-paging" in command
        and "--require-runtime-security" in command
        for command in flat
    )
    assert any(command[:3] == ["cmake", "-S", resolved_work_dir] for command in commands)
    assert any(command[:4] == ["cmake", "--build", str((work_dir / "build-qatq").resolve()), "--target"] and command[4] == "llama-simple" for command in commands)
    assert any(command[:4] == ["cmake", "--build", str((work_dir / "build-qatq").resolve()), "--target"] and command[4] == "llama-server" for command in commands)
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn llama_cpp_adapter_bootstrap_dry_run_force_records_destructive_step() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "existing-non-git"
    work_dir.mkdir()
    report = Path(raw_tmp) / "bootstrap.json"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_adapter_bootstrap.py",
            "--work-dir",
            str(work_dir),
            "--skip-build",
            "--skip-audit",
            "--force",
            "--dry-run",
            "--output",
            str(report),
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    data = json.loads(report.read_text())
    commands = [entry["argv"] for entry in data["commands"]]
    assert commands[0] == ["rm", "-rf", str(work_dir.resolve())]
    assert commands[1][:3] == ["git", "clone", "--filter=blob:none"]
    assert data["skip_build"] is True
    assert data["skip_audit"] is True
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_matrix_rejects_unknown_config_gate_names() {
    let output = run_python_snippet(
        r#"
import argparse, importlib.util, sys

spec = importlib.util.spec_from_file_location("matrix", "scripts/llama_cpp_live_vram_matrix.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

args = argparse.Namespace(
    require_stable_reclaim=False,
    require_stable_qc_bytes=False,
    max_elapsed_jitter_ratio=0.0,
    deep_latency_baseline=False,
    min_token_latency_samples=0,
    max_mixed_token_p95_regression_ratio=0.0,
    max_mixed_token_p99_regression_ratio=0.0,
    min_deep_token_latency_samples=0,
    max_deep_mixed_token_p95_regression_ratio=0.0,
    max_deep_mixed_token_p99_regression_ratio=0.0,
    host_memory_pressure_mib=0,
)
try:
    module.apply_config_gates(args, {"matrix_gates": {"min_deep_token_latency_sampels": 32}})
except SystemExit as exc:
    assert "unknown keys" in str(exc)
else:
    raise AssertionError("unknown matrix gate was accepted")
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_matrix_passes_explicit_llama_cpp_source_to_evidence_runner() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys, tempfile
from pathlib import Path
from types import SimpleNamespace

spec = importlib.util.spec_from_file_location("matrix", "scripts/llama_cpp_live_vram_matrix.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

captured = {}

def fake_run(command, cwd, text, capture_output, timeout):
    captured["command"] = command
    return SimpleNamespace(returncode=1, stdout="", stderr="synthetic failure")

module.subprocess.run = fake_run

with tempfile.TemporaryDirectory() as raw_tmp:
    result = module.run_case(
        root=Path.cwd(),
        runner=Path("scripts/llama_cpp_live_vram_evidence.py"),
        llama_simple=Path("/tmp/qatq-llama-bootstrap-proof/build-qatq/bin/llama-simple"),
        llama_cpp_source=Path("/tmp/qatq-llama-bootstrap-proof"),
        kv_bench="target/release/qatq-kv-bench",
        matrix_work_dir=Path(raw_tmp),
        timeout=10,
        skip_event_trace=False,
        skip_attention_trace=False,
        skip_attention_page_segments_trace=False,
        require_live_paging=True,
        require_native_page_streaming=True,
        native_page_streaming_contract_probe=False,
        gpu_page_staging=True,
        native_page_streaming_attention=False,
        native_page_streaming_attention_ggml=False,
        native_page_streaming_attention_backend_op=True,
        native_page_streaming_flatten_flash=True,
        attention_page_segments_live_offloaded_only=True,
        aggregate_codec_gate=True,
        prune_bulk_artifacts=True,
        deep_latency_baseline=False,
        override_short_predict=0,
        override_deep_predict=0,
        override_max_queued_pages=-1,
        case={
            "id": "source-pass-through",
            "model": "/tmp/model.gguf",
            "model_id": "model",
            "sweep_kv_gpu_layers": [1],
            "short_prompt": "short",
            "deep_prompt_seed": "deep",
        },
        iteration=1,
    )

assert result.status == "fail"
command = captured["command"]
source_flag_index = command.index("--llama-cpp-source")
assert command[source_flag_index + 1] == "/tmp/qatq-llama-bootstrap-proof"
assert "--require-native-page-streaming" in command
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_matrix_layer_memory_breadth_config_is_strict_selected_layer_shape() {
    let output = run_python_snippet(
        r#"
import json
from pathlib import Path

config = json.loads(Path("adapters/llama-cpp/live-vram-native-layer-memory-breadth.local.example.json").read_text())
cases = config["cases"]
assert [case["id"] for case in cases] == [
    "qwen25-15b-layer-memory-breadth-512p",
    "qwen25-3b-layer-memory-breadth-512p",
    "phi35-mini-layer-memory-breadth-512p",
]
for case in cases:
    assert case["sweep_kv_gpu_layers"] == [1, 2, 4]
    assert case["page_tokens"] == 512
    assert case["current_token"] == 512
    assert case["hot_window_tokens"] == 0
    assert case["next_required"] == "page-end"
    assert case["gpu_page_staging"] is True
    assert case["native_page_streaming_attention_backend_op"] is True
    assert case["native_page_streaming_flatten_flash"] is True
    assert case["aggregate_codec_gate"] is True
    assert case["skip_cpu_kv_baseline"] is True
    assert case["skip_attention_page_tensor_self_test"] is True
    assert case["live_restore_slot_pressure_max_bytes"] == 1
    assert case["mlx_streaming_attention_gate"] is True
    assert case["mlx_qatq_bin"] == "target/release/qatq"
assert cases[0]["mlx_min_layers_checked"] == 28
assert cases[0]["mlx_min_heads_checked"] == 336
assert cases[1]["mlx_min_layers_checked"] == 36
assert cases[1]["mlx_min_heads_checked"] == 576
assert cases[2]["mlx_min_layers_checked"] == 32
assert cases[2]["mlx_min_heads_checked"] == 1024
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_matrix_page_size_config_uses_current_backend_op_route() {
    let output = run_python_snippet(
        r#"
import json
from pathlib import Path

config = json.loads(Path("adapters/llama-cpp/live-vram-native-page-size.local.example.json").read_text())
cases = config["cases"]
assert [case["id"] for case in cases] == [
    "qwen25-15b-page256-deep",
    "qwen25-15b-page2048-deep",
    "qwen25-coder-3b-page256-deep",
    "qwen25-coder-3b-page2048-deep",
    "qwen25-3b-page1024-deep",
]
for case in cases:
    assert case["gpu_page_staging"] is True
    assert case["native_page_streaming_attention_ggml"] is True
    assert case["native_page_streaming_attention_backend_op"] is True
    assert case["native_page_streaming_flatten_flash"] is True
    assert case["aggregate_codec_gate"] is True
    assert case["skip_cpu_kv_baseline"] is True
    assert case["skip_attention_page_tensor_self_test"] is True
    assert case["live_restore_slot_pressure_max_bytes"] == 1
    assert case["mlx_streaming_attention_gate"] is True
assert cases[0]["sweep_kv_gpu_layers"] == [1, 2, 4]
assert cases[0]["attention_page_segments_live_offloaded_only"] is True
assert cases[1]["page_tokens"] == 2048
assert cases[1]["current_token"] == 2048
assert cases[1]["attention_page_segments_live_offloaded_only"] is False
assert cases[2]["sweep_kv_gpu_layers"] == [1, 2, 4]
assert cases[2]["attention_page_segments_live_offloaded_only"] is True
assert cases[3]["page_tokens"] == 2048
assert cases[3]["current_token"] == 2048
assert cases[3]["attention_page_segments_live_offloaded_only"] is False
assert cases[4]["page_tokens"] == 1024
assert cases[4]["current_token"] == 1024
assert cases[4]["attention_page_segments_live_offloaded_only"] is True
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_matrix_phi_page_size_config_is_strict_compact_native_route() {
    let output = run_python_snippet(
        r#"
import json
from pathlib import Path

config = json.loads(Path("adapters/llama-cpp/live-vram-native-phi-page-size.local.example.json").read_text())
cases = config["cases"]
assert [case["id"] for case in cases] == [
    "phi35-mini-page256-deep",
    "phi35-mini-page512-deep",
    "phi35-mini-page1024-deep",
]
assert [case["page_tokens"] for case in cases] == [256, 512, 1024]
assert [case["current_token"] for case in cases] == [512, 512, 1024]
for case in cases:
    assert case["model"].endswith("Phi-3.5-mini-instruct-Q4_K_M.gguf")
    assert case["sweep_kv_gpu_layers"] == [1, 2, 4]
    assert case["gpu_page_staging"] is True
    assert case["native_page_streaming_attention_backend_op"] is True
    assert case["native_page_streaming_flatten_flash"] is True
    assert case["aggregate_codec_gate"] is True
    assert case["skip_cpu_kv_baseline"] is True
    assert case["skip_attention_page_tensor_self_test"] is True
    assert case["live_restore_slot_pressure_max_bytes"] == 1
    assert case["attention_fixture_max_layers"] == 32
    assert case["mlx_streaming_attention_gate"] is True
    assert case["mlx_min_layers_checked"] == 32
    assert case["mlx_min_heads_checked"] == 1024
assert cases[0]["attention_page_segments_live_offloaded_only"] is True
assert cases[1]["attention_page_segments_live_offloaded_only"] is True
assert cases[2]["attention_page_segments_live_offloaded_only"] is False
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_matrix_latency_stats_ignore_batched_setup_rows() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys, tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("matrix", "scripts/llama_cpp_live_vram_matrix.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

with tempfile.TemporaryDirectory() as raw_tmp:
    path = Path(raw_tmp) / "tokens.csv"
    path.write_text(
        "row_type,run_name,batch_tokens,decode_us,generated_token\n"
        "decode-token,deep-full-gpu,20,10000000,1\n"
        "decode-token,deep-mixed-kv,20,12000000,1\n"
        "decode-token,deep-full-gpu,1,100,2\n"
        "decode-token,deep-full-gpu,1,110,3\n"
        "decode-token,deep-mixed-kv,1,105,2\n"
        "decode-token,deep-mixed-kv,1,120,3\n",
        encoding="utf-8",
    )
    stats = module.parse_token_latency_stats(
        path,
        baseline_run="deep-full-gpu",
        candidate_run="deep-mixed-kv",
    )
    assert stats.full_samples == 2
    assert stats.mixed_samples == 2
    assert stats.full_p95 == 110
    assert stats.mixed_p95 == 120
    assert stats.p95_regression < 0.10
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_adapter_audit_accepts_retained_page_table_backend_path() {
    let output = run_python_snippet(
        r#"
import importlib.util, json, sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("audit", "scripts/llama_cpp_live_vram_adapter_audit.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

files = module.read_files_from_patch(Path("adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch"))
checks = module.build_checks(files)
by_name = {check.name: check.passed for check in checks}
assert by_name["live.native_backend_op_avoids_full_kv_packing"] is True
assert by_name["live.native_backend_op_multi_page_streaming"] is True
assert by_name["live.native_backend_op_token_page_table"] is True
assert by_name["live.native_backend_op_consumes_staged_segments"] is True
assert by_name["live.native_backend_op_avoids_staged_arena"] is True
assert by_name["live.native_backend_op_descriptor_path"] is True
assert by_name["live.native_backend_op_transient_pool_byte_budget"] is True
assert by_name["live.native_flattened_flash_attention_route"] is True
assert by_name["live.native_flattened_flash_stream_local_contract"] is True
assert module.page_staging_ready(checks) is True
assert module.native_ggml_live_ready(checks) is True
assert module.runtime_security_ready(checks) is True
assert module.failed_required_checks(checks, module.LIVE_PAGING_REQUIRED_CHECKS) == []
assert module.failed_required_checks(checks, module.PAGE_STAGING_REQUIRED_CHECKS) == []
assert module.failed_required_checks(checks, module.RUNTIME_SECURITY_REQUIRED_CHECKS) == []
non_required = set(module.non_required_failures(checks))
assert "live.attention_persistent_page_source_path" in non_required
assert "live.no_concat_composed_attention_source" in non_required
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_adapter_audit_runtime_security_gate_fails_without_restore_slot_rejection() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("audit", "scripts/llama_cpp_live_vram_adapter_audit.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

files = module.read_files_from_patch(Path("adapters/llama-cpp/qatq-kv-export-7992aa7c8.patch"))
files["kv"] = files["kv"].replace("QATQ restore slot pressure self-test rejected", "")
checks = module.build_checks(files)
assert module.runtime_security_ready(checks) is False
failures = module.failed_required_checks(checks, module.RUNTIME_SECURITY_REQUIRED_CHECKS)
assert "live.restore_slot_pressure_self_test" in failures
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_evidence_stage_status_records_latest_stage_and_events() {
    let output = run_python_snippet(
        r#"
import importlib.util, json, sys, tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("evidence", "scripts/llama_cpp_live_vram_evidence.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp)
    module.write_stage_status(
        work_dir,
        "deep-mixed-kv-l18",
        "running",
        command="llama-simple ...",
        log=str(work_dir / "deep.log"),
    )
    module.write_stage_status(
        work_dir,
        "deep-mixed-kv-l18",
        "timeout",
        timeout_seconds=3,
        artifacts={"output_manifest": {"path": "out.json", "exists": False}},
    )
    status = json.loads((work_dir / "stage-status.json").read_text())
    events = (work_dir / "stage-events.jsonl").read_text().strip().splitlines()
    assert status["format"] == "qatq-live-vram-stage-status-v1"
    latest = status["stages"]["deep-mixed-kv-l18"]
    assert latest["status"] == "timeout"
    assert latest["timeout_seconds"] == 3
    assert len(events) == 2
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_matrix_augments_failures_with_latest_stage_status() {
    let output = run_python_snippet(
        r#"
import importlib.util, json, sys, tempfile, time
from pathlib import Path

spec = importlib.util.spec_from_file_location("matrix", "scripts/llama_cpp_live_vram_matrix.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp)
    status = {
        "format": "qatq-live-vram-stage-status-v1",
        "updated_at_unix": time.time(),
        "stages": {
            "short-full-gpu": {
                "stage": "short-full-gpu",
                "status": "pass",
                "timestamp_unix": time.time() - 10,
                "log": str(work_dir / "short.log"),
            },
            "deep-mixed-kv-l18": {
                "stage": "deep-mixed-kv-l18",
                "status": "timeout",
                "timestamp_unix": time.time(),
                "log": str(work_dir / "deep.log"),
            },
        },
    }
    (work_dir / "stage-status.json").write_text(json.dumps(status))
    failure = module.augment_failure_with_stage_status(work_dir, "runner failed")
    assert "runner failed" in failure
    assert "latest_stage=deep-mixed-kv-l18" in failure
    assert "latest_stage_status=timeout" in failure
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_parallel_stress_dry_run_shards_cases_and_iterations() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    tmp = Path(raw_tmp)
    config = tmp / "config.json"
    work = tmp / "work"
    config.write_text(json.dumps({
        "matrix_gates": {"require_stable_reclaim": True},
        "cases": [
            {
                "id": "case-a",
                "model": "/tmp/model-a.gguf",
                "model_id": "model-a",
                "sweep_kv_gpu_layers": [1],
                "short_prompt": "a",
                "deep_prompt_seed": "aa"
            },
            {
                "id": "case-b",
                "model": "/tmp/model-b.gguf",
                "model_id": "model-b",
                "sweep_kv_gpu_layers": [2],
                "short_prompt": "b",
                "deep_prompt_seed": "bb"
            }
        ]
    }), encoding="utf-8")
    completed = subprocess.run([
        sys.executable,
        "scripts/llama_cpp_live_vram_parallel_stress.py",
        "--config", str(config),
        "--matrix-runner", "scripts/llama_cpp_live_vram_matrix.py",
        "--llama-simple", "/tmp/not-needed-in-dry-run",
        "--work-dir", str(work),
        "--dry-run",
        "--jobs", "2",
        "--iterations", "2",
        "--require-live-paging",
        "--require-native-page-streaming",
        "--native-page-streaming-attention-backend-op",
        "--aggregate-codec-gate",
        "--prune-bulk-artifacts",
        "--override-short-predict", "3"
    ], text=True, capture_output=True)
    assert completed.returncode == 0, completed.stderr
    plan = json.loads((work / "parallel-stress-plan.json").read_text())
    assert plan["format"] == "qatq-live-vram-parallel-stress-plan-v1"
    assert len(plan["jobs"]) == 4
    ids = [job["case_id"] for job in plan["jobs"]]
    assert ids == ["case-a", "case-b", "case-a", "case-b"]
    for job in plan["jobs"]:
        command = job["command"]
        assert "--require-live-paging" in command
        assert "--require-native-page-streaming" in command
        assert "--native-page-streaming-attention-backend-op" in command
        assert "--aggregate-codec-gate" in command
        assert "--prune-bulk-artifacts" in command
        assert "--override-short-predict" in command
        assert len(json.loads(Path(job["config"]).read_text())["cases"]) == 1
    summary = json.loads((work / "summary.json").read_text())
    assert summary["dry_run"] is True
    assert summary["total_jobs"] == 4
    assert summary["failed"] == 0
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_burnin_dry_run_repeats_matrix_and_summarises_runs() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-burnin"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_burnin.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-layer-policy-notrace.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--runs",
            "2",
            "--timeout",
            "30",
            "--run-timeout",
            "120",
            "--max-rss-growth-jitter-ratio",
            "1.5",
            "--max-backend-kv-jitter-ratio",
            "1.1",
            "--max-projected-device-jitter-ratio",
            "1.1",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-burnin-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert plan["format"] == "qatq-live-vram-server-burnin-plan-v1"
    assert plan["runs"] == 2
    assert plan["timeout_seconds"] == 30
    assert plan["run_timeout_seconds"] == 120
    assert summary["format"] == "qatq-live-vram-server-burnin-summary-v1"
    assert summary["status"] == "dry-run"
    assert summary["runs_completed"] == 2
    assert summary["dry_run_runs"] == 2
    assert summary["aggregate_gate_failures"] == []
    assert summary["gates"]["max_rss_growth_jitter_ratio"] == 1.5
    for index in (1, 2):
        run_summary = work_dir / f"run-{index:03d}" / "summary.json"
        assert run_summary.exists(), run_summary
        data = json.loads(run_summary.read_text())
        assert data["status"] == "dry-run"
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_burnin_aggregate_gates_fail_closed() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("server_burnin", "scripts/llama_cpp_live_vram_server_burnin.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

class Args:
    max_rss_growth_jitter_ratio = 1.1
    max_backend_kv_jitter_ratio = 1.05
    max_projected_device_jitter_ratio = 1.05

runs = [
    module.BurnInRun(
        index=1,
        status="pass",
        returncode=0,
        elapsed_seconds=1.0,
        work_dir=Path("/tmp/run1"),
        summary_path=Path("/tmp/run1/summary.json"),
        stdout_path=Path("/tmp/run1/stdout"),
        stderr_path=Path("/tmp/run1/stderr"),
        failure="",
        summary={
            "cases": [
                {
                    "id": "case-a",
                    "rss_growth_kib": 100,
                    "rss_tail_growth_kib": 10,
                    "backend_accelerator_context_mib": 200,
                    "projected_device_memory_mib": 1000,
                }
            ]
        },
    ),
    module.BurnInRun(
        index=2,
        status="pass",
        returncode=0,
        elapsed_seconds=1.0,
        work_dir=Path("/tmp/run2"),
        summary_path=Path("/tmp/run2/summary.json"),
        stdout_path=Path("/tmp/run2/stdout"),
        stderr_path=Path("/tmp/run2/stderr"),
        failure="",
        summary={
            "cases": [
                {
                    "id": "case-a",
                    "rss_growth_kib": 150,
                    "rss_tail_growth_kib": 12,
                    "backend_accelerator_context_mib": 220,
                    "projected_device_memory_mib": 1001,
                }
            ]
        },
    ),
]
aggregate = module.aggregate_case_metrics(runs)
assert aggregate["case-a"]["rss_growth_kib"]["jitter_ratio"] == 1.5
assert aggregate["case-a"]["rss_tail_growth_kib"]["jitter_ratio"] == 1.2
failures = "\n".join(module.evaluate_aggregate_gates(Args(), aggregate))
assert "case-a: rss_growth_kib jitter ratio exceeded: 1.5 > 1.1" in failures
assert "case-a: backend_accelerator_context_mib jitter ratio exceeded: 1.1 > 1.05" in failures
assert "projected_device_memory_mib" not in failures
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_hardware_counter_report_separates_backend_diagnostics_from_peak_vram() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    tmp = Path(raw_tmp)
    matrix_summary = tmp / "summary.json"
    output = tmp / "hardware.json"
    matrix_summary.write_text(json.dumps({
        "status": "pass",
        "cases": [
            {
                "id": "native",
                "projected_device_memory_mib": 1458,
                "backend_memory": {
                    "memory_breakdown_mib": {
                        "MTL0 (Apple M4)": {"self": 1458, "context": 224, "compute": 299},
                        "Host": {"self": 196},
                    }
                },
            }
        ],
    }), encoding="utf-8")
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_hardware_counters.py",
            "--matrix-summary",
            str(matrix_summary),
            "--output",
            str(output),
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    report = json.loads(output.read_text())
    assert report["format"] == "qatq-live-vram-hardware-counter-capability-v1"
    assert report["backend_memory_diagnostics"]["present"] is True
    assert report["backend_memory_diagnostics"]["cases_with_projected_device_memory"] == 1
    assert report["backend_memory_diagnostics"]["cases_with_accelerator_breakdown"] == 1
    assert report["backend_memory_diagnostics"]["direct_peak_vram_counter"] is False
    assert report["direct_peak_vram_counter"]["available"] is False
    assert "Backend projected memory and RSS gates are not treated as direct peak-VRAM proof" in report["boundary"]
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_hardware_counter_report_fails_when_direct_peak_vram_required() {
    let output = run_python_snippet(
        r#"
import subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    output = Path(raw_tmp) / "hardware.json"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_hardware_counters.py",
            "--output",
            str(output),
            "--require-direct-peak-vram",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 1
    assert output.exists()
    assert "direct peak" not in result.stderr.lower()
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_hardware_counter_parses_nvidia_smi_process_memory() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys

spec = importlib.util.spec_from_file_location("hardware", "scripts/llama_cpp_live_vram_hardware_counters.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

assert module.parse_nvidia_smi_process_memory("4242, 128\n7, 999\n4242, 256 MiB\nbad\n", 4242) == [128, 256]
assert module.parse_nvidia_smi_process_memory("7, 999\n", 4242) == []
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_hardware_counter_report_accepts_sampled_nvidia_smi_peak_vram() {
    let output = run_python_snippet(
        r#"
import json, os, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    tmp = Path(raw_tmp)
    fake = tmp / "nvidia-smi"
    output = tmp / "hardware.json"
    fake.write_text("\n".join([
        '#!/usr/bin/env python3',
        'import sys',
        'if "--help-query-compute-apps" in sys.argv:',
        '    print("pid\\\\nused_memory")',
        '    raise SystemExit(0)',
        'if any(arg.startswith("--query-compute-apps") for arg in sys.argv):',
        '    print("4242, 128")',
        '    print("7, 999")',
        '    print("4242, 256")',
        '    raise SystemExit(0)',
        'raise SystemExit(2)',
    ]) + "\n", encoding="utf-8")
    fake.chmod(0o755)
    env = os.environ.copy()
    env["PATH"] = str(tmp) + os.pathsep + env.get("PATH", "")
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_hardware_counters.py",
            "--output",
            str(output),
            "--sample-pid",
            "4242",
            "--sample-seconds",
            "0",
            "--require-direct-peak-vram",
        ],
        check=False,
        text=True,
        capture_output=True,
        env=env,
    )
    assert result.returncode == 0, result.stderr
    report = json.loads(output.read_text())
    assert report["direct_peak_vram_counter"]["available"] is True
    source = report["direct_peak_vram_counter"]["sources"][0]
    assert source["backend"] == "nvidia-smi"
    assert source["sample_pid"] == 4242
    assert source["peak_memory_mib"] == 256
    assert report["nvidia_smi"]["supports_process_gpu_memory"] is True
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_abort_probe_dry_run_records_fail_closed_artifacts() {
    let output = run_python_snippet(
        r#"
import json, subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "abort-probe"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_abort_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-simple",
            "/tmp/nonexistent-llama-simple",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "abort-probe-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert plan["format"] == "qatq-live-vram-abort-probe-plan-v1"
    assert summary["status"] == "dry-run"
    command = plan["command"]
    assert "--qatq-kv-export-dir" in command
    assert "--qatq-output-manifest" in command
    assert "--qatq-token-timings" in command
    assert "--qatq-event-trace" in command
    assert "--qatq-native-page-streaming-attention-backend-op" in command
    assert "--qatq-native-page-streaming-flatten-flash" in command
    artifacts = plan["artifacts"]
    assert artifacts["output_manifest"].endswith("output-manifest.json")
    assert artifacts["token_timings"].endswith("token-timings.csv")
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_dry_run_records_qatq_server_contract() {
    let output = run_python_snippet(
        r#"
import json, subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-probe"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--host-memory-pressure-mib",
            "256",
            "--max-server-rss-growth-mib",
            "512",
            "--max-iteration-seconds",
            "15",
            "--max-followup-seconds",
            "10",
            "--require-flattened-flash-consumer",
            "--require-live-offloaded-stream-count",
            "2",
            "--require-backend-memory-diagnostics",
            "--sample-direct-peak-vram",
            "--require-direct-peak-vram-counter",
            "--direct-peak-vram-sample-interval-ms",
            "250",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-probe-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert plan["format"] == "qatq-live-vram-server-cancel-probe-plan-v1"
    assert plan["mode"] == "qatq-live-vram"
    assert summary["status"] == "dry-run"
    assert summary["mode"] == "qatq-live-vram"
    command = plan["command"]
    assert "--host" in command
    assert "--port" in command
    assert command[command.index("--port") + 1] == "0"
    assert "-np" in command
    assert command[command.index("-np") + 1] == "1"
    assert "--slots" in command
    assert "--flash-attn" in command
    env = plan["env"]
    assert env["LLAMA_QATQ_GPU_PAGE_STAGING"] == "1"
    assert env["LLAMA_QATQ_NATIVE_PAGE_STREAMING_ATTENTION"] == "1"
    assert env["LLAMA_QATQ_NATIVE_PAGE_STREAMING_ATTENTION_BACKEND"] == "backend-op"
    assert env["LLAMA_QATQ_NATIVE_PAGE_STREAMING_FLATTEN_FLASH"] == "1"
    assert env["LLAMA_QATQ_TRACE_MAX_QUEUED_PAGES"] == "32"
    assert env["LLAMA_QATQ_ATTENTION_PAGE_SEGMENTS_MAX_PAGES"] == "4"
    assert env["LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_SOURCE_PAGES"] == "4"
    assert env["LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_RETAINED_BYTES"] == "1073741824"
    assert env["LLAMA_QATQ_GRAPH_EXTRA_NODES"] == "32768"
    assert plan["derived"]["derived_page_segments"] == 4
    assert plan["derived"]["max_page_segments"] == 4
    assert plan["derived"]["graph_extra_nodes"] == 32768
    assert plan["derived"]["max_retained_page_pool_mib"] == 1024
    assert plan["derived"]["max_retained_page_pool_bytes"] == 1073741824
    assert plan["iterations"] == 1
    assert plan["warmup_iterations"] == 0
    assert plan["host_memory_pressure_mib"] == 256
    assert plan["max_server_rss_growth_mib"] == 512
    assert plan["max_rss_tail_growth_kib"] == 0
    assert plan["rss_tail_window"] == 4
    assert plan["max_retained_page_pool_mib"] == 1024
    assert plan["max_iteration_seconds"] == 15.0
    assert plan["max_followup_seconds"] == 10.0
    assert plan["require_flattened_flash_consumer"] is True
    assert plan["require_live_offloaded_stream_count"] == 2
    assert plan["require_backend_memory_diagnostics"] is True
    assert plan["sample_direct_peak_vram"] is True
    assert plan["require_direct_peak_vram_counter"] is True
    assert plan["direct_peak_vram_sample_interval_ms"] == 250
    assert summary["iterations"] == 1
    assert summary["warmup_iterations"] == 0
    assert summary["host_memory_pressure_mib"] == 256
    assert summary["max_server_rss_growth_mib"] == 512
    assert summary["max_rss_tail_growth_kib"] == 0
    assert summary["rss_tail_window"] == 4
    assert summary["max_retained_page_pool_mib"] == 1024
    assert summary["max_iteration_seconds"] == 15.0
    assert summary["max_followup_seconds"] == 10.0
    assert summary["require_flattened_flash_consumer"] is True
    assert summary["require_live_offloaded_stream_count"] == 2
    assert summary["require_backend_memory_diagnostics"] is True
    assert summary["sample_direct_peak_vram"] is True
    assert summary["require_direct_peak_vram_counter"] is True
    assert summary["direct_peak_vram_sample_interval_ms"] == 250
    artifacts = plan["artifacts"]
    assert artifacts["event_trace"].endswith("event-trace.jsonl")
    assert artifacts["page_segments"].endswith("page-segments.jsonl")
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_rejects_required_direct_peak_vram_without_sampling() {
    let output = run_python_snippet(
        r#"
import subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-probe"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--require-direct-peak-vram-counter",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 1
    assert "--require-direct-peak-vram-counter requires --sample-direct-peak-vram" in result.stderr
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_dry_run_records_native_baseline_mode() {
    let output = run_python_snippet(
        r#"
import json, subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-native-baseline"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--native-baseline",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-probe-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert plan["mode"] == "native-baseline"
    assert summary["mode"] == "native-baseline"
    assert plan["env"] == {}
    assert summary["env"] == {}
    assert plan["artifacts"]["page_segments"].endswith("page-segments.jsonl")
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_dry_run_can_disable_qatq_traces_for_perf() {
    let output = run_python_snippet(
        r#"
import json, subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-no-trace"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--disable-qatq-traces",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-probe-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    env = plan["env"]
    assert plan["mode"] == "qatq-live-vram"
    assert plan["qatq_traces_enabled"] is False
    assert summary["qatq_traces_enabled"] is False
    assert env["LLAMA_QATQ_GPU_PAGE_STAGING"] == "1"
    assert env["LLAMA_QATQ_NATIVE_PAGE_STREAMING_ATTENTION"] == "1"
    assert "LLAMA_QATQ_ATTENTION_PAGE_SEGMENTS_TRACE" not in env
    assert "LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_TRACE" not in env
    assert "LLAMA_QATQ_LIVE_PERSISTENT_PAGE_POOL_TRACE" not in env
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_extracts_llama_server_timing_metrics() {
    let output = run_python_snippet(
        r#"
import importlib.util, json, sys

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

payload = json.dumps({
    "content": "healthy",
    "timings": {
        "prompt_n": 42,
        "prompt_ms": 120.5,
        "prompt_per_second": 348.5477,
        "predicted_n": 8,
        "predicted_ms": 95.25,
        "predicted_per_second": 83.9895,
    },
}).encode("utf-8")
metrics = module.extract_completion_metrics(payload)
assert metrics["prompt_tokens"] == 42
assert metrics["predicted_tokens"] == 8
assert metrics["prompt_ms"] == 120.5
assert metrics["predicted_ms"] == 95.25
assert metrics["prompt_per_second"] == 348.5477
assert metrics["predicted_per_second"] == 83.9895
assert module.extract_completion_metrics(b"not-json") == {}
summary = module.aggregate_followup_completion_metrics([
    {"followup_metrics": {"predicted_per_second": 80.0, "predicted_tokens": 8}},
    {"followup_metrics": {"predicted_per_second": 100.0, "predicted_tokens": 8}},
    {"followup_metrics": {"ignored": "not numeric"}},
])
assert summary["predicted_per_second"]["count"] == 2
assert summary["predicted_per_second"]["p05"] == 80.0
assert summary["predicted_per_second"]["p50"] == 80.0
assert summary["predicted_per_second"]["p95"] == 100.0
assert summary["predicted_tokens"]["count"] == 2
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_rejects_native_baseline_trace_gates() {
    let output = run_python_snippet(
        r#"
import subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-native-baseline"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--native-baseline",
            "--require-flattened-flash-consumer",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode != 0
    assert "native-baseline" in result.stderr or "native-baseline" in result.stdout
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_rejects_no_trace_with_trace_gates() {
    let output = run_python_snippet(
        r#"
import subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-no-trace"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--disable-qatq-traces",
            "--require-live-offloaded-stream-count",
            "2",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode != 0
    assert "disable-qatq-traces" in result.stderr or "disable-qatq-traces" in result.stdout
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_strict_cases() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-strict.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--max-cases",
            "2",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert plan["format"] == "qatq-live-vram-server-cancel-matrix-plan-v1"
    assert summary["format"] == "qatq-live-vram-server-cancel-matrix-summary-v1"
    assert summary["status"] == "dry-run"
    assert summary["total_cases"] == 2
    assert summary["dry_run_cases"] == 2
    assert summary["failed"] == 0
    first = plan["cases"][0]["command"]
    assert plan["cases"][0]["gates"]["max_backend_accelerator_self_mib"] == 1600
    assert plan["cases"][0]["gates"]["max_backend_accelerator_context_mib"] == 256
    assert plan["cases"][0]["gates"]["max_backend_accelerator_compute_mib"] == 360
    assert plan["cases"][0]["gates"]["max_projected_device_memory_mib"] == 1600
    assert "--parallel-slots" in first
    assert first[first.index("--parallel-slots") + 1] == "2"
    assert "--page-tokens" in first
    assert first[first.index("--page-tokens") + 1] == "64"
    assert "--concurrent-followup-during-cancel" in first
    assert "--require-flattened-flash-consumer" in first
    assert "--require-live-offloaded-stream-count" in first
    assert first[first.index("--require-live-offloaded-stream-count") + 1] == "2"
    assert "--require-backend-memory-diagnostics" in first
    assert "--keep-work-dir" in first
    assert "--dry-run" in first
    case_summary = json.loads((work_dir / "qwen25-15b-strict-server-cancel" / "summary.json").read_text())
    assert case_summary["status"] == "dry-run"
    assert summary["cases"][0]["gates"]["max_backend_accelerator_self_mib"] == 1600
    assert summary["cases"][0]["gate_failures"] == []
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_extended_cases() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-extended-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-extended.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert summary["status"] == "dry-run"
    assert summary["total_cases"] == 2

    long_context = plan["cases"][0]["command"]
    assert "--ctx-size" in long_context
    assert long_context[long_context.index("--ctx-size") + 1] == "16384"
    assert "--current-token" in long_context
    assert long_context[long_context.index("--current-token") + 1] == "8192"
    assert "--prompt-repeat" in long_context
    assert long_context[long_context.index("--prompt-repeat") + 1] == "160"
    assert "--require-flattened-flash-consumer" in long_context
    assert "--require-live-offloaded-stream-count" in long_context

    page_variant = plan["cases"][1]["command"]
    assert page_variant[page_variant.index("--page-tokens") + 1] == "128"
    assert page_variant[page_variant.index("--ctx-size") + 1] == "8192"
    assert page_variant[page_variant.index("--iterations") + 1] == "5"
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_mixed_prompt_cases() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-mixed-prompt-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-mixed-prompts.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert summary["status"] == "dry-run"
    assert summary["total_cases"] == 3

    prompts = []
    for case in plan["cases"]:
        command = case["command"]
        assert "--prompt" in command
        prompts.append(command[command.index("--prompt") + 1])
        assert command[command.index("--iterations") + 1] == "3"
        assert "--require-flattened-flash-consumer" in command
        assert "--require-live-offloaded-stream-count" in command
        assert "--require-backend-memory-diagnostics" in command
        assert case["gates"]["max_backend_accelerator_self_mib"] == 1600
        assert case["gates"]["max_backend_accelerator_context_mib"] == 256
        assert case["gates"]["max_backend_accelerator_compute_mib"] == 360
        assert case["gates"]["max_projected_device_memory_mib"] == 1600

    assert len(set(prompts)) == 3
    assert any("Daily-driver" in prompt for prompt in prompts)
    assert any("Software engineering" in prompt for prompt in prompts)
    assert any("Retrieval-heavy" in prompt for prompt in prompts)
    assert all(case["gate_failures"] == [] for case in summary["cases"])
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_mixed_model_prompt_cases() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-mixed-model-prompt-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-mixed-model-prompts.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert summary["status"] == "dry-run"
    assert summary["total_cases"] == 3

    expected = {
        "qwen25-15b-daily-driver-mixed-model": (1600, 256, 360, 1600, "Daily-driver"),
        "qwen25-3b-code-review-mixed-model": (2600, 320, 360, 2600, "Software engineering"),
        "phi35-mini-ops-incident-mixed-model": (5600, 3200, 180, 5600, "Operations incident"),
    }
    for case in plan["cases"]:
        case_id = case["id"]
        assert case_id in expected
        self_mib, context_mib, compute_mib, projected_mib, marker = expected[case_id]
        command = case["command"]
        assert command[command.index("--iterations") + 1] == "3"
        assert marker in command[command.index("--prompt") + 1]
        assert "--require-flattened-flash-consumer" in command
        assert "--require-live-offloaded-stream-count" in command
        assert "--require-backend-memory-diagnostics" in command
        assert case["gates"]["max_backend_accelerator_self_mib"] == self_mib
        assert case["gates"]["max_backend_accelerator_context_mib"] == context_mib
        assert case["gates"]["max_backend_accelerator_compute_mib"] == compute_mib
        assert case["gates"]["max_projected_device_memory_mib"] == projected_mib
    phi = next(case for case in plan["cases"] if case["id"] == "phi35-mini-ops-incident-mixed-model")
    assert phi["command"][phi["command"].index("--page-tokens") + 1] == "128"
    assert all(case["gate_failures"] == [] for case in summary["cases"])
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_mixed_model_soak_cases() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-mixed-model-soak-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-mixed-model-soak.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert summary["status"] == "dry-run"
    assert summary["total_cases"] == 3

    expected = {
        "qwen25-15b-daily-driver-mixed-model-soak": (1600, 256, 360, 1600, "Daily-driver"),
        "qwen25-3b-code-review-mixed-model-soak": (2600, 320, 360, 2600, "Software engineering"),
        "phi35-mini-ops-incident-mixed-model-soak": (5600, 3200, 180, 5600, "Operations incident"),
    }
    for case in plan["cases"]:
        case_id = case["id"]
        assert case_id in expected
        self_mib, context_mib, compute_mib, projected_mib, marker = expected[case_id]
        command = case["command"]
        assert command[command.index("--iterations") + 1] == "10"
        assert marker in command[command.index("--prompt") + 1]
        assert "--require-flattened-flash-consumer" in command
        assert "--require-live-offloaded-stream-count" in command
        assert "--require-backend-memory-diagnostics" in command
        assert case["gates"]["max_backend_accelerator_self_mib"] == self_mib
        assert case["gates"]["max_backend_accelerator_context_mib"] == context_mib
        assert case["gates"]["max_backend_accelerator_compute_mib"] == compute_mib
        assert case["gates"]["max_projected_device_memory_mib"] == projected_mib
    phi = next(case for case in plan["cases"] if case["id"] == "phi35-mini-ops-incident-mixed-model-soak")
    assert phi["command"][phi["command"].index("--page-tokens") + 1] == "128"
    assert all(case["gate_failures"] == [] for case in summary["cases"])
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_baseline_comparison() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-baseline-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-baseline.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert summary["status"] == "dry-run"
    assert summary["total_cases"] == 6
    assert len(summary["comparisons"]) == 3
    native = plan["cases"][0]["command"]
    qatq = plan["cases"][1]["command"]
    assert plan["cases"][0]["comparison_group"] == "qwen25-15b"
    assert plan["cases"][1]["comparison_group"] == "qwen25-15b"
    assert "--native-baseline" in native
    assert "--require-flattened-flash-consumer" not in native
    assert "--native-baseline" not in qatq
    assert "--require-flattened-flash-consumer" in qatq
    assert "--require-live-offloaded-stream-count" in qatq
    assert summary["cases"][0]["mode"] == "native-baseline"
    assert summary["cases"][1]["mode"] == "qatq-live-vram"
    assert summary["cases"][0]["comparison_group"] == "qwen25-15b"
    assert summary["comparisons"][0]["status"] == "incomplete"
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_computes_native_ratios() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("server_matrix", "scripts/llama_cpp_live_vram_server_cancel_matrix.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

native = module.CaseResult(
    case_id="native",
    comparison_group="qwen",
    status="pass",
    returncode=0,
    elapsed_seconds=1,
    work_dir=Path("/tmp/native"),
    summary_path=Path("/tmp/native/summary.json"),
    stdout_path=Path("/tmp/native/stdout"),
    stderr_path=Path("/tmp/native/stderr"),
    failure="",
    probe_summary={
        "mode": "native-baseline",
        "latency_checks": {
            "iteration_duration_seconds": {"p95": 5.0},
            "followup_duration_seconds": {"p95": 2.0},
        },
        "memory_checks": {
            "growth_kib": 100,
            "rss_tail_growth_kib": 25,
            "rss_tail_gate_growth_kib": 25,
        },
        "followup_completion_metrics": {
            "predicted_per_second": {"p05": 70.0, "p50": 80.0, "p95": 90.0},
        },
        "checks": {"page_segment_counts": {}},
        "direct_peak_vram_counter": {
            "available": True,
            "backend": "nvidia-smi",
            "peak_memory_mib": 640,
        },
        "backend_memory": {
            "projected_device_memory_mib": 640,
            "memory_breakdown_mib": {
                "MTL0 (Apple M4)": {
                    "total": 18186,
                    "free": 17000,
                    "self": 640,
                    "model": 200,
                    "context": 320,
                    "compute": 56,
                    "unaccounted": 32,
                },
            },
        },
    },
    gates={},
    gate_failures=[],
)
qatq = module.CaseResult(
    case_id="qatq",
    comparison_group="qwen",
    status="pass",
    returncode=0,
    elapsed_seconds=1,
    work_dir=Path("/tmp/qatq"),
    summary_path=Path("/tmp/qatq/summary.json"),
    stdout_path=Path("/tmp/qatq/stdout"),
    stderr_path=Path("/tmp/qatq/stderr"),
    failure="",
    probe_summary={
        "mode": "qatq-live-vram",
        "latency_checks": {
            "iteration_duration_seconds": {"p95": 6.5},
            "followup_duration_seconds": {"p95": 3.0},
        },
        "memory_checks": {
            "growth_kib": 250,
            "rss_tail_growth_kib": 32,
            "rss_tail_gate_growth_kib": 32,
            "rss_tail_range_kib": 512,
        },
        "followup_completion_metrics": {
            "predicted_per_second": {"p05": 63.0, "p50": 60.0, "p95": 72.0},
        },
        "checks": {
            "page_segment_counts": {
                "live_offloaded_segments": 8,
                "consumer.backend_scheduled_flattened_flash_attention": 4,
            },
            "persistent_page_source_stats": {
                "max_retained_bytes": 1048576,
            },
        },
        "direct_peak_vram_counter": {
            "available": True,
            "backend": "nvidia-smi",
            "peak_memory_mib": 512,
        },
        "backend_memory": {
            "projected_device_memory_mib": 512,
            "memory_breakdown_mib": {
                "MTL0 (Apple M4)": {
                    "total": 18186,
                    "free": 17000,
                    "self": 512,
                    "model": 200,
                    "context": 256,
                    "compute": 56,
                    "unaccounted": 32,
                },
                "Host": {
                    "self": 96,
                    "model": 32,
                    "context": 32,
                    "compute": 32,
                },
            },
        },
    },
    gates={},
    gate_failures=[],
)
case_summary = module.summarise_case(qatq)
assert case_summary["max_retained_bytes"] == 1048576
assert case_summary["backend_accelerator"] == "MTL0 (Apple M4)"
assert case_summary["backend_accelerator_self_mib"] == 512
assert case_summary["backend_accelerator_context_mib"] == 256
assert case_summary["backend_accelerator_compute_mib"] == 56
assert case_summary["projected_device_memory_mib"] == 512
assert case_summary["direct_peak_vram_counter_available"] is True
assert case_summary["direct_peak_vram_mib"] == 512
assert case_summary["direct_peak_vram_backend"] == "nvidia-smi"
assert case_summary["rss_tail_growth_kib"] == 32
assert case_summary["rss_tail_gate_growth_kib"] == 32
assert case_summary["rss_tail_range_kib"] == 512
comparisons = module.build_comparisons([native, qatq])
assert comparisons[0]["comparison_group"] == "qwen"
assert comparisons[0]["status"] == "pass"
assert abs(comparisons[0]["iteration_p95_ratio"] - 1.3) < 1e-9
assert abs(comparisons[0]["followup_p95_ratio"] - 1.5) < 1e-9
assert comparisons[0]["predicted_per_second_p05_ratio"] == 0.9
assert comparisons[0]["predicted_per_second_p50_ratio"] == 0.75
assert comparisons[0]["predicted_per_second_p95_ratio"] == 0.8
assert comparisons[0]["rss_growth_ratio"] == 2.5
assert comparisons[0]["rss_tail_growth_ratio"] == 1.28
assert comparisons[0]["rss_tail_growth_delta_kib"] == 7
assert comparisons[0]["backend_accelerator_context_ratio"] == 0.8
assert comparisons[0]["projected_device_memory_ratio"] == 0.8
assert comparisons[0]["direct_peak_vram_ratio"] == 0.8
assert comparisons[0]["direct_peak_vram_counters_available"] is True
assert comparisons[0]["qatq_live_offloaded_segments"] == 8
passing_gates = module.evaluate_comparison_gates(
    comparisons,
    {
        "min_predicted_per_second_p50_ratio": 0.70,
        "min_predicted_per_second_p05_ratio": 0.85,
        "max_iteration_p95_ratio": 1.50,
        "max_followup_p95_ratio": 1.60,
        "max_rss_growth_ratio": 3.0,
        "max_rss_tail_growth_delta_kib": 8,
        "max_rss_tail_growth_ratio": 1.3,
        "max_backend_accelerator_context_ratio": 0.9,
        "max_projected_device_memory_ratio": 0.9,
        "max_direct_peak_vram_ratio": 0.9,
        "require_direct_peak_vram_counters": 1,
    },
)
assert passing_gates == [], passing_gates
failing_gates = "\n".join(
    module.evaluate_comparison_gates(
        comparisons,
        {
            "min_predicted_per_second_p50_ratio": 0.80,
            "min_predicted_per_second_p05_ratio": 0.95,
            "max_iteration_p95_ratio": 1.20,
            "max_followup_p95_ratio": 1.40,
            "max_rss_growth_ratio": 2.0,
            "max_rss_tail_growth_delta_kib": 6,
            "max_rss_tail_growth_ratio": 1.1,
            "max_backend_accelerator_context_ratio": 0.7,
            "max_projected_device_memory_ratio": 0.7,
            "max_direct_peak_vram_ratio": 0.7,
        },
    )
)
assert "min_predicted_per_second_p50_ratio violated: 0.75 < 0.8" in failing_gates
assert "min_predicted_per_second_p05_ratio violated: 0.9 < 0.95" in failing_gates
assert "max_iteration_p95_ratio violated: 1.3 > 1.2" in failing_gates
assert "max_followup_p95_ratio violated: 1.5 > 1.4" in failing_gates
assert "max_rss_growth_ratio violated: 2.5 > 2.0" in failing_gates
assert "max_rss_tail_growth_delta_kib violated: 7.0 > 6" in failing_gates
assert "max_rss_tail_growth_ratio violated: 1.28 > 1.1" in failing_gates
assert "max_backend_accelerator_context_ratio violated: 0.8 > 0.7" in failing_gates
assert "max_projected_device_memory_ratio violated: 0.8 > 0.7" in failing_gates
assert "max_direct_peak_vram_ratio violated: 0.8 > 0.7" in failing_gates
missing_counter = dict(comparisons[0])
missing_counter["direct_peak_vram_counters_available"] = False
missing_counter_failures = "\n".join(
    module.evaluate_comparison_gates(
        [missing_counter],
        {"require_direct_peak_vram_counters": 1},
    )
)
assert "require_direct_peak_vram_counters violated" in missing_counter_failures
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_backend_memory_gates_fail_closed() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys

spec = importlib.util.spec_from_file_location("server_matrix", "scripts/llama_cpp_live_vram_server_cancel_matrix.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

probe_summary = {
    "status": "pass",
    "backend_memory": {
        "projected_device_memory_mib": 1426,
        "memory_breakdown_mib": {
            "MTL0 (Apple M4)": {
                "total": 18186,
                "free": 16699,
                "self": 1426,
                "model": 934,
                "context": 192,
                "compute": 299,
                "unaccounted": 59,
            },
        },
    },
}

passing = module.evaluate_case_gates(
    "qwen25-15b",
    probe_summary,
    {
        "max_backend_accelerator_self_mib": 1600,
        "max_backend_accelerator_context_mib": 256,
        "max_backend_accelerator_compute_mib": 360,
        "max_projected_device_memory_mib": 1600,
    },
)
assert passing == [], passing

failing = module.evaluate_case_gates(
    "qwen25-15b",
    probe_summary,
    {
        "max_backend_accelerator_self_mib": 1400,
        "max_backend_accelerator_context_mib": 128,
        "max_backend_accelerator_compute_mib": 200,
        "max_projected_device_memory_mib": 1400,
    },
)
failures = "\n".join(failing)
assert "max_backend_accelerator_self_mib exceeded: 1426 > 1400" in failures
assert "max_backend_accelerator_context_mib exceeded: 192 > 128" in failures
assert "max_backend_accelerator_compute_mib exceeded: 299 > 200" in failures
assert "max_projected_device_memory_mib exceeded: 1426 > 1400" in failures
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_compares_multiple_qatq_candidates() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("server_matrix", "scripts/llama_cpp_live_vram_server_cancel_matrix.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

def result(case_id, mode, p95, followup, rss):
    return module.CaseResult(
        case_id=case_id,
        comparison_group="qwen",
        status="pass",
        returncode=0,
        elapsed_seconds=1,
        work_dir=Path("/tmp") / case_id,
        summary_path=Path("/tmp") / case_id / "summary.json",
        stdout_path=Path("/tmp") / case_id / "stdout",
        stderr_path=Path("/tmp") / case_id / "stderr",
        failure="",
        probe_summary={
            "mode": mode,
            "latency_checks": {
                "iteration_duration_seconds": {"p95": p95},
                "followup_duration_seconds": {"p95": followup},
            },
            "memory_checks": {"growth_kib": rss},
            "followup_completion_metrics": {
                "predicted_per_second": {"p50": 100.0 / p95, "p95": 120.0 / p95},
            },
            "checks": {"page_segment_counts": {"live_offloaded_segments": 8}},
        },
        gates={},
        gate_failures=[],
    )

comparisons = module.build_comparisons([
    result("native", "native-baseline", 5.0, 2.0, 100),
    result("qatq-q8", "qatq-live-vram", 5.5, 2.2, 150),
    result("qatq-q16", "qatq-live-vram", 6.0, 2.4, 180),
])
assert [item["qatq_case"] for item in comparisons] == ["qatq-q8", "qatq-q16"]
assert comparisons[0]["iteration_p95_ratio"] == 1.1
assert abs(comparisons[0]["predicted_per_second_p50_ratio"] - (5.0 / 5.5)) < 1e-9
assert comparisons[1]["rss_growth_ratio"] == 1.8
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_queue_depth_candidates() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-queue-depth-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-queue-depth.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert summary["status"] == "dry-run"
    assert summary["total_cases"] == 4
    assert len(summary["comparisons"]) == 1
    assert summary["comparisons"][0]["status"] == "incomplete"
    assert [case["comparison_group"] for case in plan["cases"]] == ["qwen25-15b-queue-depth"] * 4
    q8 = plan["cases"][1]["command"]
    q16 = plan["cases"][2]["command"]
    q32 = plan["cases"][3]["command"]
    assert q8[q8.index("--max-queued-pages") + 1] == "8"
    assert q16[q16.index("--max-queued-pages") + 1] == "16"
    assert q32[q32.index("--max-queued-pages") + 1] == "32"
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_perf_notrace_candidate() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-perf-notrace-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-perf-notrace.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    native = plan["cases"][0]["command"]
    qatq = plan["cases"][1]["command"]
    assert "--native-baseline" in native
    assert "--disable-qatq-traces" in qatq
    assert summary["cases"][1]["qatq_traces_enabled"] is False
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_layer_sweep_notrace_candidates() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-layer-sweep-notrace-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-layer-sweep-notrace.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    commands = [case["command"] for case in plan["cases"]]
    assert "--native-baseline" in commands[0]
    assert [cmd[cmd.index("--kv-gpu-layers") + 1] for cmd in commands[1:]] == ["0", "1", "2", "4"]
    assert all("--disable-qatq-traces" in cmd for cmd in commands[1:])
    for case in plan["cases"]:
        assert case["gates"]["max_backend_accelerator_self_mib"] == 1600
        assert case["gates"]["max_backend_accelerator_context_mib"] == 256
        assert case["gates"]["max_backend_accelerator_compute_mib"] == 384
        assert case["gates"]["max_projected_device_memory_mib"] == 1600
    assert summary["comparison_gates"] == {}
    assert summary["comparison_gate_failures"] == []
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_passes_direct_peak_vram_sampling_flags() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    tmp = Path(raw_tmp)
    config = tmp / "direct-peak-vram-matrix.json"
    work_dir = tmp / "server-cancel-matrix"
    config.write_text(json.dumps({
        "defaults": {
            "ctx_size": 4096,
            "parallel_slots": 1,
            "page_tokens": 64,
            "current_token": 2048,
            "sample_direct_peak_vram": True,
            "require_direct_peak_vram_counter": True,
            "direct_peak_vram_sample_interval_ms": 250,
        },
        "cases": [
            {
                "id": "direct-counter-case",
                "model": "/tmp/nonexistent-model.gguf",
                "model_id": "direct-counter-case",
            }
        ],
    }), encoding="utf-8")
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            str(config),
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    command = plan["cases"][0]["command"]
    assert "--sample-direct-peak-vram" in command
    assert "--require-direct-peak-vram-counter" in command
    assert command[command.index("--direct-peak-vram-sample-interval-ms") + 1] == "250"
    case = summary["cases"][0]
    assert case["sample_direct_peak_vram"] is True
    assert case["require_direct_peak_vram_counter"] is True
    assert case["direct_peak_vram_counter_available"] is None
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_layer_policy_notrace_candidate() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-layer-policy-notrace-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-layer-policy-notrace.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    commands = [case["command"] for case in plan["cases"]]
    assert "--native-baseline" in commands[0]
    assert commands[1][commands[1].index("--kv-gpu-layers") + 1] == "1"
    assert commands[1][commands[1].index("--warmup-iterations") + 1] == "1"
    assert commands[1][commands[1].index("--max-rss-tail-growth-kib") + 1] == "8192"
    assert commands[1][commands[1].index("--rss-tail-window") + 1] == "4"
    assert "--disable-qatq-traces" in commands[1]
    for case in plan["cases"]:
        assert case["gates"]["max_backend_accelerator_self_mib"] == 1600
        assert case["gates"]["max_backend_accelerator_context_mib"] == 256
        assert case["gates"]["max_backend_accelerator_compute_mib"] == 384
        assert case["gates"]["max_projected_device_memory_mib"] == 1600
    assert summary["comparison_gates"] == {
        "max_backend_accelerator_context_ratio": 0.99,
        "max_followup_p95_ratio": 1.2,
        "max_iteration_p95_ratio": 1.12,
        "max_projected_device_memory_ratio": 1.0,
        "max_rss_tail_growth_delta_kib": 2048,
        "min_predicted_per_second_p50_ratio": 0.9,
        "min_predicted_per_second_p05_ratio": 0.9,
    }
    assert summary["comparison_gate_failures"] == []
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_family_policy_notrace_candidates() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-family-policy-notrace-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-family-policy-notrace.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert len(plan["cases"]) == 6
    commands = [case["command"] for case in plan["cases"]]
    native_commands = [command for command in commands if "--native-baseline" in command]
    qatq_commands = [command for command in commands if "--native-baseline" not in command]
    assert len(native_commands) == 3
    assert len(qatq_commands) == 3
    for command in commands:
        assert command[command.index("--warmup-iterations") + 1] == "1"
        assert command[command.index("--max-rss-tail-growth-kib") + 1] == "8192"
        assert command[command.index("--rss-tail-window") + 1] == "4"
        assert "--require-backend-memory-diagnostics" in command
    for command in native_commands:
        assert "--disable-qatq-traces" not in command
    for command in qatq_commands:
        assert "--disable-qatq-traces" in command
        assert command[command.index("--kv-gpu-layers") + 1] == "1"
    qwen3b_qatq = next(command for command in qatq_commands if "qwen2.5-3b-qatq-l1-family-policy-notrace" in command)
    assert qwen3b_qatq[qwen3b_qatq.index("--page-tokens") + 1] == "256"
    assert qwen3b_qatq[qwen3b_qatq.index("--max-queued-pages") + 1] == "4"
    phi_qatq = next(command for command in qatq_commands if "phi3.5-mini-qatq-l1-p128-family-policy-notrace" in command)
    assert phi_qatq[phi_qatq.index("--page-tokens") + 1] == "128"
    phi_native = next(command for command in native_commands if "phi3.5-mini-native-family-policy-baseline" in command)
    assert phi_native[phi_native.index("--prompt-repeat") + 1] == "48"
    assert phi_qatq[phi_qatq.index("--prompt-repeat") + 1] == "48"
    assert summary["comparison_gates"] == {
        "max_backend_accelerator_context_ratio": 0.99,
        "max_followup_p95_ratio": 1.45,
        "max_iteration_p95_ratio": 1.55,
        "max_projected_device_memory_ratio": 1.0,
        "max_rss_tail_growth_delta_kib": 2048,
        "min_predicted_per_second_p50_ratio": 0.85,
        "min_predicted_per_second_p05_ratio": 0.85,
    }
    assert summary["comparison_gate_failures"] == []
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_family_policy_soak_notrace_candidates() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-family-policy-soak-notrace-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-family-policy-soak-notrace.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert len(plan["cases"]) == 6
    commands = [case["command"] for case in plan["cases"]]
    native_commands = [command for command in commands if "--native-baseline" in command]
    qatq_commands = [command for command in commands if "--native-baseline" not in command]
    assert len(native_commands) == 3
    assert len(qatq_commands) == 3
    for command in commands:
        assert command[command.index("--warmup-iterations") + 1] == "1"
        assert command[command.index("--iterations") + 1] == "10"
        assert command[command.index("--max-rss-tail-growth-kib") + 1] == "8192"
        assert command[command.index("--rss-tail-window") + 1] == "4"
        assert "--require-backend-memory-diagnostics" in command
    for command in native_commands:
        assert "--disable-qatq-traces" not in command
    for command in qatq_commands:
        assert "--disable-qatq-traces" in command
        assert command[command.index("--kv-gpu-layers") + 1] == "1"
    qwen3b_qatq = next(command for command in qatq_commands if "qwen2.5-3b-qatq-l1-family-policy-soak-notrace" in command)
    assert qwen3b_qatq[qwen3b_qatq.index("--page-tokens") + 1] == "256"
    assert qwen3b_qatq[qwen3b_qatq.index("--max-queued-pages") + 1] == "4"
    phi_qatq = next(command for command in qatq_commands if "phi3.5-mini-qatq-l1-p128-family-policy-soak-notrace" in command)
    assert phi_qatq[phi_qatq.index("--page-tokens") + 1] == "128"
    phi_native = next(command for command in native_commands if "phi3.5-mini-native-family-policy-soak-baseline" in command)
    assert phi_native[phi_native.index("--prompt-repeat") + 1] == "48"
    assert phi_qatq[phi_qatq.index("--prompt-repeat") + 1] == "48"
    assert summary["comparison_gates"] == {
        "max_backend_accelerator_context_ratio": 0.99,
        "max_followup_p95_ratio": 1.45,
        "max_iteration_p95_ratio": 1.55,
        "max_projected_device_memory_ratio": 1.0,
        "max_rss_tail_growth_delta_kib": 2048,
        "min_predicted_per_second_p50_ratio": 0.85,
        "min_predicted_per_second_p05_ratio": 0.85,
    }
    assert summary["comparison_gate_failures"] == []
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_dry_run_builds_phi_policy_burnin_candidates() {
    let output = run_python_snippet(
        r#"
import json, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-family-phi-policy-burnin-notrace-matrix"
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            "adapters/llama-cpp/live-vram-server-family-phi-policy-burnin-notrace.local.example.json",
            "--probe-runner",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-matrix-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert len(plan["cases"]) == 2
    commands = [case["command"] for case in plan["cases"]]
    native_commands = [command for command in commands if "--native-baseline" in command]
    qatq_commands = [command for command in commands if "--native-baseline" not in command]
    assert len(native_commands) == 1
    assert len(qatq_commands) == 1
    for command in commands:
        assert command[command.index("--warmup-iterations") + 1] == "1"
        assert command[command.index("--iterations") + 1] == "10"
        assert command[command.index("--prompt-repeat") + 1] == "48"
        assert command[command.index("--max-rss-tail-growth-kib") + 1] == "8192"
        assert command[command.index("--rss-tail-window") + 1] == "4"
        assert "--require-backend-memory-diagnostics" in command
    native = native_commands[0]
    qatq = qatq_commands[0]
    assert "--disable-qatq-traces" not in native
    assert "--disable-qatq-traces" in qatq
    assert qatq[qatq.index("--page-tokens") + 1] == "128"
    assert qatq[qatq.index("--kv-gpu-layers") + 1] == "1"
    assert summary["comparison_gates"] == {
        "max_backend_accelerator_context_ratio": 0.99,
        "max_followup_p95_ratio": 1.45,
        "max_iteration_p95_ratio": 1.55,
        "max_projected_device_memory_ratio": 1.0,
        "max_rss_tail_growth_delta_kib": 2048,
        "min_predicted_per_second_p50_ratio": 0.85,
        "min_predicted_per_second_p05_ratio": 0.85,
    }
    assert summary["comparison_gate_failures"] == []
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_matrix_fails_closed_on_probe_failure() {
    let output = run_python_snippet(
        r#"
import json, os, subprocess, sys, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    tmp = Path(raw_tmp)
    config = tmp / "config.json"
    probe = tmp / "fake_probe.py"
    work_dir = tmp / "work"
    config.write_text(json.dumps({
        "defaults": {
            "parallel_slots": 2,
            "concurrent_followup_during_cancel": True,
            "require_flattened_flash_consumer": True
        },
        "cases": [
            {
                "id": "bad-case",
                "model": "/tmp/model.gguf",
                "model_id": "bad-case",
                "require_live_offloaded_stream_count": 2
            },
            {
                "id": "should-not-run",
                "model": "/tmp/model.gguf",
                "model_id": "should-not-run"
            }
        ]
    }), encoding="utf-8")
    probe.write_text(
        "import argparse, json\n"
        "from pathlib import Path\n"
        "parser = argparse.ArgumentParser()\n"
        "parser.add_argument('--work-dir', required=True)\n"
        "parser.add_argument('--llama-server')\n"
        "parser.add_argument('--model')\n"
        "parser.add_argument('--model-id')\n"
        "parser.add_argument('--parallel-slots')\n"
        "parser.add_argument('--concurrent-followup-during-cancel', action='store_true')\n"
        "parser.add_argument('--require-flattened-flash-consumer', action='store_true')\n"
        "parser.add_argument('--require-live-offloaded-stream-count')\n"
        "args, _ = parser.parse_known_args()\n"
        "work = Path(args.work_dir)\n"
        "work.mkdir(parents=True, exist_ok=True)\n"
        "summary = {'format': 'qatq-live-vram-server-cancel-probe-summary-v1', 'status': 'fail', 'checks': {'failures': ['synthetic failure']}}\n"
        "(work / 'summary.json').write_text(json.dumps(summary), encoding='utf-8')\n"
        "raise SystemExit(7)\n",
        encoding="utf-8",
    )
    result = subprocess.run(
        [
            sys.executable,
            "scripts/llama_cpp_live_vram_server_cancel_matrix.py",
            "--config",
            str(config),
            "--probe-runner",
            str(probe),
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 1, result.stdout
    summary = json.loads((work_dir / "summary.json").read_text())
    assert summary["status"] == "fail"
    assert summary["failed"] == 1
    assert summary["total_cases"] == 1
    assert "synthetic failure" in summary["cases"][0]["failure"]
    assert not (work_dir / "should-not-run").exists()
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_dry_run_records_soak_iterations() {
    let output = run_python_snippet(
        r#"
import json, subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-probe"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--iterations",
            "5",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-probe-plan.json").read_text())
    summary = json.loads((work_dir / "summary.json").read_text())
    assert plan["iterations"] == 5
    assert summary["iterations"] == 5
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_dry_run_records_concurrent_mode() {
    let output = run_python_snippet(
        r#"
import json, subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-probe"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--parallel-slots",
            "2",
            "--kv-unified",
            "--concurrent-followup-during-cancel",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-probe-plan.json").read_text())
    command = plan["command"]
    assert command[command.index("-np") + 1] == "2"
    assert "--kv-unified" in command
    summary = json.loads((work_dir / "summary.json").read_text())
    assert summary["status"] == "dry-run"
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_dry_run_allows_nonunified_concurrent_mode() {
    let output = run_python_snippet(
        r#"
import json, subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-probe"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--parallel-slots",
            "2",
            "--concurrent-followup-during-cancel",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-probe-plan.json").read_text())
    command = plan["command"]
    assert command[command.index("-np") + 1] == "2"
    assert "--kv-unified" not in command
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_classifies_context_http_400_as_fatal_shape_failure() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

assert module.is_fatal_request_shape_failure([
    "streaming completion returned HTTP 400: b'{\"error\":{\"message\":\"request exceeds the available context size\",\"type\":\"exceed_context_size_error\"}}'"
])
assert not module.is_fatal_request_shape_failure([
    "streaming request was not cancelled",
    "server did not complete a follow-up request after cancellation",
])
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_memory_checks_include_iteration_rss_peaks() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

checks = module.evaluate_memory_samples(
    [{"rss_kib": 1000}, {"rss_kib": 900}],
    1,
    iterations=[
        {"rss_before_kib": 1100, "rss_after_kib": 1600},
        {"rss_before_kib": 1200, "rss_after_kib": 3000},
    ],
)
assert checks["baseline_rss_kib"] == 1000
assert checks["peak_rss_kib"] == 3000
assert checks["growth_kib"] == 2000
assert checks["rss_sample_count"] == 6
assert checks["rss_after_sample_count"] == 2
assert checks["rss_after_last_minus_first_kib"] == 1400
assert checks["rss_tail_range_kib"] == 1400
assert checks["failures"], checks
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_memory_checks_gate_tail_growth() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

passing = module.evaluate_memory_samples(
    [{"label": "post-warmup", "rss_kib": 1000}, {"label": "post-iterations", "rss_kib": 1120}],
    2,
    iterations=[
        {"rss_before_kib": 1000, "rss_after_kib": 2000},
        {"rss_before_kib": 2000, "rss_after_kib": 2050},
        {"rss_before_kib": 2050, "rss_after_kib": 2060},
        {"rss_before_kib": 2060, "rss_after_kib": 2070},
        {"rss_before_kib": 2070, "rss_after_kib": 2080},
    ],
    max_rss_tail_growth_kib=128,
    rss_tail_window=4,
)
assert passing["rss_after_last_minus_first_kib"] == 80
assert passing["rss_tail_range_kib"] == 30
assert passing["rss_tail_last_minus_first_kib"] == 30
assert passing["rss_tail_growth_kib"] == 30
assert passing["rss_tail_gate_growth_kib"] == 30
assert passing["rss_tail_window_used"] == 4
assert passing["failures"] == [], passing

failing = module.evaluate_memory_samples(
    [{"label": "post-warmup", "rss_kib": 1000}, {"label": "post-iterations", "rss_kib": 1800}],
    2,
    iterations=[
        {"rss_before_kib": 1000, "rss_after_kib": 2000},
        {"rss_before_kib": 2000, "rss_after_kib": 2050},
        {"rss_before_kib": 2050, "rss_after_kib": 2700},
        {"rss_before_kib": 2700, "rss_after_kib": 2800},
        {"rss_before_kib": 2800, "rss_after_kib": 3000},
    ],
    max_rss_tail_growth_kib=128,
    rss_tail_window=4,
)
assert failing["rss_tail_range_kib"] == 950
assert failing["rss_tail_last_minus_first_kib"] == 950
assert failing["rss_tail_growth_kib"] == 950
assert failing["rss_tail_gate_growth_kib"] == 950
assert any("server steady RSS tail growth was 950 KiB" in failure for failure in failing["failures"]), failing

releasing = module.evaluate_memory_samples(
    [{"label": "post-warmup", "rss_kib": 1000}, {"label": "post-iterations", "rss_kib": 1800}],
    2,
    iterations=[
        {"rss_before_kib": 1000, "rss_after_kib": 3000},
        {"rss_before_kib": 3000, "rss_after_kib": 2900},
        {"rss_before_kib": 2900, "rss_after_kib": 2000},
        {"rss_before_kib": 2000, "rss_after_kib": 2500},
        {"rss_before_kib": 2500, "rss_after_kib": 1800},
    ],
    max_rss_tail_growth_kib=128,
    rss_tail_window=4,
)
assert releasing["rss_tail_range_kib"] == 1100
assert releasing["rss_tail_last_minus_first_kib"] == -1100
assert releasing["rss_tail_growth_kib"] == 0
assert releasing["rss_tail_gate_growth_kib"] == 0
assert releasing["failures"] == [], releasing

recovering_below_baseline = module.evaluate_memory_samples(
    [{"label": "post-warmup", "rss_kib": 1000}, {"label": "post-iterations", "rss_kib": 1100}],
    2,
    iterations=[
        {"rss_before_kib": 1000, "rss_after_kib": 3000},
        {"rss_before_kib": 3000, "rss_after_kib": 2400},
        {"rss_before_kib": 2400, "rss_after_kib": 1800},
        {"rss_before_kib": 1800, "rss_after_kib": 2200},
        {"rss_before_kib": 2200, "rss_after_kib": 2500},
    ],
    max_rss_tail_growth_kib=128,
    rss_tail_window=4,
)
assert recovering_below_baseline["rss_tail_range_kib"] == 700
assert recovering_below_baseline["rss_tail_last_minus_first_kib"] == 100
assert recovering_below_baseline["rss_tail_growth_kib"] == 100
assert recovering_below_baseline["rss_tail_gate_growth_kib"] == 0
assert recovering_below_baseline["failures"] == [], recovering_below_baseline

too_short = module.evaluate_memory_samples(
    [{"label": "post-warmup", "rss_kib": 1000}, {"label": "post-iterations", "rss_kib": 1010}],
    1,
    iterations=[{"rss_before_kib": 1000, "rss_after_kib": 1010}],
    max_rss_tail_growth_kib=128,
    rss_tail_window=4,
)
assert any("only 1 RSS-after iteration samples" in failure for failure in too_short["failures"]), too_short
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_memory_checks_can_start_after_warmup() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

samples = [
    {"label": "post-readiness", "rss_kib": 1000},
    {"label": "post-warmup", "rss_kib": 1800},
    {"label": "post-iterations", "rss_kib": 1810},
]
warmup_checks = module.evaluate_memory_samples(
    module.warmup_memory_samples(samples),
    0,
    iterations=[{"rss_before_kib": 1000, "rss_after_kib": 1800}],
)
steady_checks = module.evaluate_memory_samples(
    module.measured_memory_samples(samples, True),
    1,
    iterations=[{"rss_before_kib": 1800, "rss_after_kib": 1810}],
)
assert warmup_checks["baseline_rss_kib"] == 1000
assert warmup_checks["peak_rss_kib"] == 1800
assert warmup_checks["growth_kib"] == 800
assert steady_checks["baseline_rss_kib"] == 1800
assert steady_checks["peak_rss_kib"] == 1810
assert steady_checks["growth_kib"] == 10
assert steady_checks["failures"] == []
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_requires_flattened_multistream_trace() {
    let output = run_python_snippet(
        r#"
import importlib.util, json, sys, tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp)
    page_segments = work_dir / "page-segments.jsonl"
    page_segments.write_text(
        json.dumps({
            "event": "attention-page-segments",
            "attention_consumed": True,
            "consumer": "backend_scheduled_flattened_flash_attention",
            "segments": [
                {"stream_index": 0, "live_offloaded": True, "shape": [128, 2, 64, 1]},
                {"stream_index": 1, "live_offloaded": True, "shape": [128, 2, 64, 1]},
            ],
        }) + "\n",
        encoding="utf-8",
    )
    artifacts = {
        "event_trace": work_dir / "event-trace.jsonl",
        "page_segments": page_segments,
        "persistent_page_source": work_dir / "persistent-page-source.jsonl",
        "persistent_pool": work_dir / "persistent-pool.jsonl",
    }
    result = module.evaluate_result(
        artifacts,
        stream_cancelled=True,
        stream_bytes=256,
        health_after_cancel_ok=True,
        followup_ok=True,
        followup_bytes=128,
        process_returncode=0,
        startup_failures=[],
        require_flattened_flash_consumer=True,
        require_live_offloaded_stream_count=2,
    )
    assert result["failures"] == [], result
    counts = result["page_segment_counts"]
    assert counts["consumer.backend_scheduled_flattened_flash_attention"] == 1
    assert counts["live_offloaded_stream.0"] == 1
    assert counts["live_offloaded_stream.1"] == 1
    assert counts["live_offloaded_shape.128x2x64x1"] == 2
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_rejects_native_trace_fallback() {
    let output = run_python_snippet(
        r#"
import importlib.util, json, sys, tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp)
    page_segments = work_dir / "page-segments.jsonl"
    page_segments.write_text(
        json.dumps({
            "event": "attention-page-segments",
            "attention_consumed": True,
            "consumer": "ggml_concat_compatibility_fallback",
            "segments": [
                {"stream_index": 0, "live_offloaded": True, "shape": [128, 2, 64, 1]},
            ],
        }) + "\n",
        encoding="utf-8",
    )
    artifacts = {
        "event_trace": work_dir / "event-trace.jsonl",
        "page_segments": page_segments,
        "persistent_page_source": work_dir / "persistent-page-source.jsonl",
        "persistent_pool": work_dir / "persistent-pool.jsonl",
    }
    result = module.evaluate_result(
        artifacts,
        stream_cancelled=True,
        stream_bytes=256,
        health_after_cancel_ok=True,
        followup_ok=True,
        followup_bytes=128,
        process_returncode=0,
        startup_failures=[],
        require_flattened_flash_consumer=True,
        require_live_offloaded_stream_count=2,
    )
    failures = "\n".join(result["failures"])
    assert "no flattened Flash attention consumers" in failures
    assert "unexpected attention consumers" in failures
    assert "1 live-offloaded stream indices" in failures
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_summarises_persistent_page_source_retention() {
    let output = run_python_snippet(
        r#"
import importlib.util, json, sys, tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

with tempfile.TemporaryDirectory() as raw_tmp:
    path = Path(raw_tmp) / "persistent-page-source.jsonl"
    path.write_text(
        json.dumps({
            "event": "attention-persistent-page-source",
            "composition": "ggml_concat",
            "native_page_streaming": False,
            "retained_bytes": 1024,
            "source_bytes": 256,
            "composed_bytes": 512,
            "requested_bytes": 128,
            "allocated_bytes": 256,
            "retained_pages": 2,
        }) + "\n" +
        json.dumps({
            "event": "attention-persistent-page-source",
            "composition": "retained_tiled_table",
            "native_page_streaming": True,
            "retained_bytes": 4096,
            "source_bytes": 512,
            "composed_bytes": 1024,
            "requested_bytes": 256,
            "allocated_bytes": 1024,
            "retained_pages": 4,
        }) + "\n",
        encoding="utf-8",
    )
    stats = module.count_persistent_page_source_events(path)
    assert stats["events"] == 2
    assert stats["max_retained_bytes"] == 4096
    assert stats["max_source_bytes"] == 512
    assert stats["max_composed_bytes"] == 1024
    assert stats["max_requested_bytes"] == 256
    assert stats["max_allocated_bytes"] == 1024
    assert stats["max_retained_pages"] == 4
    assert stats["native_page_streaming_true"] == 1
    assert stats["native_page_streaming_false"] == 1
    assert stats["composition_counts"]["ggml_concat"] == 1
    assert stats["composition_counts"]["retained_tiled_table"] == 1
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_parses_llama_cpp_backend_memory() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys, tempfile
from pathlib import Path

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

with tempfile.TemporaryDirectory() as raw_tmp:
    path = Path(raw_tmp) / "server-stderr.log"
    path.write_text(
        "llama_params_fit_impl: projected to use 5304 MiB of device memory vs. 18182 MiB of free device memory\n"
        "llama_model_load_from_file_impl: using device MTL0 (Apple M4) (unknown id) - 18185 MiB free\n"
        "load_tensors:   CPU_Mapped model buffer size =    52.84 MiB\n"
        "load_tensors:  MTL0_Mapped model buffer size =  2228.82 MiB\n"
        "llama_kv_cache:        CPU KV buffer size =    96.00 MiB\n"
        "llama_kv_cache:       MTL0 KV buffer size =  2976.00 MiB\n"
        "sched_reserve:       MTL0 compute buffer size =    96.70 MiB\n"
        "sched_reserve:        CPU compute buffer size =    30.24 MiB\n"
        "llama_memory_breakdown_print: |   - MTL0 (Apple M4)    | 18186 = 12673 + (5339 =  2228 +    2976 +     134) +         173 |\n"
        "llama_memory_breakdown_print: |   - Host               |                   184 =    52 +      96 +      35                |\n",
        encoding="utf-8",
    )
    stats = module.parse_llama_cpp_backend_memory(path)
    assert stats["projected_device_memory_mib"] == 5304
    assert stats["projected_free_device_memory_mib"] == 18182
    assert stats["device_free_mib"]["MTL0 (Apple M4)"] == 18185
    assert stats["model_buffers_mib"]["MTL0_Mapped"] == 2228.82
    assert stats["kv_buffers_mib"]["MTL0"] == 2976.0
    assert stats["compute_buffers_mib"]["CPU"] == 30.24
    mtl = stats["memory_breakdown_mib"]["MTL0 (Apple M4)"]
    assert mtl["total"] == 18186
    assert mtl["free"] == 12673
    assert mtl["self"] == 5339
    assert mtl["model"] == 2228
    assert mtl["context"] == 2976
    assert mtl["compute"] == 134
    assert mtl["unaccounted"] == 173
    host = stats["memory_breakdown_mib"]["Host"]
    assert host["self"] == 184
    assert host["model"] == 52
    assert host["context"] == 96
    assert host["compute"] == 35
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_backend_memory_gate_fails_closed() {
    let output = run_python_snippet(
        r#"
import importlib.util, sys

spec = importlib.util.spec_from_file_location("server_cancel", "scripts/llama_cpp_live_vram_server_cancel_probe.py")
module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = module
spec.loader.exec_module(module)

missing = module.evaluate_backend_memory_diagnostics({
    "projected_device_memory_mib": None,
    "memory_breakdown_mib": {},
})
assert "missing projected device memory" in "\n".join(missing)
assert "missing accelerator memory breakdown" in "\n".join(missing)

present = module.evaluate_backend_memory_diagnostics({
    "projected_device_memory_mib": 1426,
    "memory_breakdown_mib": {
        "MTL0 (Apple M4)": {
            "self": 1426,
            "model": 934,
            "context": 192,
            "compute": 299,
        }
    },
})
assert present == [], present
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_vram_server_cancel_probe_scales_budget_for_small_pages() {
    let output = run_python_snippet(
        r#"
import json, subprocess, tempfile
from pathlib import Path

with tempfile.TemporaryDirectory() as raw_tmp:
    work_dir = Path(raw_tmp) / "server-cancel-probe"
    result = subprocess.run(
        [
            "python3",
            "scripts/llama_cpp_live_vram_server_cancel_probe.py",
            "--model",
            "/tmp/nonexistent-model.gguf",
            "--llama-server",
            "/tmp/nonexistent-llama-server",
            "--work-dir",
            str(work_dir),
            "--page-tokens",
            "64",
            "--ctx-size",
            "4096",
            "--dry-run",
        ],
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 0, result.stderr
    plan = json.loads((work_dir / "server-cancel-probe-plan.json").read_text())
    env = plan["env"]
    assert plan["derived"]["derived_page_segments"] == 64
    assert plan["derived"]["max_page_segments"] == 64
    assert plan["derived"]["graph_extra_nodes"] == 131072
    assert env["LLAMA_QATQ_PAGE_TOKENS"] == "64"
    assert env["LLAMA_QATQ_ATTENTION_PAGE_SEGMENTS_MAX_PAGES"] == "64"
    assert env["LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_SOURCE_PAGES"] == "64"
    assert env["LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_MAX_RETAINED_BYTES"] == "1073741824"
    assert env["LLAMA_QATQ_GRAPH_EXTRA_NODES"] == "131072"
"#,
    );

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
