#!/usr/bin/env bash
#
# inject.sh - roda no HOST. Baixa o binário do bdd e abre o injetor (TUI), que
# acha as VMs, deixa você marcar MGM/N1/N2 e instala o bdd em cada uma por SSH.
#
set -euo pipefail

URL="https://github.com/paulo-granthon/bdd/releases/latest/download/bdd"
DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="${DIR}/bdd"

echo "[inject] baixando o bdd..."
if command -v curl >/dev/null 2>&1; then
  curl -fSL "$URL" -o "$BIN"
elif command -v wget >/dev/null 2>&1; then
  wget -O "$BIN" "$URL"
else
  echo "[inject] preciso de curl ou wget no host." >&2
  exit 1
fi
chmod +x "$BIN"

exec "$BIN" inject
