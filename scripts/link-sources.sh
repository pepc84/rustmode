#!/usr/bin/env bash
# link-sources.sh
# Symlink RustMode's Java source files into the processing4 source tree so
# the IDE's Gradle build picks them up.
#
# Usage:
#   ./scripts/link-sources.sh                          # uses $PROCESSING4_DIR env var
#   PROCESSING4_DIR=~/Projects/processing4 ./scripts/link-sources.sh
#
# This script lives in RustMode/scripts/, so RustMode's own root is one
# directory up from wherever this script actually is.

set -euo pipefail

RUSTMODE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC_DIR="$RUSTMODE_DIR/src/java"

if [ ! -d "$SRC_DIR" ]; then
  echo "ERROR: $SRC_DIR does not exist."
  echo "Expected the real .java source files to live there."
  exit 1
fi

if [ -z "${PROCESSING4_DIR:-}" ]; then
  echo "ERROR: set PROCESSING4_DIR to your processing4 checkout root."
  echo "  e.g.  PROCESSING4_DIR=~/Projects/processing4 $0"
  exit 1
fi

DEST_DIR="$PROCESSING4_DIR/java/src/processing/mode/rust"
mkdir -p "$DEST_DIR"

echo "real source:  $SRC_DIR"
echo "linking into: $DEST_DIR"
echo

shopt -s nullglob
SOURCE_FILES=("$SRC_DIR"/*.java)
shopt -u nullglob

if [ ${#SOURCE_FILES[@]} -eq 0 ]; then
  echo "WARNING: no .java files found in $SRC_DIR -- nothing to link."
  exit 0
fi

linked=0
skipped=0
warned=0

for f in "${SOURCE_FILES[@]}"; do
  name="$(basename "$f")"
  target="$DEST_DIR/$name"

  if [ ! -e "$f" ]; then
    echo "WARNING: $name disappeared from source -- skipping."
    warned=$((warned+1))
    continue
  fi

  if [ -L "$target" ]; then
    current_link="$(readlink "$target")"
    if [ "$current_link" == "$f" ]; then
      skipped=$((skipped+1))
      continue
    fi
    echo "Replacing stale/incorrect symlink: $name (was -> $current_link)"
    rm "$target"
  elif [ -e "$target" ]; then
    backup="$target.bak"
    n=1
    while [ -e "$backup" ]; do
      backup="$target.bak.$n"
      n=$((n+1))
    done
    echo "Backing up existing $name -> $(basename "$backup")"
    mv "$target" "$backup"
  fi

  ln -s "$f" "$target"
  echo "linked $name"
  linked=$((linked+1))
done

echo
echo "Done: $linked linked, $skipped already correct, $warned warnings."
echo "$DEST_DIR now points at the real source in $SRC_DIR."
echo "Build the jar by running ./gradlew from $PROCESSING4_DIR."
