#!/bin/bash
set -euo pipefail
cd -- "$(dirname -- "$0")"
if ! command -v python3 >/dev/null 2>&1; then
  echo "Python 3 is required with Xcode Command Line Tools."
  exit 1
fi
python3 scripts/build-macos.py
printf '\nBuild complete. The universal app ZIP is in the dist folder.\n'
