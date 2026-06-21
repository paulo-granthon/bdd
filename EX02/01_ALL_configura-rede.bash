#!/usr/bin/env bash
#
# EX02 - 01 - Máquina: depende do papel (BDD_ROLE: mgm/n1/n2)
# Configura a rede interna do cluster: IP estático em enp0s8 e o hostname.
# Mantém enp0s3 em DHCP (placa em bridge, usada pelo host via SSH).
# Idempotente: sobrescreve os arquivos com o conteúdo correto.
#
set -uo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

log()  { echo "[EX02/01][rede] $*"; }
erro() { echo "[EX02/01][rede][ERRO] $*" >&2; exit 1; }

case "${BDD_ROLE:-}" in
  mgm) IP=192.168.1.1; HOST=mgm ;;
  n1)  IP=192.168.1.2; HOST=n1  ;;
  n2)  IP=192.168.1.3; HOST=n2  ;;
  *)   erro "papel da máquina indefinido. Rode 'bdd id' antes." ;;
esac

log "configurando $HOST com IP interno $IP ..."

echo "$HOST" > /etc/hostname
hostnamectl set-hostname "$HOST" 2>/dev/null || hostname "$HOST"

cat > /etc/network/interfaces <<EOF
auto lo
iface lo inet loopback

# Placa em bridge (acesso pelo host / internet) - DHCP
auto enp0s3
iface enp0s3 inet dhcp

# Rede interna do cluster - IP estático
auto enp0s8
iface enp0s8 inet static
address $IP
netmask 255.255.255.0
gateway 0.0.0.0
EOF

log "reiniciando a interface interna..."
{ systemctl restart networking 2>/dev/null; } || { ifdown enp0s8 2>/dev/null; ifup enp0s8 2>/dev/null; } || true

log "feito. $HOST = $IP (interna), enp0s3 segue em DHCP."
