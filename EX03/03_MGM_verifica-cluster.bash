#!/usr/bin/env bash
#
# EX03 - 03 - Maquina: MGM (192.168.1.1)
# Mostra o status do cluster: quais nos estao conectados.
#
set -uo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

echo "[EX03/03][MGM] status do cluster:"
if ! command -v ndb_mgm >/dev/null 2>&1; then
  echo "[EX03/03][MGM][ERRO] ndb_mgm nao encontrado. Rode o passo 3.1 antes." >&2
  exit 1
fi
ndb_mgm -e show || { echo "[EX03/03][MGM][ERRO] nao consegui falar com o gerenciador (ndb_mgmd no ar?)." >&2; exit 1; }
