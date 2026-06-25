#!/usr/bin/env python3
"""Audit a patched llama.cpp checkout for QATQ live-VRAM adapter readiness.

This is a structural gate, not a benchmark. It checks whether the runtime patch
contains the hooks needed for exported-KV evidence and whether it appears to
contain the harder page-granular attention-loop machinery required before QATQ
can claim transparent live VRAM reduction.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Check:
    name: str
    passed: bool
    detail: str


LIVE_PAGING_REQUIRED_CHECKS = [
    "export.c_api",
    "export.runner_flags",
    "export.token_page_manifest",
    "export.manifest_allocator_attestation",
    "export.event_trace_schema",
    "export.event_trace_scheduler_alignment",
    "live.logical_page_residency_manifest",
    "live.attention_page_segment_api",
    "live.native_attention_bypasses_whole_tensor_getters",
    "live.native_page_streaming_preflight",
    "live.native_segmented_kqv_contract",
    "live.native_attention_bypasses_concat_page_sources",
    "live.native_page_streaming_attention_kernel",
    "live.native_multi_segment_attention_reduction",
    "live.native_multi_segment_backend_surface",
    "live.native_multi_segment_backend_graph_integration",
    "live.native_backend_op_mask_support",
    "live.native_backend_op_typed_mask_support",
    "live.native_backend_op_typed_kv_support",
    "live.native_backend_op_transient_pool_byte_budget",
    "live.native_backend_op_avoids_full_kv_packing",
    "live.native_backend_op_multi_page_streaming",
    "live.native_backend_op_consumes_staged_segments",
    "live.native_backend_op_avoids_staged_arena",
    "live.native_backend_op_descriptor_path",
    "live.native_backend_op_selective_pool_default",
    "live.native_flattened_flash_attention_route",
    "live.native_flattened_flash_stream_local_contract",
    "live.native_attention_not_cpu_custom_op",
    "live.gpu_page_staging_mode",
    "live.restore_slot_pressure_self_test",
    "live.physical_per_page_allocation_attestation",
    "live.attention_loop_lifecycle_trace",
]


PAGE_STAGING_REQUIRED_CHECKS = [
    "export.c_api",
    "export.runner_flags",
    "export.token_page_manifest",
    "export.manifest_allocator_attestation",
    "export.event_trace_schema",
    "export.event_trace_scheduler_alignment",
    "live.logical_page_residency_manifest",
    "live.attention_page_segment_api",
    "live.native_page_streaming_preflight",
    "live.gpu_page_staging_mode",
    "live.persistent_page_pool_self_test",
    "live.restore_slot_pressure_self_test",
    "live.physical_per_page_allocation_attestation",
    "live.attention_loop_lifecycle_trace",
    "live.backend_page_self_test",
]

RUNTIME_SECURITY_REQUIRED_CHECKS = [
    "export.event_trace_schema",
    "export.event_trace_scheduler_alignment",
    "live.logical_page_residency_manifest",
    "live.gpu_page_staging_mode",
    "live.backend_page_self_test",
    "live.restore_slot_pressure_self_test",
    "live.physical_per_page_allocation_attestation",
    "live.attention_loop_lifecycle_trace",
    "live.native_backend_op_unsupported_cases_fail_closed",
]


DIAGNOSTIC_COMPATIBILITY_CHECKS = {
    "live.attention_materialized_source_path",
    "live.attention_page_composed_source_path",
    "live.attention_persistent_page_source_path",
    "live.no_concat_composed_attention_source",
    "live.attention_loop_trace_not_export_only",
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--llama-cpp", default="/private/tmp/qatq-llama.cpp")
    parser.add_argument(
        "--patch-file",
        help=(
            "Audit a unified diff patch directly. This is a lightweight "
            "snippet audit for the pinned adapter patch; an applied-source "
            "audit remains authoritative before native production claims."
        ),
    )
    parser.add_argument("--output", help="Optional JSON report path")
    parser.add_argument(
        "--require-live-paging",
        action="store_true",
        help="Return non-zero unless page-granular attention-loop live paging appears implemented.",
    )
    parser.add_argument(
        "--require-runtime-security",
        action="store_true",
        help="Return non-zero unless runtime security and resource-limit adapter gates appear implemented.",
    )
    args = parser.parse_args()

    if args.patch_file:
        patch = Path(args.patch_file)
        require(patch.exists(), f"patch file does not exist: {patch}")
        files = read_files_from_patch(patch)
        source = str(patch)
        commit = "patch-file"
        audit_scope = "patch-snippet"
    else:
        root = Path(args.llama_cpp)
        require(root.exists(), f"llama.cpp path does not exist: {root}")
        files = {
            "include": read_text(root / "include" / "llama.h"),
            "llama": read_text(root / "src" / "llama.cpp"),
            "context": read_text(root / "src" / "llama-context.cpp"),
            "context_h": read_text(root / "src" / "llama-context.h"),
            "kv": read_text(root / "src" / "llama-kv-cache.cpp"),
            "kv_h": read_text(root / "src" / "llama-kv-cache.h"),
            "graph": read_text(root / "src" / "llama-graph.cpp"),
            "ggml_h": read_text(root / "ggml" / "include" / "ggml.h"),
            "ggml_c": read_text(root / "ggml" / "src" / "ggml.c"),
            "metal": read_text(root / "ggml" / "src" / "ggml-metal" / "ggml-metal.metal"),
            "metal_device_h": read_text(root / "ggml" / "src" / "ggml-metal" / "ggml-metal-device.h"),
            "metal_device_cpp": read_text(root / "ggml" / "src" / "ggml-metal" / "ggml-metal-device.cpp"),
            "metal_device_m": read_text(root / "ggml" / "src" / "ggml-metal" / "ggml-metal-device.m"),
            "metal_ops_h": read_text(root / "ggml" / "src" / "ggml-metal" / "ggml-metal-ops.h"),
            "metal_ops_cpp": read_text(root / "ggml" / "src" / "ggml-metal" / "ggml-metal-ops.cpp"),
            "simple": read_text(root / "examples" / "simple" / "simple.cpp"),
        }
        source = str(root)
        commit = git_commit(root)
        audit_scope = "applied-source"
    checks = build_checks(files)
    export_ready = all(check.passed for check in checks if check.name.startswith("export."))
    staging_ready = page_staging_ready(checks)
    live_ready = native_ggml_live_ready(checks)
    security_ready = runtime_security_ready(checks)
    report = {
        "format": "qatq-llama-cpp-live-vram-adapter-audit-v1",
        "llama_cpp": source,
        "commit": commit,
        "audit_scope": audit_scope,
        "authoritative_for_native_release": audit_scope == "applied-source",
        "export_ready": export_ready,
        "page_staging_ready": staging_ready,
        "live_paging_ready": live_ready,
        "runtime_security_ready": security_ready,
        "live_paging_readiness_basis": "native-ggml-production-required-checks",
        "runtime_security_readiness_basis": "runtime-security-required-checks",
        "required_live_paging_failures": failed_required_checks(checks, LIVE_PAGING_REQUIRED_CHECKS),
        "required_page_staging_failures": failed_required_checks(checks, PAGE_STAGING_REQUIRED_CHECKS),
        "required_runtime_security_failures": failed_required_checks(
            checks, RUNTIME_SECURITY_REQUIRED_CHECKS
        ),
        "diagnostic_compatibility_failures": failed_required_checks(
            checks, sorted(DIAGNOSTIC_COMPATIBILITY_CHECKS)
        ),
        "non_required_failures": non_required_failures(checks),
        "checks": [check.__dict__ for check in checks],
    }

    rendered = json.dumps(report, indent=2) + "\n"
    if args.output:
        Path(args.output).write_text(rendered, encoding="utf-8")
    print(rendered, end="")

    if args.require_live_paging and not live_ready:
        return 1
    if args.require_runtime_security and not security_ready:
        return 1
    return 0


def build_checks(files: dict[str, str]) -> list[Check]:
    wrapper_impl = files["llama"] + "\n" + files["context"]
    all_sources = "\n".join(files.values())
    graph_get_sites = re.findall(r"mctx_cur->get_[kv]\(ctx0,\s*il\)", files["graph"])
    get_k_body = function_body(files["kv"], "llama_kv_cache::get_k")
    get_v_body = function_body(files["kv"], "llama_kv_cache::get_v")
    context_get_k_body = function_body(files["kv"], "llama_kv_cache_context::get_k")
    context_get_v_body = function_body(files["kv"], "llama_kv_cache_context::get_v")
    constructor_body = function_body(files["kv"], "llama_kv_cache::llama_kv_cache")
    segmented_graph_body = function_body(files["graph"], "llm_graph_context::build_attn_mha_segmented_kqv")
    backend_op_body = function_body(files["graph"], "qatq_native_page_streaming_backend_op")
    compact_backend_op_body = re.sub(r"\s+", "", backend_op_body)
    compact_segmented_graph_body = re.sub(r"\s+", "", segmented_graph_body)
    compact_graph = re.sub(r"\s+", "", files["graph"])
    flattened_flash_body = snippet_between(
        files["graph"],
        'if (qatq_parse_u64_env("LLAMA_QATQ_NATIVE_PAGE_STREAMING_FLATTEN_FLASH", 0) != 0)',
        "ggml_tensor * cur = qatq_native_page_streaming_backend_op",
    )
    qatq_segmented_kqv_call_in_backend = "ggml_qatq_segmented_kqv(ctx0" in compact_backend_op_body
    qatq_segmented_kqv_call_in_graph = "ggml_qatq_segmented_kqv(ctx0" in compact_graph
    native_attention_selective_resident_fast_path = (
        "live_offloaded" in files["kv_h"]
        and "live_offloaded" in files["kv"]
        and "qatq_segments_include_live_offloaded_page" in files["graph"]
        and "!qatq_segments_include_live_offloaded_page(k_segments)&&!qatq_segments_include_live_offloaded_page(v_segments)" in compact_segmented_graph_body
        and "build_attn_mha(q,k,v,kq_b,kq_mask,sinks,v_mla,kq_scale,il)" in compact_segmented_graph_body
    )
    native_attention_route_enabled = (
        "qatq_native_page_streaming_attention_enabled()" in files["graph"]
        and (
            "has_qatq_page_staging(il)" in files["graph"]
            or "if (qatq_native_page_streaming_attention_enabled())" in files["graph"]
        )
    )
    native_attention_bypasses_whole_tensor_getters = (
        native_attention_route_enabled
        and "build_attn_mha_segmented_kqv(mctx_cur" in files["graph"]
        and (
            (
                "mctx_cur->get_k(ctx0, il)" not in segmented_graph_body
                and "mctx_cur->get_v(ctx0, il)" not in segmented_graph_body
            )
            or native_attention_selective_resident_fast_path
        )
    )
    native_attention_bypasses_concat_page_sources = (
        "qatq_maybe_page_compose_attention_source" not in segmented_graph_body
        and "qatq_maybe_persistent_page_attention_source" not in segmented_graph_body
        and "ggml_concat(ctx, composed" not in segmented_graph_body
    )
    native_backend_op_avoids_full_kv_packing = (
        qatq_segmented_kqv_call_in_backend
        and "k_packed" not in backend_op_body
        and "v_packed" not in backend_op_body
        and "ggml_concat(ctx0, k_packed" not in backend_op_body
        and "ggml_concat(ctx0, v_packed" not in backend_op_body
    )
    native_backend_op_multi_page_streaming = (
        native_backend_op_avoids_full_kv_packing
        and "k_segments.size() == 1" not in backend_op_body
        and "requires a native page-table backend before multi-page execution" not in backend_op_body
        and (
            "qatq_build_segmented_kqv_staged_arena(ctx0, k_segments" in backend_op_body
            or "qatq_build_segmented_kqv_graph_scheduled_arena(ctx0, gf, k_segments" in backend_op_body
            or "qatq_build_segmented_kqv_page_pool(ctx0, gf, sched, target_backend, k_segments" in backend_op_body
            or "qatq_segment_page_table" in backend_op_body
            or "qatq_page_table" in backend_op_body
        )
        and (
            "qatq_build_segmented_kqv_staged_arena(ctx0, v_segments" in backend_op_body
            or "qatq_build_segmented_kqv_graph_scheduled_arena(ctx0, gf, v_segments" in backend_op_body
            or "qatq_build_segmented_kqv_page_pool(ctx0, gf, sched, target_backend, v_segments" in backend_op_body
            or "qatq_segment_page_table" in backend_op_body
            or "qatq_page_table" in backend_op_body
        )
        and "page_offsets.reserve(k_segments.size() + 1)" in backend_op_body
        and "k_stride_token" in files.get("metal", "")
        and "v_stride_token" in files.get("metal", "")
        and (
            "token*args.k_stride_token" in files.get("metal", "")
            or "local_token*args.k_stride_token" in files.get("metal", "")
        )
        and (
            "token*args.v_stride_token" in files.get("metal", "")
            or "local_token*args.v_stride_token" in files.get("metal", "")
        )
        and (
            "k_stride_page" not in files.get("metal", "")
            or "page*args.k_stride_page" in files.get("metal", "")
        )
        and (
            "v_stride_page" not in files.get("metal", "")
            or "page*args.v_stride_page" in files.get("metal", "")
        )
    )
    native_backend_op_avoids_staged_arena = (
        native_backend_op_avoids_full_kv_packing
        and "qatq_build_segmented_kqv_staged_arena" not in backend_op_body
        and "qatq_build_segmented_kqv_graph_scheduled_arena" not in backend_op_body
        and "ggml_concat(ctx0, arena" not in files["graph"]
        and "staged_arena" not in backend_op_body
        and "graph_scheduled_arena" not in backend_op_body
    )
    native_backend_op_descriptor_path = (
        native_backend_op_avoids_staged_arena
        and "qatq_build_segmented_kqv_page_pool" not in backend_op_body
        and "LLAMA_QATQ_NATIVE_PAGE_STREAMING_DIRECT_SOURCE_FALLBACK" not in files["graph"]
        and (
            "qatq_segment_page_table" in backend_op_body
            or "qatq_page_table" in backend_op_body
            or "qatq_page_descriptor" in backend_op_body
            or "qatq_retained_page" in backend_op_body
        )
        and (
            "argument buffer" in all_sources
            or "descriptor" in files.get("metal", "")
            or "page_table" in files.get("metal", "")
        )
    )
    native_backend_op_token_page_table = (
        native_backend_op_avoids_full_kv_packing
        and "token_page_table" in backend_op_body
        and "token_local_table" in backend_op_body
        and "qatq_segmented_kqv_token_page_table" in backend_op_body
        and "qatq_segmented_kqv_token_local_table" in backend_op_body
        and "token_page_table[token]" in files.get("metal", "")
        and "token_local_table[token]" in files.get("metal", "")
        and "local_token >= page_token_offsets[page + 1u] - page_token_offsets[page]" in files.get("metal", "")
    )
    native_backend_op_consumes_staged_segments = (
        qatq_segmented_kqv_call_in_backend
        and "mctx_cur->get_k_page_source(ctx0, il)" not in backend_op_body
        and "mctx_cur->get_v_page_source(ctx0, il)" not in backend_op_body
        and (
            "k_segments[i].tensor" in backend_op_body
            or "k_segments.front().tensor" in backend_op_body
            or "k_segments.data()" in backend_op_body
            or "qatq_build_segmented_kqv_page_pool(ctx0, gf, sched, target_backend, k_segments" in backend_op_body
            or "qatq_build_segmented_kqv_staged_arena(ctx0, k_segments" in backend_op_body
            or "qatq_segment_page_table" in backend_op_body
            or "qatq_page_table" in backend_op_body
        )
        and (
            "v_segments[i].tensor" in backend_op_body
            or "v_segments.front().tensor" in backend_op_body
            or "v_segments.data()" in backend_op_body
            or "qatq_build_segmented_kqv_page_pool(ctx0, gf, sched, target_backend, v_segments" in backend_op_body
            or "qatq_build_segmented_kqv_staged_arena(ctx0, v_segments" in backend_op_body
            or "qatq_segment_page_table" in backend_op_body
            or "qatq_page_table" in backend_op_body
        )
    )
    attention_trace_hooked = (
        "qatq_trace_attention_use" in files["kv"]
        and "qatq-live-vram-attention-trace-v1" in files["kv"]
        and "qatq_trace_attention_use" in context_get_k_body
        and "qatq_trace_attention_use" in context_get_v_body
    )
    attention_event_trace_hooked = (
        "LLAMA_QATQ_ATTENTION_EVENT_TRACE" in files["kv"]
        and "trace_qatq_attention_lifecycle" in files["kv"]
        and "trace_qatq_attention_lifecycle" in files["kv_h"]
        and "trace_qatq_attention_lifecycle" in context_get_k_body
        and "trace_qatq_attention_lifecycle" in context_get_v_body
        and "--qatq-attention-event-trace" in files["simple"]
    )
    segmented_reduce_surface = (
        "qatq_segmented_kqv_online_page_summary_reduce" in all_sources
        or "ggml_segmented_kqv_online_page_summary" in all_sources
        or "ggml_segmented_kqv_global_softmax" in all_sources
        or "kernel_qatq_segmented_kqv" in all_sources
    )
    segmented_backend_op_route_surface = (
        "--qatq-native-page-streaming-attention-backend-op" in files["simple"]
        and 'LLAMA_QATQ_NATIVE_PAGE_STREAMING_ATTENTION_BACKEND", "backend-op"' in files["simple"]
        and "QATQ_SEGMENTED_KQV_BACKEND_CONTRACT_V1" in files["graph"]
        and "qatq-segmented-kqv-backend-contract-v1" in files["graph"]
        and "QATQ_SEGMENTED_KQV_BACKEND_MAX_SEGMENTS" in files["graph"]
        and "QATQ_SEGMENTED_KQV_BACKEND_MAX_PAGE_TOKENS" in files["graph"]
        and "QATQ_SEGMENTED_KQV_BACKEND_MAX_TOTAL_TOKENS" in files["graph"]
        and "qatq_segmented_kqv_backend_supported_dtype" in files["graph"]
        and "qatq_validate_segmented_kqv_backend_contract" in files["graph"]
        and "qatq_native_page_streaming_attention_backend_op_enabled" in files["graph"]
        and "qatq_native_page_streaming_backend_op" in files["graph"]
        and qatq_segmented_kqv_call_in_graph
        and "backend_scheduled_segmented_attention" in files["graph"]
        and "accelerated_runtime_attention_graph" in files["graph"]
    )
    segmented_backend_resource_bounds = (
        "QATQ_SEGMENTED_KQV_BACKEND_MAX_SEGMENTS" in files["graph"]
        and "QATQ_SEGMENTED_KQV_BACKEND_MAX_PAGE_TOKENS" in files["graph"]
        and "QATQ_SEGMENTED_KQV_BACKEND_MAX_TOTAL_TOKENS" in files["graph"]
        and "exceeded max segment count" in files["graph"]
        and "exceeded max page tokens" in files["graph"]
        and "exceeded max total tokens" in files["graph"]
    )
    segmented_backend_transient_pool_budget = (
        "--qatq-native-page-streaming-transient-pool-max-bytes" in files["simple"]
        and "LLAMA_QATQ_NATIVE_PAGE_STREAMING_TRANSIENT_POOL_MAX_BYTES" in files["simple"]
        and "LLAMA_QATQ_NATIVE_PAGE_STREAMING_TRANSIENT_POOL_MAX_BYTES" in files["graph"]
        and "QATQ_SEGMENTED_KQV_BACKEND_DEFAULT_TRANSIENT_POOL_MAX_BYTES" in files["graph"]
        and "transient page pool byte budget exceeded" in files["graph"]
        and "max_transient_pool_bytes - transient_pool_bytes" in files["graph"]
    )
    segmented_backend_selective_pool_default = (
        "LLAMA_QATQ_NATIVE_PAGE_STREAMING_SHARED_TILED_POOL" in files["kv"]
        and 'qatq_parse_u64_env("LLAMA_QATQ_NATIVE_PAGE_STREAMING_SHARED_TILED_POOL", 0)' in files["kv"]
        and "layer_pool_count = use_shared_tiled_pool ? std::max<size_t>(layers.size(), 1) : 1" in files["kv"]
    )
    native_flattened_flash_attention_route = (
        "--qatq-native-page-streaming-flatten-flash" in files["simple"]
        and "LLAMA_QATQ_NATIVE_PAGE_STREAMING_FLATTEN_FLASH" in files["simple"]
        and "LLAMA_QATQ_NATIVE_PAGE_STREAMING_FLATTEN_FLASH" in files["graph"]
        and "backend_scheduled_flattened_flash_attention" in files["graph"]
        and "cparams.flash_attn && kq_b == nullptr" in files["graph"]
        and "QATQ flattened native attention" in files["graph"]
        and "qatq_prepare_segmented_kqv_backend_page_table" in files["graph"]
        and "max_transient_pool_bytes - transient_pool_bytes" in files["graph"]
        and "qatq_flattened_flash_k" in files["graph"]
        and "qatq_flattened_flash_v" in files["graph"]
        and "build_attn_mha(q_stream, k_flat, v_flat" in files["graph"]
        and "qatq_filter_segments_for_stream" in files["graph"]
        and "stream_index" in files["graph"]
        and "stream_index" in files["kv"]
        and "trace_qatq_attention_page_segments(il, \"key\", k_segments, true, true, consumer)" in files["graph"]
        and "trace_qatq_attention_page_segments(il, \"value\", v_segments, true, true, consumer)" in files["graph"]
    )
    native_flattened_flash_stream_local_contract = (
        native_flattened_flash_attention_route
        and bool(flattened_flash_body)
        and "qatq_validate_segmented_kqv_backend_contract" not in flattened_flash_body
        and "if (n_stream > 1 && segment.stream_index != 0)" in flattened_flash_body
        and "total_tokens <= static_cast<int64_t>(QATQ_SEGMENTED_KQV_BACKEND_MAX_TOTAL_TOKENS)" in flattened_flash_body
        and "qatq_filter_segments_for_stream(k_segments, stream_index)" in flattened_flash_body
        and "qatq_filter_segments_for_stream(v_segments, stream_index)" in flattened_flash_body
        and "q->ne[2] % n_stream == 0" in flattened_flash_body
    )
    segmented_backend_dtype_bounds = (
        "qatq_segmented_kqv_backend_supported_dtype" in files["graph"]
        and "GGML_TYPE_F16" in files["graph"]
        and "GGML_TYPE_BF16" in files["graph"]
        and "GGML_TYPE_F32" in files["graph"]
        and "supports only f16, bf16, and f32 K/V pages" in files["graph"]
    )
    segmented_metal_kernel_source = (
        "kernel_qatq_segmented_kqv" in all_sources
        and "qatq_segmented_kqv_kernel_args" in all_sources
        and "online page-summary" in all_sources
    )
    segmented_backend_surface = (
        "GGML_OP_QATQ_SEGMENTED_KQV" in all_sources
        and "ggml_qatq_segmented_kqv" in files.get("ggml_h", "")
        and "ggml_qatq_segmented_kqv" in files.get("ggml_c", "")
        and "ggml_metal_library_get_pipeline_qatq_segmented_kqv" in all_sources
        and "ggml_metal_op_qatq_segmented_kqv" in all_sources
        and (
            "ggml_metal_device_supports_op" in all_sources
            or "op->src[3] != NULL && op->src[3]->type == GGML_TYPE_I32" in all_sources
        )
        and "QATQ_SEGMENTED_KQV" in all_sources
    )
    segmented_backend_graph_integration = (
        qatq_segmented_kqv_call_in_graph
        and "backend_scheduled_segmented_attention" in files["graph"]
        and "accelerated_runtime_attention_graph" in files["graph"]
    )
    segmented_backend_mask_support = (
        "struct ggml_tensor  * mask" in files.get("ggml_h", "")
        and "struct ggml_tensor  * mask" in files.get("ggml_c", "")
        and ("op->src[4]" in all_sources or "op->src[6]" in all_sources)
        and "qatq_segmented_kqv_mask_value" in files.get("metal", "")
        and "requires an explicit attention mask" in files["graph"]
        and "kq_mask" in files["graph"]
    )
    segmented_backend_typed_mask_support = (
        "kq_mask->type == GGML_TYPE_F32" in files["graph"]
        and "kq_mask->type == GGML_TYPE_F16" in files["graph"]
        and "mask->type == GGML_TYPE_F32" in files.get("ggml_c", "")
        and "mask->type == GGML_TYPE_F16" in files.get("ggml_c", "")
        and (
            "op->src[4]->type == GGML_TYPE_F32" in files.get("metal_ops_cpp", "")
            or "op->src[6]->type == GGML_TYPE_F32" in files.get("metal_ops_cpp", "")
        )
        and (
            "op->src[4]->type == GGML_TYPE_F16" in files.get("metal_ops_cpp", "")
            or "op->src[6]->type == GGML_TYPE_F16" in files.get("metal_ops_cpp", "")
        )
        and "mask_is_f16" in files.get("metal", "")
        and "device const char * mask" in files.get("metal", "")
        and "device const half *) mask" in files.get("metal", "")
    )
    segmented_backend_typed_kv_support = (
        "kernel_qatq_segmented_kqv_f16" in all_sources
        and "kernel_qatq_segmented_kqv_bf16" in all_sources
        and "GGML_TYPE_F16" in files.get("metal_device_m", "")
        and "GGML_TYPE_BF16" in files.get("metal_device_m", "")
        and "currently requires f32 K/V page tensors" not in files["graph"]
    )

    checks = [
        Check(
            "export.c_api",
            "llama_qatq_export_kv_cache_with_trace" in files["include"]
            and "llama_qatq_export_kv_cache_with_trace" in wrapper_impl,
            "trace-capable C API is declared and implemented",
        ),
        Check(
            "export.runner_flags",
            "--qatq-kv-export-dir" in files["simple"]
            and "--qatq-event-trace" in files["simple"]
            and "--qatq-attention-event-trace" in files["simple"]
            and "--qatq-attention-materialized-source-trace" in files["simple"]
            and "--qatq-attention-page-composed-source-trace" in files["simple"]
            and "--qatq-attention-persistent-page-source-trace" in files["simple"]
            and "--qatq-attention-page-segments-trace" in files["simple"]
            and "--qatq-native-page-streaming-preflight" in files["simple"]
            and "--qatq-native-page-streaming-contract" in files["simple"]
            and "--qatq-native-page-streaming-attention" in files["simple"]
            and "--qatq-native-page-streaming-attention-ggml" in files["simple"]
            and "--qatq-gpu-page-staging" in files["simple"]
            and "--qatq-live-persistent-page-pool-self-test" in files["simple"]
            and "--qatq-live-restore-slot-pressure-self-test" in files["simple"]
            and "--qatq-page-tokens" in files["simple"]
            and "--qatq-trace-hot-window-tokens" in files["simple"]
            and "--qatq-trace-prefetch-window-tokens" in files["simple"]
            and "--qatq-trace-next-required" in files["simple"]
            and "--qatq-live-page-self-test" in files["simple"]
            and "--qatq-model-id" in files["simple"],
            "patched llama-simple exposes export, event-trace, attention-event-trace, materialized-source, page-composed-source, persistent-page-source, page-segment, native preflight, executable native contract, native attention validation, GPU page-staging, persistent-page-pool, restore-slot pressure, page-token, hot/prefetch trace-scheduler, and page self-test flags",
        ),
        Check(
            "export.token_page_manifest",
            "LLAMA_QATQ_PAGE_TOKENS" in files["kv"]
            and ("\\\"token_start\\\"" in files["kv"] or '"token_start"' in files["kv"])
            and ("\\\"token_end\\\"" in files["kv"] or '"token_end"' in files["kv"]),
            "export path can split active KV tensors into bounded token-range files",
        ),
        Check(
            "export.manifest_allocator_attestation",
            "gpu_allocation_granularity" in files["kv"]
            and "gpu_context_bytes" in files["kv"]
            and "total_context_bytes" in files["kv"],
            "export manifest records allocator residency fields",
        ),
        Check(
            "export.event_trace_schema",
            "qatq-live-vram-event-trace-v1" in files["kv"]
            and "offload-committed" in files["kv"]
            and "restore-committed" in files["kv"]
            and "attention-use" in files["kv"],
            "export path writes QATQ event trace schema",
        ),
        Check(
            "export.event_trace_scheduler_alignment",
            "LLAMA_QATQ_TRACE_HOT_WINDOW_TOKENS" in files["kv"]
            and "LLAMA_QATQ_TRACE_PREFETCH_WINDOW_TOKENS" in files["kv"]
            and "LLAMA_QATQ_TRACE_NEXT_REQUIRED" in files["kv"]
            and "qatq_trace_should_offload_page" in files["kv"],
            "export-time event traces can align offload events with QATQ hot plus prefetch scheduler decisions",
        ),
        Check(
            "live.attention_read_hook_sites_identified",
            len(graph_get_sites) >= 2,
            f"found {len(graph_get_sites)} attention KV read sites in llama-graph.cpp",
        ),
        Check(
            "live.no_whole_tensor_view_for_paged_k",
            "ggml_view_4d" not in get_k_body,
            "diagnostic default K getter is no longer a plain whole-cache ggml_view_4d",
        ),
        Check(
            "live.no_whole_tensor_view_for_paged_v",
            "ggml_view_4d" not in get_v_body,
            "diagnostic default V getter is no longer a plain whole-cache ggml_view_4d",
        ),
        Check(
            "live.native_attention_bypasses_whole_tensor_getters",
            native_attention_bypasses_whole_tensor_getters,
            "native page-streaming attention bypasses default whole-tensor get_k/get_v sources for cold-page layers, with only a live_offloaded-guarded resident fast path allowed",
        ),
        Check(
            "live.page_allocator_present",
            "page_resident" in files["kv_h"]
            or "page_resident" in files["kv"]
            or ("restore_page" in files["kv_h"] and "evict_page" in files["kv_h"])
            or ("restore_page" in files["kv"] and "evict_page" in files["kv"]),
            "runtime exposes page residency metadata and evict/restore operations",
        ),
        Check(
            "live.logical_page_residency_manifest",
            "live_page_residency_granularity" in files["kv"] and "per-page" in files["kv"],
            "manifest distinguishes logical per-page residency from physical GPU allocation granularity",
        ),
        Check(
            "live.physical_page_tensor_self_test",
            "llama_qatq_live_physical_page_alloc_self_test" in files["include"]
            and "qatq_live_physical_page_alloc_self_test" in wrapper_impl
            and "qatq_live_physical_page_alloc_self_test" in files["kv"]
            and "ggml_backend_alloc_ctx_tensors_from_buft" in files["kv"]
            and "ggml_backend_tensor_set" in files["kv"]
            and "ggml_backend_tensor_get" in files["kv"]
            and "ggml_backend_buffer_free" in files["kv"],
            "adapter can allocate, round-trip bytes through, and free a page-sized non-host backend tensor",
        ),
        Check(
            "live.attention_path_page_tensor_self_test",
            "LLAMA_QATQ_ATTENTION_PAGE_TENSOR_SELF_TEST" in files["kv"]
            and "qatq_attention_page_tensor_self_test" in files["kv"]
            and "trace_qatq_attention_lifecycle" in files["kv"]
            and "ggml_backend_alloc_ctx_tensors_from_buft" in files["kv"]
            and "ggml_backend_tensor_set" in files["kv"]
            and "ggml_backend_tensor_get" in files["kv"]
            and "--qatq-attention-page-tensor-self-test" in files["simple"],
            "actual attention path can materialise real K/V page bytes into page-sized non-host backend tensors",
        ),
        Check(
            "live.attention_materialized_source_path",
            "LLAMA_QATQ_ATTENTION_MATERIALIZED_SOURCE_TRACE" in files["kv"]
            and "qatq_maybe_materialize_attention_source" in files["kv"]
            and "qatq_trace_attention_materialized_source" in files["kv"]
            and "ggml_cont(ctx, source)" in files["kv"]
            and "--qatq-attention-materialized-source-trace" in files["simple"],
            "adapter can make attention consume materialized K/V source tensors instead of returning the raw view directly",
        ),
        Check(
            "live.attention_page_composed_source_path",
            "LLAMA_QATQ_ATTENTION_PAGE_COMPOSED_SOURCE_TRACE" in files["kv"]
            and "qatq_maybe_page_compose_attention_source" in files["kv"]
            and "qatq_trace_attention_page_composed_source" in files["kv"]
            and "ggml_concat(ctx, composed, materialized_page" in files["kv"]
            and "--qatq-attention-page-composed-source-trace" in files["simple"],
            "adapter can make attention consume K/V sources composed from bounded materialized token pages",
        ),
        Check(
            "live.attention_persistent_page_source_path",
            "LLAMA_QATQ_ATTENTION_PERSISTENT_PAGE_SOURCE_TRACE" in files["kv"]
            and "qatq_maybe_persistent_page_attention_source" in files["kv"]
            and "qatq_trace_attention_persistent_page_source" in files["kv"]
            and "ggml_cpy(ctx, page_view, retained_tensor)" in files["kv"]
            and "ggml_concat(ctx, composed, copied_page" in files["kv"]
            and "--qatq-attention-persistent-page-source-trace" in files["simple"],
            "adapter can make attention consume retained backend page tensors filled through graph-native page copies",
        ),
        Check(
            "live.attention_page_source_composition_schema",
            '\\"composition\\":\\"ggml_concat\\"' in files["kv"]
            and '\\"native_page_streaming\\":false' in files["kv"],
            "page-source traces explicitly attest whether runtime attention is concat-composed or native page-streamed",
        ),
        Check(
            "live.attention_page_segment_api",
            "qatq_attention_page_segment" in files["kv_h"]
            and "get_k_page_segments" in files["kv_h"]
            and "get_v_page_segments" in files["kv_h"]
            and "qatq_build_attention_page_segments" in files["kv"]
            and "LLAMA_QATQ_ATTENTION_PAGE_SEGMENTS_TRACE" in files["kv"]
            and "qatq-live-vram-attention-page-segments-v1" in files["kv"]
            and "--qatq-attention-page-segments-trace" in files["simple"],
            "adapter exposes bounded K/V page segments from the actual attention read path before native attention consumes them",
        ),
        Check(
            "live.native_page_streaming_preflight",
            "LLAMA_QATQ_NATIVE_PAGE_STREAMING_PREFLIGHT" in files["graph"]
            and "qatq_native_page_streaming_preflight" in files["graph"]
            and "qatq_validate_native_page_segment_pairing" in files["graph"]
            and "mctx_cur->get_k_page_segments(ctx0, il)" in files["graph"]
            and "mctx_cur->get_v_page_segments(ctx0, il)" in files["graph"]
            and "ggml_segmented_kqv_preflight" in files["graph"]
            and "--qatq-native-page-streaming-preflight" in files["simple"],
            "graph-build path can preflight paired K/V page segments at the future native consumer boundary without claiming final native attention",
        ),
        Check(
            "live.native_segmented_kqv_contract",
            "LLAMA_QATQ_NATIVE_PAGE_STREAMING_CONTRACT" in files["graph"]
            and "qatq_native_page_streaming_contract_enabled" in files["graph"]
            and "qatq_validate_segmented_kqv_contract" in files["graph"]
            and "QATQ segmented KQV contract" in files["graph"]
            and "ggml_segmented_kqv_contract" in files["graph"]
            and "qatq_native_page_streaming_contract(ctx0, mctx_cur, q, kq_b, sinks, v_mla, il)" in files["graph"]
            and "--qatq-native-page-streaming-contract" in files["simple"]
            and "QATQ segmented KQV contract probe completed before backend execution" in files["graph"],
            "graph-build path exposes an executable fail-closed segmented K/Q/V contract hook that validates page geometry and unsupported feature boundaries independently of backend-op execution",
        ),
        Check(
            "live.no_concat_composed_attention_source",
            "ggml_concat(ctx, composed, materialized_page" not in files["kv"]
            and "ggml_concat(ctx, composed, copied_page" not in files["kv"],
            "diagnostic page-source composition no longer rebuilds logical K/V sources through ggml_concat",
        ),
        Check(
            "live.native_attention_bypasses_concat_page_sources",
            native_attention_bypasses_concat_page_sources,
            "native page-streaming attention route bypasses concat-composed page-source compatibility paths",
        ),
        Check(
            "live.native_page_streaming_attention_kernel",
            "LLAMA_QATQ_NATIVE_PAGE_STREAMING_ATTENTION" in files["graph"]
            and "LLAMA_QATQ_NATIVE_PAGE_STREAMING_ATTENTION_BACKEND" in files["simple"]
            and "qatq_native_page_streaming_attention_enabled" in files["graph"]
            and '"ggml_segmented_kqv"' in files["graph"]
            and "build_attn_mha_segmented_kqv" in files["graph"]
            and (
                "ggml_segmented_kqv_logits_bridge" in files["graph"]
                or "ggml_segmented_kqv_single" in files["graph"]
                or "kernel_qatq_segmented_kqv" in all_sources
            )
            and "LLAMA_QATQ_NATIVE_PAGE_STREAMING_ATTENTION" in files["simple"]
            and "--qatq-native-page-streaming-attention" in files["simple"]
            and "--qatq-native-page-streaming-attention-ggml" in files["simple"],
            "runtime has an executable non-concat ggml segmented KQ/V attention consumer for bounded native validation",
        ),
        Check(
            "live.native_multi_segment_graph_bridge",
            "ggml_segmented_kqv_logits_bridge" in files["graph"]
            and "ggml_segmented_kqv_global_softmax" in files["graph"]
            and "ggml_segmented_kqv_prob_page_" in files["graph"]
            and "ggml_concat(ctx0, kq_all, kq, 0)" in files["graph"],
            "runtime can consume multiple K/V page segments without rebuilding full K/V tensors by applying one global softmax over concatenated logits",
        ),
        Check(
            "live.native_multi_segment_attention_reduction",
            "QATQ segmented KQV multi-segment softmax reduction is not implemented yet" not in files["graph"]
            and "ggml_segmented_kqv_single" not in files["graph"]
            and segmented_reduce_surface,
            "native validation needs a correct multi-segment softmax reduction bridge, not only the absence of the single-segment guard",
        ),
        Check(
            "live.native_multi_segment_backend_surface",
            segmented_backend_surface,
            "production native live VRAM needs a backend-schedulable segmented K/Q/V op surface rather than a graph-only marker",
        ),
        Check(
            "live.native_multi_segment_backend_graph_integration",
            segmented_backend_graph_integration,
            "runtime graph must route bounded K/V page segments into the backend-schedulable QATQ segmented K/Q/V op",
        ),
        Check(
            "live.native_metal_segmented_kqv_kernel_source",
            segmented_metal_kernel_source,
            "Metal source contains the first online page-summary segmented K/Q/V kernel target",
        ),
        Check(
            "live.native_multi_segment_backend_op_route",
            segmented_backend_op_route_surface,
            "adapter exposes a guarded backend-op route for the fused online page-summary segmented attention kernel",
        ),
        Check(
            "live.native_backend_op_resource_bounds",
            segmented_backend_resource_bounds,
            "backend-op contract bounds segment count, page tokens, and total token fan-out",
        ),
        Check(
            "live.native_backend_op_transient_pool_byte_budget",
            segmented_backend_transient_pool_budget,
            "backend-op mode fails closed when graph-local transient K/V page pools exceed the configured byte budget",
        ),
        Check(
            "live.native_backend_op_dtype_bounds",
            segmented_backend_dtype_bounds,
            "backend-op contract rejects K/V dtypes outside f16, bf16, and f32",
        ),
        Check(
            "live.native_attention_not_cpu_custom_op",
            "ggml_custom_4d" not in files["graph"],
            "production native live VRAM must not rely on a CPU custom-op attention consumer",
        ),
        Check(
            "live.native_backend_op_mask_support",
            segmented_backend_mask_support,
            "backend-op mode passes the real attention mask into the segmented online softmax recurrence",
        ),
        Check(
            "live.native_backend_op_typed_mask_support",
            segmented_backend_typed_mask_support,
            "backend-op mode accepts f32 and f16 attention masks used by real flash-attention graph paths",
        ),
        Check(
            "live.native_backend_op_typed_kv_support",
            segmented_backend_typed_kv_support,
            "backend-op mode has f32, f16, and bf16 K/V page kernels",
        ),
        Check(
            "live.native_backend_op_avoids_full_kv_packing",
            native_backend_op_avoids_full_kv_packing,
            "backend-op mode streams page segments without first packing full K/V windows through ggml_concat",
        ),
        Check(
            "live.native_backend_op_multi_page_streaming",
            native_backend_op_multi_page_streaming,
            "backend-op mode supports multi-page graph-scheduled arena, retained page-pool, or native page descriptors instead of a single-page proof path",
        ),
        Check(
            "live.native_backend_op_token_page_table",
            native_backend_op_token_page_table,
            "backend-op mode carries explicit token-to-page and token-to-local page tables into the Metal kernel",
        ),
        Check(
            "live.native_backend_op_consumes_staged_segments",
            native_backend_op_consumes_staged_segments,
            "backend-op mode must feed explicit K/V page tensors, a retained page-pool, or an explicit page table into attention, not reopen the original whole-cache K/V source",
        ),
        Check(
            "live.native_backend_op_avoids_staged_arena",
            native_backend_op_avoids_staged_arena,
            "production backend-op mode must avoid graph-level ggml_concat staging arenas before the fused attention kernel",
        ),
        Check(
            "live.native_backend_op_descriptor_path",
            native_backend_op_descriptor_path,
            "production backend-op mode must consume retained page descriptors or a page table directly instead of staged arenas or direct-source diagnostics",
        ),
        Check(
            "live.native_backend_op_selective_pool_default",
            segmented_backend_selective_pool_default,
            "backend-op retained page pools default to per-layer allocation so selective cold-layer paging does not reserve every layer's pool unless explicitly requested",
        ),
        Check(
            "live.native_flattened_flash_attention_route",
            native_flattened_flash_attention_route,
            "eligible one-stream and stream-split multi-stream page tables can be flattened into llama.cpp's backend-scheduled Flash Attention path under the same transient page-pool byte budget",
        ),
        Check(
            "live.native_flattened_flash_stream_local_contract",
            native_flattened_flash_stream_local_contract,
            "flattened Flash validates total-token fan-out per stream and does not apply the aggregate segmented-backend cap before stream splitting",
        ),
        Check(
            "live.native_backend_op_unsupported_cases_fail_closed",
            segmented_backend_op_route_surface
            and "throw std::runtime_error" in files["graph"]
            and "does not support transposed V cache layout yet" in files["graph"]
            and "does not support attention soft cap yet" in files["graph"]
            and "requires an explicit attention mask" in files["graph"],
            "backend-op mode rejects unsupported layouts and dtypes explicitly instead of returning incorrect attention",
        ),
        Check(
            "live.gpu_page_staging_mode",
            "LLAMA_QATQ_GPU_PAGE_STAGING" in files["kv"]
            and "qatq_page_buft" in files["kv_h"]
            and "gpu_page_staging_bytes" in files["kv"]
            and "gpu_page_staging_tensors" in files["kv"]
            and "--qatq-gpu-page-staging" in files["simple"],
            "adapter can keep canonical KV off GPU while staging attention pages into retained accelerator tensors",
        ),
        Check(
            "live.persistent_page_pool_self_test",
            "llama_qatq_live_persistent_page_pool_self_test" in files["include"]
            and "qatq_live_persistent_page_pool_self_test" in wrapper_impl
            and "qatq_live_persistent_page_pool_self_test" in files["kv"]
            and "qatq_live_persistent_pages" in files["kv_h"]
            and "LLAMA_QATQ_LIVE_PERSISTENT_PAGE_POOL_TRACE" in files["kv"]
            and "qatq-live-vram-persistent-page-pool-v1" in files["kv"]
            and "--qatq-live-persistent-page-pool-self-test" in files["simple"]
            and "--qatq-live-persistent-page-pool-trace" in files["simple"],
            "adapter can retain a bounded pool of verified non-host K/V page tensors until context teardown",
        ),
        Check(
            "live.restore_slot_pressure_self_test",
            "llama_qatq_live_restore_slot_pressure_self_test" in files["include"]
            and "qatq_live_restore_slot_pressure_self_test" in wrapper_impl
            and "qatq_live_restore_slot_pressure_self_test" in files["kv"]
            and "qatq_live_restore_slot_pressure_self_test" in files["kv_h"]
            and "--qatq-live-restore-slot-pressure-self-test" in files["simple"]
            and "--qatq-live-restore-slot-pressure-max-bytes" in files["simple"]
            and "QATQ restore slot pressure self-test rejected" in files["kv"],
            "adapter can prove a bounded restore-slot resource limit rejects an oversized real accelerator page before allocation",
        ),
        Check(
            "live.physical_per_page_allocation_attestation",
            "gpu_allocation_granularity" in files["kv"]
            and (
                'qatq_gpu_allocation_granularity = "per-page"' in files["kv"]
                or '\\"gpu_allocation_granularity\\":\\"per-page\\"' in files["kv"]
            ),
            "manifest can attest physical per-page GPU allocation granularity, not only logical residency",
        ),
        Check(
            "live.attention_loop_trace_not_export_only",
            attention_trace_hooked,
            "attention path emits trace events from llama_kv_cache_context::get_k/get_v",
        ),
        Check(
            "live.attention_loop_lifecycle_trace",
            attention_event_trace_hooked,
            "attention path emits QATQ lifecycle events that can be checked by the event-trace verifier",
        ),
        Check(
            "live.backend_page_self_test",
            "llama_qatq_live_page_self_test" in files["include"]
            and "qatq_live_page_self_test" in wrapper_impl
            and "qatq_live_page_self_test" in files["kv"]
            and "ggml_backend_tensor_set" in files["kv"]
            and "qatq_tensor_slice_equals" in files["kv"],
            "adapter can snapshot, mutate, restore, and verify a real backend KV tensor page",
        ),
        Check(
            "live.persistent_kv_not_single_backend_buffer",
            "ggml_backend_alloc_ctx_tensors_from_buft" not in constructor_body,
            "persistent live-paged KV should not be allocated as one backend buffer per buffer type",
        ),
    ]
    return checks


def read_files_from_patch(path: Path) -> dict[str, str]:
    """Return per-file new-side snippets from a unified diff.

    This intentionally keeps context and added lines while dropping removed
    lines. It is good enough for repo-local readiness smoke checks over the
    pinned adapter patch. The applied-source mode remains the authoritative
    release gate because absence in a patch snippet is not proof that the final
    file lacks a construct.
    """

    key_by_path = {
        "include/llama.h": "include",
        "src/llama.cpp": "llama",
        "src/llama-context.cpp": "context",
        "src/llama-context.h": "context_h",
        "src/llama-kv-cache.cpp": "kv",
        "src/llama-kv-cache.h": "kv_h",
        "src/llama-graph.cpp": "graph",
        "ggml/include/ggml.h": "ggml_h",
        "ggml/src/ggml.c": "ggml_c",
        "ggml/src/ggml-metal/ggml-metal.metal": "metal",
        "ggml/src/ggml-metal/ggml-metal-device.h": "metal_device_h",
        "ggml/src/ggml-metal/ggml-metal-device.cpp": "metal_device_cpp",
        "ggml/src/ggml-metal/ggml-metal-device.m": "metal_device_m",
        "ggml/src/ggml-metal/ggml-metal-ops.h": "metal_ops_h",
        "ggml/src/ggml-metal/ggml-metal-ops.cpp": "metal_ops_cpp",
        "examples/simple/simple.cpp": "simple",
    }
    snippets = {key: [] for key in key_by_path.values()}
    current_key = None
    in_hunk = False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("+++ b/"):
            current_key = key_by_path.get(line[len("+++ b/") :])
            in_hunk = False
            continue
        if line.startswith("@@"):
            in_hunk = True
            continue
        if current_key is None or not in_hunk:
            continue
        if line.startswith("+") and not line.startswith("+++"):
            snippets[current_key].append(line[1:])
        elif line.startswith(" "):
            snippets[current_key].append(line[1:])
    return {key: "\n".join(lines) for key, lines in snippets.items()}


def native_ggml_live_ready(checks: list[Check]) -> bool:
    by_name = {check.name: check.passed for check in checks}
    return all(by_name.get(name) is True for name in LIVE_PAGING_REQUIRED_CHECKS)


def page_staging_ready(checks: list[Check]) -> bool:
    by_name = {check.name: check.passed for check in checks}
    return all(by_name.get(name) is True for name in PAGE_STAGING_REQUIRED_CHECKS) and (
        by_name.get("live.native_page_streaming_attention_kernel") is True
        or by_name.get("live.native_flags_fail_closed_without_consumer") is True
    )


def runtime_security_ready(checks: list[Check]) -> bool:
    by_name = {check.name: check.passed for check in checks}
    return all(by_name.get(name) is True for name in RUNTIME_SECURITY_REQUIRED_CHECKS)


def failed_required_checks(checks: list[Check], required_names: list[str]) -> list[str]:
    by_name = {check.name: check.passed for check in checks}
    return [name for name in required_names if by_name.get(name) is not True]


def non_required_failures(checks: list[Check]) -> list[str]:
    required = (
        set(LIVE_PAGING_REQUIRED_CHECKS)
        | set(PAGE_STAGING_REQUIRED_CHECKS)
        | set(RUNTIME_SECURITY_REQUIRED_CHECKS)
    )
    return [
        check.name
        for check in checks
        if not check.passed and check.name not in required
    ]


def function_body(text: str, qualified_name: str) -> str:
    start = text.find(qualified_name)
    if start < 0:
        return ""
    brace = text.find("{", start)
    if brace < 0:
        return ""
    depth = 0
    for index in range(brace, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[brace : index + 1]
    return text[brace:]


def snippet_between(text: str, start_marker: str, end_marker: str) -> str:
    start = text.find(start_marker)
    if start < 0:
        return ""
    end = text.find(end_marker, start + len(start_marker))
    if end < 0:
        return text[start:]
    return text[start:end]


def read_text(path: Path) -> str:
    require(path.exists(), f"required source file missing: {path}")
    return path.read_text(encoding="utf-8")


def git_commit(root: Path) -> str:
    completed = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, text=True, capture_output=True)
    if completed.returncode != 0:
        return "unknown"
    return completed.stdout.strip()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        print("interrupted", file=sys.stderr)
        raise SystemExit(130)
