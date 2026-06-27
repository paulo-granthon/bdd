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

# Sem a OVA do Cassandra: se não estiver instalado, instala (Java 8 + Cassandra
# 3.11 pelo repositório oficial Apache). Idempotente.
instala_cassandra() {
  command -v cqlsh >/dev/null 2>&1 && [ -f "$YAML" ] && return 0
  log "Cassandra não encontrado; instalando (Java 8 + Cassandra 3.11)..."
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -y || true
  apt-get install -y openjdk-8-jdk-headless apt-transport-https ca-certificates curl gnupg \
    || apt-get install -y openjdk-8-jre-headless ca-certificates curl \
    || true
  echo "deb https://debian.cassandra.apache.org 311x main" > /etc/apt/sources.list.d/cassandra.sources.list
  # chave do repo (apt-key é depreciado mas funciona no 16.04); tenta 2 origens.
  { curl -fsSL https://downloads.apache.org/cassandra/KEYS \
      || curl -fsSL https://archive.apache.org/dist/cassandra/KEYS; } 2>/dev/null | apt-key add - 2>/dev/null || true
  apt-get update -y || true
  apt-get install -y cassandra || erro "falha ao instalar o Cassandra via apt (veja a saída acima)."
  log "Cassandra instalado."
}
instala_cassandra
[ -f "$YAML" ] || erro "não achei $YAML mesmo após a instalação. Veja a saída acima."

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
