#!/bin/bash
# No raw hex colors outside styles.css (design token enforcement).
set -e
if grep -rEn "#[0-9a-fA-F]{3,8}\b" apps/desktop/src --include='*.ts' --include='*.tsx' | grep -v 'styles.css'; then
  echo "raw hex color outside styles.css" >&2
  exit 1
fi
