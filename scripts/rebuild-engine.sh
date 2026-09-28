#!/usr/bin/env bash
# rebuild-engine.sh
# Rebuild the processing Rust crate (src/lib.rs) and copy the result into the
# build cache so that the next sketch build picks up the new library without
# having to re-scaffold the cache project.
#
# Usage:
#   ./scripts/rebuild-engine.sh               # debug build (faster)
#   ./scripts/rebuild-engine.sh --release     # release build
#   ./scripts/rebuild-engine.sh --wasm        # WASM target (requires wasm-pack)
#   ./scripts/rebuild-engine.sh --check       # cargo check only (no codegen)
#
# What it does:
#   1. cargo build [--release] inside RustMode/ (the processing crate).
#   2. Copies the compiled rlib + all dep artifacts into the persistent build
#      cache at ~/.processing/rust-build-cache/.processing-lib/ so every
#      sketch's Cargo.lock stays satisfied without re-downloading crates.
#   3. Touches a version stamp so RustBuild knows it can skip re-scaffolding
#      the processing/ sub-crate on the next run.
#
# The cache layout mirrors what RustBuild.scaffoldProject() creates:
#   ~/.processing/rust-build-cache/
#     .lib-stamp            <- mtime updated here; RustBuild checks this
#     <sketch-name>/
#       processing/         <- copy of the crate; overwritten by rebuild-engine

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUSTMODE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

PROFILE="dev"
WASM=0
CHECK=0
EXTRA_FLAGS=()

# ── Argument parsing ───────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)  PROFILE="release"; shift ;;
    --wasm)     WASM=1;            shift ;;
    --check)    CHECK=1;           shift ;;
    --)         shift; EXTRA_FLAGS+=("$@"); break ;;
    *)          EXTRA_FLAGS+=("$1"); shift ;;
  esac
done

# ── Sanity checks ─────────────────────────────────────────────────────────────

if ! command -v cargo &>/dev/null; then
  echo "ERROR: cargo not found. Install Rust from https://rustup.rs."
  exit 1
fi

if [[ $WASM -eq 1 ]]; then
  if ! command -v wasm-pack &>/dev/null; then
    echo "ERROR: wasm-pack not found."
    echo "  cargo install wasm-pack"
    exit 1
  fi
  if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo "Adding wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
  fi
fi

# ── Build ─────────────────────────────────────────────────────────────────────

cd "$RUSTMODE_DIR"

if [[ $CHECK -eq 1 ]]; then
  echo "cargo check  ($RUSTMODE_DIR)"
  cargo check "${EXTRA_FLAGS[@]}"
  echo
  echo "ok   check passed."
  exit 0
fi

if [[ $WASM -eq 1 ]]; then
  echo "wasm-pack build  (--features wasm --no-default-features)"
  wasm-pack build \
    --target web \
    --out-dir pkg \
    -- \
    --features wasm \
    --no-default-features \
    "${EXTRA_FLAGS[@]}"
  echo
  echo "ok   WASM package written to $RUSTMODE_DIR/pkg/"
  exit 0
fi

RELEASE_FLAG=()
[[ "$PROFILE" == "release" ]] && RELEASE_FLAG=(--release)

echo "cargo build  profile=$PROFILE  ($RUSTMODE_DIR)"
cargo build "${RELEASE_FLAG[@]}" "${EXTRA_FLAGS[@]}"

# ── Update the build cache ────────────────────────────────────────────────────
# Copy the compiled processing crate source into every existing cache project's
# processing/ subdir so the next sketch build uses the freshly built library
# without re-fetching or re-scaffolding.

CACHE_ROOT="${XDG_DATA_HOME:-$HOME/.processing}/rust-build-cache"

if [ -d "$CACHE_ROOT" ]; then
  UPDATED=0
  for sketch_dir in "$CACHE_ROOT"/*/; do
    dest="$sketch_dir/processing"
    if [ -d "$dest" ]; then
      # Copy only the two files RustBuild.scaffoldProject() places there.
      cp -f "$RUSTMODE_DIR/Cargo.toml"  "$dest/Cargo.toml"
      cp -f "$RUSTMODE_DIR/src/lib.rs"  "$dest/src/lib.rs"
      UPDATED=$((UPDATED + 1))
    fi
  done

  # Touch the library stamp so RustBuild skips re-copying on next run.
  touch "$CACHE_ROOT/.lib-stamp"

  if [[ $UPDATED -gt 0 ]]; then
    echo
    echo "updated processing crate in $UPDATED cached sketch project(s) under $CACHE_ROOT"
  fi
else
  echo
  echo "note: no build cache at $CACHE_ROOT yet (it is created on first sketch run)."
fi

echo
TARGET_DIR="$RUSTMODE_DIR/target/$PROFILE"
echo "ok   build complete  ->  $TARGET_DIR"
