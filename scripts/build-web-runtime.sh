#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

wasm-pack build "$repo_root/crates/runtime/pam/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/pam-tool/assets/pam/worker/pkg" \
  --out-name pam_editor_worker \
  --locked
rm "$repo_root/crates/tools/pam-tool/assets/pam/worker/pkg/.gitignore"

wasm-pack build "$repo_root/crates/runtime/pam/renderer" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/pam-tool/assets/pam/renderer/pkg" \
  --out-name pam_editor_renderer \
  --locked
rm "$repo_root/crates/tools/pam-tool/assets/pam/renderer/pkg/.gitignore"

wasm-pack build "$repo_root/crates/runtime/rsb/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/rsb-tool/assets/rsb/worker/pkg" \
  --out-name rsb_preview_worker \
  --locked

wasm-pack build "$repo_root/crates/runtime/rsb-patch/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/rsb-patch-tool/assets/rsb-patch/worker/pkg" \
  --out-name rsb_patch_worker \
  --locked

wasm-pack build "$repo_root/crates/runtime/rton/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/rton-tool/assets/rton/worker/pkg" \
  --out-name rton_editor_worker \
  --locked

wasm-pack build "$repo_root/crates/runtime/smf/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/smf-tool/assets/smf/worker/pkg" \
  --out-name smf_worker \
  --locked

wasm-pack build "$repo_root/crates/runtime/compiled-text/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/compiled-text-tool/assets/compiled-text/worker/pkg" \
  --out-name compiled_text_worker \
  --locked

wasm-pack build "$repo_root/crates/runtime/wem/worker" \
  --target web \
  --release \
  --out-dir "$repo_root/crates/tools/wem-tool/assets/wem/worker/pkg" \
  --out-name wem_audio_worker \
  --locked

find "$repo_root/crates/tools" -path '*/pkg/.gitignore' -delete
