#!/usr/bin/env bash
set -euo pipefail

pos_root="$(cd "$(dirname "$0")/.." && pwd)"
out_dir="${pos_root}/web/src/wasm/generated"
scratch_dir="$(mktemp -d)"
trap 'rm -rf "${scratch_dir}"' EXIT

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "rebuild-wasm: wasm-pack is not installed." >&2
  echo "  Install with:  bash ../scripts/install-wasm-pack.sh" >&2
  exit 1
fi

cd "${pos_root}/core"
wasm-pack build --target web --release --out-dir "${scratch_dir}" --out-name janus_pos --no-opt -- --features wasm

mkdir -p "${out_dir}"
for f in janus_pos.js janus_pos.d.ts janus_pos_bg.wasm janus_pos_bg.wasm.d.ts; do
  cp "${scratch_dir}/${f}" "${out_dir}/${f}"
done
echo "rebuild-wasm: refreshed ${out_dir}"
