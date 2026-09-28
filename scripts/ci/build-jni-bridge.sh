#!/usr/bin/env bash
# Build the java-tree-sitter JNI bridge from the bundled C source.
# Usage: build-jni-bridge.sh <cc> <shared_flags> <jni_os_include> <out_dir>
# e.g.:  build-jni-bridge.sh gcc "-shared -fPIC" linux /tmp/out
set -euo pipefail

CC="${1}"
SHARED_FLAGS="${2}"
JNI_OS="${3}"
OUT_DIR="${4}"

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
JNI_SRC="$REPO_ROOT/jni"
TS_RUST_SRC="$REPO_ROOT/vendor/tree-sitter-rust/src"

# Locate JDK headers
JAVA_INCLUDE="$JAVA_HOME/include"

# Determine library name from OS
case "$(uname -s)" in
  Darwin) LIBNAME="libjava-tree-sitter.dylib" ;;
  MINGW*|MSYS*|CYGWIN*) LIBNAME="java-tree-sitter.dll" ;;
  *) LIBNAME="libjava-tree-sitter.so" ;;
esac

mkdir -p "$OUT_DIR"

$CC $SHARED_FLAGS \
  -O2 \
  -o "$OUT_DIR/$LIBNAME" \
  "$JNI_SRC/jts_impl.c" \
  "$TS_RUST_SRC/parser.c" \
  "$TS_RUST_SRC/scanner.c" \
  -I"$JNI_SRC" \
  -I"$TS_RUST_SRC" \
  -I"$JAVA_INCLUDE" \
  -I"$JAVA_INCLUDE/$JNI_OS" \
  -Wl,-rpath,'\$ORIGIN'

echo "Built: $OUT_DIR/$LIBNAME"
