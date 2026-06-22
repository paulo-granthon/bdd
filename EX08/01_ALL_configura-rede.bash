#!/usr/bin/env bash
#
# EX08 - 01 - Máquina: depende do papel (BDD_ROLE)
# Rede interna das VMs do Cassandra: IP estático em enp0s8 e hostname
# (node1/node2/node3). Mantém enp0s3 em DHCP (acesso pelo host).
# Idempotente.
#
set -uo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

log()  { echo "[EX08/01][rede] $*"; }
erro() { echo "[EX08/01][rede][ERRO] $*" >&2; exit 1; }

case "${BDD_ROLE:-}" in
  mgm) IP=192.168.1.1; HOST=node1 ;;
  n1)  IP=192.168.1.2; HOST=node2 ;;
  n2)  IP=192.168.1.3; HOST=node3 ;;
  *)   erro "papel da máquina indefinido. Rode 'bdd id' antes." ;;
esac

log "configurando $HOST com IP interno $IP ..."
echo "$HOST" > /etc/hostname
hostnamectl set-hostname "$HOST" 2>/dev/null || hostname "$HOST"
grep -q "127.0.1.1[[:space:]]\+${HOST}\b" /etc/hosts || echo "127.0.1.1 ${HOST}" >> /etc/hosts

cat > /etc/network/interfaces <<EOF
auto lo
iface lo inet loopback

auto enp0s3
iface enp0s3 inet dhcp

auto enp0s8
iface enp0s8 inet static
address $IP
netmask 255.255.255.0
gateway 0.0.0.0
EOF

log "reiniciando a interface interna..."
{ systemctl restart networking 2>/dev/null; } || { ifdown enp0s8 2>/dev/null; ifup enp0s8 2>/dev/null; } || true
log "feito. $HOST = $IP (interna)."
