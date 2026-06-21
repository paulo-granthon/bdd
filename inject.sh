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
  # silencioso no sucesso (-s); mostra só erros (-S). Mantém o terminal limpo
  # para a TUI aparecer logo abaixo do prompt.
  if command -v curl >/dev/null 2>&1; then curl -fsS "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then wget -q -O "$2" "$1"
  else echo "[inject] preciso de curl ou wget no host." >&2; return 2; fi
}

ok=0
for try in 1 2 3 4 5 6; do
  if fetch "$URL" "$BIN" && [ -s "$BIN" ]; then ok=1; break; fi
  rc=$?
  [ "$rc" = 2 ] && exit 1
  echo "[inject] binario ainda nao disponivel (tentativa ${try}), aguardando 10s..." >&2
  sleep 10
done
[ "$ok" = 1 ] || { echo "[inject] nao consegui baixar o binario." >&2; exit 1; }

chmod +x "$BIN"
exec "$BIN" inject
