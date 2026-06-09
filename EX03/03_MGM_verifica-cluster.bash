#!/usr/bin/env bash
#
# EX03 - 03 - Maquina: MGM (192.168.1.1)
# Mostra o status do cluster: quais nos estao conectados.
#
set -euo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

echo "[EX03/03][MGM] status do cluster:"
ndb_mgm -e show
