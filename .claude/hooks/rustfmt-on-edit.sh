#!/bin/sh
# PostToolUse (Edit|Write) : formate le fichier .rs modifié. Silencieux si rustfmt est absent.
f=$(jq -r '.tool_input.file_path // empty')
case "$f" in
  *.rs) command -v rustfmt >/dev/null 2>&1 && [ -f "$f" ] && rustfmt --edition 2024 -q "$f" ;;
esac
exit 0
