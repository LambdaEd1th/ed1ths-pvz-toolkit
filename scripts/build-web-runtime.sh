#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

wasm-pack build "$repo_root/crates/pam/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/pam-tool/assets/pam/worker/pkg" \
  --out-name pam_viewer_worker \
  --locked

wasm-pack build "$repo_root/crates/pam/renderer" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/pam-tool/assets/pam/renderer/pkg" \
  --out-name pam_viewer_renderer \
  --locked

wasm-pack build "$repo_root/crates/rsb/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/rsb-tool/assets/rsb/worker/pkg" \
  --out-name rsb_preview_worker \
  --locked

wasm-pack build "$repo_root/crates/rton/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/rton-tool/assets/rton/worker/pkg" \
  --out-name rton_editor_worker \
  --locked

wasm-pack build "$repo_root/crates/wem/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/wem-tool/assets/wem/worker/pkg" \
  --out-name wem_audio_worker \
  --locked

find "$repo_root/crates/tools" -path '*/pkg/.gitignore' -delete
