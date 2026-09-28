#!/usr/bin/env bash
# Builds lib/<platform>/libjava-tree-sitter.so for a Processing mode
# (LuaMode, RustMode, SchemeMode, ...). Drop it in <Mode>/native/.
#
# The Java side of java-tree-sitter comes from the maven jar pinned in
# build.gradle, so the native side has to be built from the SAME tag or
# JNI_OnLoad goes looking for classes that don't exist and crashes the IDE.
#
# Only one grammar is compiled in. The Language enum still asks every
# other language for its version/fields/symbols at class init, so we patch
# those three natives to return 0 for a null language instead of segfaulting.
#
# tree-sitter core is the copy pinned by java-tree-sitter, compiled in
# statically with hidden visibility, so it can't clash with the system
# libtree-sitter or with CppMode/RustMode's copies.
#
# Usage: native/build.sh [grammar]   default grammar: lua
#   e.g. native/build.sh rust, native/build.sh scheme
#   LIB_PLATFORM=linux-x86-64 native/build.sh rust   (override lib/<dir> name)
# Work dir: native/.build, safe to delete.

set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(dirname "$HERE")
WORK=${WORK:-$HERE/.build}
JTS_REPO=https://github.com/seart-group/java-tree-sitter
GRAMMAR=${1:-lua}
GRAMMAR_REPO=tree-sitter-$GRAMMAR
GRAMMAR_MACRO=TS_LANGUAGE_$(echo "$GRAMMAR" | tr 'a-z-' 'A-Z_')
GRAMMAR_JNI=Java_ch_usi_si_seart_treesitter_Language_$(echo "$GRAMMAR" | sed -E 's/-(.)/\U\1/g')

# Keep the native build locked to whatever build.gradle embeds.
JTS_VERSION=$(grep -oP "ch\.usi\.si\.seart:java-tree-sitter:\K[0-9.]+" "$ROOT/build.gradle" || true)
[ -n "$JTS_VERSION" ] || { echo "build.gradle doesn't depend on ch.usi.si.seart:java-tree-sitter" >&2; exit 1; }
JTS_TAG="v$JTS_VERSION"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)  PLATFORM=linux-x86_64 ;;
  Linux-aarch64) PLATFORM=linux-aarch64 ;;
  *) echo "unsupported platform: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

JAVA_HOME=${JAVA_HOME:-$(dirname "$(dirname "$(readlink -f "$(which java)")")")}
[ -f "$JAVA_HOME/include/jni.h" ] || { echo "no jni.h under $JAVA_HOME (need a JDK)" >&2; exit 1; }

PLATFORM=${LIB_PLATFORM:-$PLATFORM}
echo "java-tree-sitter $JTS_TAG + $GRAMMAR_REPO -> lib/$PLATFORM"

# ── sources ────────────────────────────────────────────────────────────────
JTS=$WORK/jts-$JTS_VERSION
if [ ! -d "$JTS/.git" ]; then
  rm -rf "$JTS"
  git clone -q --depth=1 --branch "$JTS_TAG" "$JTS_REPO" "$JTS"
fi
git -C "$JTS" submodule update -q --init --depth=1 tree-sitter "$GRAMMAR_REPO"
grep -q "$GRAMMAR_MACRO" "$JTS/lib/ch_usi_si_seart_treesitter_Language.cc" || {
  echo "java-tree-sitter $JTS_TAG has no $GRAMMAR_MACRO" >&2; exit 1; }

# ── null guards so the Language enum can init invalid languages safely ────
LANG_CC=$JTS/lib/ch_usi_si_seart_treesitter_Language.cc
for fn in ts_language_version ts_language_symbol_count ts_language_field_count; do
  sed -i "s|return (jint)$fn((const TSLanguage \*)id);|return id ? (jint)$fn((const TSLanguage *)id) : 0;|" "$LANG_CC"
done
for fn in ts_language_version ts_language_symbol_count ts_language_field_count; do
  grep -q "return id ? (jint)$fn" "$LANG_CC" || { echo "null-guard patch failed for $fn" >&2; exit 1; }
done

# ── compile ────────────────────────────────────────────────────────────────
OBJ=$WORK/obj-$PLATFORM
rm -rf "$OBJ" && mkdir -p "$OBJ"
CFLAGS="-fPIC -O2 -fvisibility=hidden"

TS=$JTS/tree-sitter/lib
gcc $CFLAGS -I "$TS/include" -I "$TS/src" -c "$TS/src/lib.c" -o "$OBJ/tree-sitter.o"

case $GRAMMAR in   # same layout rules as upstream build.py
  markdown)   G_SRC=$JTS/$GRAMMAR_REPO/$GRAMMAR_REPO/src ;;
  ocaml)      G_SRC=$JTS/$GRAMMAR_REPO/ocaml/src ;;
  tsx)        G_SRC=$JTS/$GRAMMAR_REPO/tsx/src ;;
  typescript) G_SRC=$JTS/$GRAMMAR_REPO/typescript/src ;;
  *)          G_SRC=$JTS/$GRAMMAR_REPO/src ;;
esac
gcc $CFLAGS -I "$G_SRC" -c "$G_SRC/parser.c" -o "$OBJ/grammar-parser.o"
if [ -f "$G_SRC/scanner.c" ]; then
  gcc $CFLAGS -I "$G_SRC" -c "$G_SRC/scanner.c" -o "$OBJ/grammar-scanner.o"
elif [ -f "$G_SRC/scanner.cc" ]; then
  g++ $CFLAGS -I "$G_SRC" -c "$G_SRC/scanner.cc" -o "$OBJ/grammar-scanner.o"
fi

OUT=$OBJ/libjava-tree-sitter.so
g++ -shared $CFLAGS -Wl,-z,defs -D"$GRAMMAR_MACRO" \
  -I "$JTS/include" -I "$TS/include" \
  -I "$JAVA_HOME/include" -I "$JAVA_HOME/include/linux" \
  "$JTS"/lib/*.cc "$OBJ"/*.o \
  -o "$OUT"
strip --strip-unneeded "$OUT"

# ── sanity checks ──────────────────────────────────────────────────────────
# (capture first: grep -q + pipefail would trip on SIGPIPE)
SYMS=$(nm -D --defined-only "$OUT")
grep -q " JNI_OnLoad$" <<<"$SYMS" || { echo "JNI_OnLoad missing" >&2; exit 1; }
grep -q " T $GRAMMAR_JNI$" <<<"$SYMS" || { echo "$GRAMMAR_JNI missing" >&2; exit 1; }
if grep -q " T ts_" <<<"$SYMS"; then
  echo "tree-sitter symbols leaked into the dynamic table" >&2; exit 1
fi
if grep -q tree-sitter <<<"$(ldd "$OUT")"; then
  echo "linked against a system libtree-sitter, expected static" >&2; exit 1
fi

# ── install ────────────────────────────────────────────────────────────────
DEST=$ROOT/lib/$PLATFORM
mkdir -p "$DEST"
install -m 755 "$OUT" "$DEST/libjava-tree-sitter.so"
rm -f "$DEST/libtree-sitter-$GRAMMAR.so"   # grammar now lives inside libjava-tree-sitter

ls -la "$DEST/libjava-tree-sitter.so"
echo "done. now: SKIP_RUNNER=1 gradle modeJar"
