#!/usr/bin/env bash
#
# EX08 - 02 - Máquina: depende do papel (BDD_ROLE)
# Configura o cassandra.yaml para o cluster multi-node, limpa os dados antigos
# e reinicia o serviço. Rode primeiro no node1 (seed), depois node2 e node3.
# Idempotente.
#
set -uo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

log()  { echo "[EX08/02][cassandra] $*"; }
erro() { echo "[EX08/02][cassandra][ERRO] $*" >&2; exit 1; }

case "${BDD_ROLE:-}" in
  mgm) IP=192.168.1.1 ;;
  n1)  IP=192.168.1.2 ;;
  n2)  IP=192.168.1.3 ;;
  *)   erro "papel da máquina indefinido. Rode 'bdd id' antes." ;;
esac
YAML=/etc/cassandra/cassandra.yaml
[ -f "$YAML" ] || erro "não achei $YAML (o Cassandra está instalado nesta VM?)"

log "parando o Cassandra e limpando dados antigos..."
{ service cassandra stop 2>/dev/null || /etc/init.d/cassandra stop 2>/dev/null; } || true
rm -rf /var/lib/cassandra/* 2>/dev/null || true

log "ajustando $YAML (seed=192.168.1.1, listen/rpc=$IP)..."
sed -i "s/^cluster_name:.*/cluster_name: 'BDD FATEC'/" "$YAML"
sed -i "s/\(- seeds:\).*/\1 \"192.168.1.1\"/" "$YAML"
sed -i "s/^listen_address:.*/listen_address: $IP/" "$YAML"
sed -i "s/^rpc_address:.*/rpc_address: $IP/" "$YAML"
sed -i "s/^endpoint_snitch:.*/endpoint_snitch: SimpleSnitch/" "$YAML"

log "subindo o Cassandra..."
{ service cassandra start 2>/dev/null || /etc/init.d/cassandra start 2>/dev/null; } || erro "não consegui iniciar o Cassandra"
log "feito. (no node1 rode antes dos outros; depois confira com 'bdd 8.3')"
