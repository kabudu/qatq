#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
output_root=${1:-"$repo_root/validation/oracle-v0.4.1/evidence"}
sage_image=${SAGE_IMAGE:-sagemath/sagemath:10.6@sha256:19995db6194f4a4bab18ce9a88556fd15b9ed5e916b4504fefe618a7796ddbdb}

case "$output_root" in
  /*) ;;
  *) output_root="$repo_root/$output_root" ;;
esac

if [ -e "$output_root" ]; then
  echo "refusing to overwrite existing evidence directory: $output_root" >&2
  exit 2
fi

cargo build --manifest-path "$repo_root/Cargo.toml" --release --features oracle --bin qatq-oracle
mkdir -p "$output_root"
python3 "$repo_root/scripts/oracle_validation/export_corpus.py" \
  --corpus "$repo_root/validation/oracle-v0.4.1/corpus.json" \
  --qatq-oracle "$repo_root/target/release/qatq-oracle" \
  --output "$output_root"

docker run --rm \
  --platform linux/amd64 \
  --network none \
  --user "$(id -u):$(id -g)" \
  --group-add 1000 \
  --env HOME=/tmp \
  --env PYTHONDONTWRITEBYTECODE=1 \
  --volume "$repo_root:/workspace:ro" \
  "$sage_image" \
  sage -python /workspace/scripts/oracle_validation/test_reproduce.py

docker run --rm \
  --platform linux/amd64 \
  --network none \
  --user "$(id -u):$(id -g)" \
  --group-add 1000 \
  --env HOME=/tmp \
  --volume "$repo_root:/workspace:ro" \
  --volume "$output_root:/evidence" \
  "$sage_image" \
  sage -python /workspace/scripts/oracle_validation/reproduce.py \
  --manifest /evidence/export-manifest.json \
  --root /evidence \
  --output /evidence/sagemath-results.json

python3 "$repo_root/scripts/oracle_validation/finalize_results.py" \
  --root "$output_root" \
  --qatq-oracle "$repo_root/target/release/qatq-oracle" \
  --sage-results "$output_root/sagemath-results.json" \
  --sage-image "$sage_image" \
  --output "$output_root/results.json"

echo "independent validation evidence: $output_root/results.json"
