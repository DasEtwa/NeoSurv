#!/usr/bin/env bash
set -euo pipefail

SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${WINDOWS_TARGET:-/mnt/z/Workspace/game-engine}"

mkdir -p "$TARGET_DIR"

rsync -av --delete \
  --exclude '.git/' \
  --exclude 'target/' \
  --exclude '**/target/' \
  --exclude '.idea/' \
  --exclude '.vscode/' \
  --exclude 'legacy/cpp-bootstrap/build/' \
  "$SRC_DIR/" "$TARGET_DIR/"

echo "✅ Synced: $SRC_DIR -> $TARGET_DIR"
