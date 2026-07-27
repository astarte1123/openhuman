#!/usr/bin/env bash
# audit.sh — Run security vulnerability scan on Rust dependencies.
# Usage: bash scripts/audit.sh
# Exit code 0 = clean, non-zero = vulnerabilities found.
#
# Ignored advisories are documented below with rationale.
# To add a new ignore: append --ignore RUSTSEC-XXXX-YYYY to IGNORE_FLAGS
# and add a comment explaining why.
set -euo pipefail

cd "$(dirname "$0")/.."

echo "[audit] Checking cargo-audit installation..."
if ! command -v cargo-audit &>/dev/null; then
  echo "[audit] Installing cargo-audit..."
  cargo install cargo-audit --locked
fi

# Known false positives — transitive deps outside direct project control.
# Format: --ignore RUSTSEC-ID  # crate: reason
IGNORE_FLAGS=(
  # GTK3 bindings — pulled in transitively by Tauri's Linux GTK backend.
  # Unmaintained because gtk-rs moved to GTK4; fix is upstream in Tauri.
  --ignore RUSTSEC-2024-0411  # gdkwayland-sys: Tauri Linux transitive dep
  --ignore RUSTSEC-2024-0412  # gdk: Tauri Linux transitive dep
  --ignore RUSTSEC-2024-0413  # atk: Tauri Linux transitive dep
  --ignore RUSTSEC-2024-0414  # gdkx11-sys: Tauri Linux transitive dep
  --ignore RUSTSEC-2024-0415  # gtk: Tauri Linux transitive dep
  --ignore RUSTSEC-2024-0416  # atk-sys: Tauri Linux transitive dep
  --ignore RUSTSEC-2024-0417  # gdkx11: Tauri Linux transitive dep
  --ignore RUSTSEC-2024-0418  # gdk-sys: Tauri Linux transitive dep
  --ignore RUSTSEC-2024-0419  # gtk3-macros: Tauri Linux transitive dep
  --ignore RUSTSEC-2024-0420  # gtk-sys: Tauri Linux transitive dep
  # glib VariantStrIter unsoundness — same Tauri/GTK3 transitive chain.
  # Not reachable from project code; fix is upstream in Tauri.
  --ignore RUSTSEC-2024-0429  # glib: Tauri Linux transitive dep, not reachable
  # bincode — pulled in by hnsw_rs (vector search). No updated hnsw_rs release.
  # Advisory is a maintenance warning, not a CVE.
  --ignore RUSTSEC-2025-0141  # bincode: hnsw_rs transitive dep
  # proc-macro-error — build-time dep via glib-macros / yew-macro.
  # Compile-time only, no exploit surface.
  --ignore RUSTSEC-2024-0370  # proc-macro-error: build-time transitive dep
  # unic-* — transitive; no CVE, purely maintenance warnings.
  --ignore RUSTSEC-2025-0075  # unic-char-range: transitive dep
  --ignore RUSTSEC-2025-0080  # unic-common: transitive dep
  --ignore RUSTSEC-2025-0081  # unic-char-property: transitive dep
  --ignore RUSTSEC-2025-0098  # unic-ucd-version: transitive dep
  --ignore RUSTSEC-2025-0100  # unic-ucd-ident: transitive dep
)

echo "[audit] Running dependency vulnerability scan..."
cargo audit -f Cargo.lock "${IGNORE_FLAGS[@]}"

echo "[audit] Scan complete — no HIGH/CRITICAL vulnerabilities found."
