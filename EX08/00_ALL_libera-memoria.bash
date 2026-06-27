#!/usr/bin/env bash
#
# EX08 - 00 (OPCIONAL) - Máquina: todas (papel via BDD_ROLE)
# Libera memória parando os serviços do MySQL Cluster antes de subir o Cassandra.
# Só faz sentido se você está reaproveitando as VMs do MySQL Cluster. Se as VMs
# do Cassandra forem dedicadas, pule este passo. Idempotente e seguro.
#
set -uo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

log() { echo "[EX08/00][libera-memoria] $*"; }

log "parando serviços do MySQL Cluster para liberar RAM (ignora se não existirem)..."
case "${BDD_ROLE:-}" in
  mgm)
    pkill -x ndb_mgmd 2>/dev/null && log "ndb_mgmd parado." || log "ndb_mgmd não estava rodando."
    ;;
  n1|n2)
    { service mysql stop 2>/dev/null || /etc/init.d/mysql stop 2>/dev/null; } && log "mysql parado." || log "mysql não estava rodando."
    pkill -x ndbmtd 2>/dev/null && log "ndbmtd parado." || true
    pkill -x ndbd   2>/dev/null && log "ndbd parado."   || log "ndbd/ndbmtd não estavam rodando."
    ;;
  *)
    log "papel indefinido; tentando parar tudo que houver..."
    { service mysql stop 2>/dev/null || /etc/init.d/mysql stop 2>/dev/null; } || true
    pkill -x ndb_mgmd 2>/dev/null || true
    pkill -x ndbmtd 2>/dev/null || true
    pkill -x ndbd 2>/dev/null || true
    ;;
esac

log "memória livre agora:"
free -h 2>/dev/null || free
log "feito. (opcional) Siga com 'bdd 8.1' e 'bdd 8.2'."
