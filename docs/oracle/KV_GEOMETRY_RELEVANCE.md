# KV Geometry Relevance Gate

## Decision

The preregistered study did not demonstrate a credible product application for further Capacity Oracle theorem expansion. Real post-RoPE KV captures occupied a high-correlation regime, and the experiment supplied no defensible external `required_states` or required-separation guarantee. Capacity Oracle remains a narrow, validated research capability. QATQ engineering should prioritize live cold-page KV compression and restoration evidence.

## Preregistered corpus

The machine-readable corpus contains 36 profiles from 12 captures. It covers Qwen2.5 and Phi3 model families; factual, conversational, and code prompts; observed context lengths of 7, 30, and 67 tokens; f16 and bf16 KV caches; early, middle, and late layers; every exported KV head in those layers; key and value tensors; and `layer-head-token`, `layer-token`, and 32-token `layer-head-chunk` partitions.

All captures came from the pinned QATQ llama.cpp export path. The corpus records the runtime binary SHA-256, model SHA-256, prompt SHA-256, capture SHA-256, deterministic seed 42, and one-million-pair ceiling. Raw captures remain outside the repository because they are reproducible and substantially larger than the result corpus. Published result bundles and their aggregate manifest are in [`validation/geometry-v0.4.2`](../../validation/geometry-v0.4.2).

## Observations

Across the 180 post-RoPE key groups in the primary `layer-head-token` profiles, maximum cosine similarity ranged from 0.871583 to 0.998824, with median 0.954426. Median group p95 cosine similarity was 0.897252. No meaningful normalized key family had maximum inner product at or below zero.

Across 180 value groups, maximum cosine similarity ranged from 0.220832 to 1.0, with median 0.916848. Median group p95 cosine similarity was 0.652637. No zero or non-finite vectors were observed in these primary profiles.

Every pairwise population in this compact corpus was computed exactly. Overall profile status is `APPROXIMATE` because spectral concentration includes deterministic power iteration. Descriptive sign and threshold binarizations do not define application-level state separation.

## Limitations

The pinned exporter exposed stored post-RoPE keys, not matched pre-RoPE keys, so this study cannot compare those representations. It uses one runtime, two quantized model artifacts, three selected layers, compact contexts, and one deterministic generation per cell. Results do not establish stability across hardware, larger models, longer production contexts, sampling seeds, or task outcomes.

The observed vector count is not a capacity requirement. No retrieval, continuation-equivalence, semantic-state, angular-margin, or collision-budget requirement was supplied. Therefore the study cannot create a scientifically justified capture bridge even if a finite mathematical bound happened to be numerically small.

The v0.4.1 separate-software reproduction remains complete. External human coding-theory review remains outstanding and is not claimed by this report.

## Consequence

Do not add positive-inner-product cap certificates, Gegenbauer optimization, construction search, automatic distortion-to-separation conversion, or an Oracle-integrated capture bridge on the basis of this corpus. Reopen theorem work only if a future preregistered application supplies external capacity and separation requirements and representative states enter a regime where a finite bound can make non-vacuous decisions.

FREEZE: no credible QATQ product application was demonstrated
