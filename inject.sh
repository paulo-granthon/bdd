#!/usr/bin/env bash
#
# inject.sh - roda no HOST. Baixa o binário do bdd e abre o injetor (TUI), que
# acha as VMs, deixa você marcar MGM/N1/N2 e instala o bdd em cada uma por SSH.
#
set -euo pipefail

URL="https://paulo-granthon.github.io/bdd/bin"
DIR="$(cd "$(dirname "$0")" && pwd)"
BIN="${DIR}/bdd"

fetch() {
  if command -v curl >/dev/null 2>&1; then curl -fSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then wget -O "$2" "$1"
  else echo "[inject] preciso de curl ou wget no host." >&2; return 2; fi
}

echo "[inject] baixando o bdd..."
# A release 'latest' e re-publicada a cada push; logo apos um push o asset pode
# dar 404 por ~1 min enquanto sobe. Por isso tentamos algumas vezes.
ok=0
for try in 1 2 3 4 5 6; do
  if fetch "$URL" "$BIN" && [ -s "$BIN" ]; then ok=1; break; fi
  rc=$?
  [ "$rc" = 2 ] && exit 1
  echo "[inject] ainda nao disponivel (tentativa ${try}), aguardando 10s..."
  sleep 10
done
[ "$ok" = 1 ] || { echo "[inject] nao consegui baixar o binario." >&2; exit 1; }

chmod +x "$BIN"
exec "$BIN" inject
