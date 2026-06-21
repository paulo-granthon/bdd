#!/usr/bin/env bash
#
# EX02 - 02 - Máquina: depende do papel (BDD_ROLE: mgm/n1/n2)
# Verifica que esta máquina enxerga as outras duas na rede interna (ping).
#
set -uo pipefail

log() { echo "[EX02/02][rede] $*"; }

case "${BDD_ROLE:-}" in
  mgm) PARES="192.168.1.2 192.168.1.3" ;;
  n1)  PARES="192.168.1.1 192.168.1.3" ;;
  n2)  PARES="192.168.1.1 192.168.1.2" ;;
  *)   PARES="192.168.1.1 192.168.1.2 192.168.1.3" ;;
esac

ok=1
for ip in $PARES; do
  if ping -c1 -W2 "$ip" >/dev/null 2>&1; then
    log "ping $ip OK"
  else
    log "ping $ip FALHOU"
    ok=0
  fi
done

if [ "$ok" = 1 ]; then
  log "esta máquina enxerga as outras na rede interna."
else
  echo "[EX02/02][rede][ERRO] alguma máquina não respondeu ao ping (confira o passo 2.1 nas três)." >&2
  exit 1
fi
