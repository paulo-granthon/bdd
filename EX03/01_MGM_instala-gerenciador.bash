#!/usr/bin/env bash
#
# EX03 - 01 - Maquina: MGM (192.168.1.1)
# Instala o no gerenciador (ndb_mgm / ndb_mgmd), cria o config.ini, configura o
# servico de boot e sobe o gerenciador do cluster.
# Idempotente e tolerante a progresso parcial.
#
set -uo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

log()  { echo "[EX03/01][MGM] $*"; }
erro() { echo "[EX03/01][MGM][ERRO] $*" >&2; exit 1; }

# Baixa $1 (url) em $2 (arquivo). O wget do Ubuntu 16.04 usa GnuTLS e as vezes
# falha o TLS com CDNs novos; por isso tenta wget, wget TLS1.2 e curl (OpenSSL).
baixar_arquivo() {
  wget --tries=3 -O "$2" "$1" && return 0
  log "wget falhou (provavel TLS); tentando wget --secure-protocol=TLSv1_2..."
  wget --tries=3 --secure-protocol=TLSv1_2 -O "$2" "$1" && return 0
  if command -v curl >/dev/null 2>&1; then
    log "tentando curl..."
    curl -fSL --retry 3 -o "$2" "$1" && return 0
  fi
  return 1
}

VER="7.3.26"
PKG="mysql-cluster-gpl-${VER}-linux-glibc2.12-x86_64"
URL="https://downloads.mysql.com/archives/get/p/14/file/${PKG}.tar.gz"

log "iniciando instalacao do gerenciador do cluster..."

# 1) Binarios do gerenciador (pula se ja estiverem instalados).
if command -v ndb_mgmd >/dev/null 2>&1 && command -v ndb_mgm >/dev/null 2>&1; then
  log "ndb_mgm/ndb_mgmd ja instalados, pulando download."
else
  log "baixando e instalando os binarios do gerenciador..."
  mkdir -p /usr/src/mysql-mgm
  cd /usr/src/mysql-mgm || erro "nao consegui entrar em /usr/src/mysql-mgm"
  if [ ! -f "${PKG}.tar.gz" ]; then
    baixar_arquivo "$URL" "${PKG}.tar.gz" \
      || erro "falha no download do MySQL Cluster (TLS? rode: apt-get install -y curl ca-certificates)"
  fi
  tar -zxf "${PKG}.tar.gz" || erro "falha ao extrair o pacote"
  cp "${PKG}/bin/ndb_mgm"  /usr/bin/ || erro "falha ao copiar ndb_mgm"
  cp "${PKG}/bin/ndb_mgmd" /usr/bin/ || erro "falha ao copiar ndb_mgmd"
  chmod 755 /usr/bin/ndb_mg*
  cd /
  rm -rf /usr/src/mysql-mgm
  log "binarios copiados para /usr/bin."
fi

# 2) Diretorio de dados/config do gerenciador.
mkdir -p /var/lib/mysql-cluster

# 3) config.ini (sobrescreve com o conteudo correto).
log "gravando /var/lib/mysql-cluster/config.ini ..."
cat > /var/lib/mysql-cluster/config.ini <<'EOF'
# MySQL Cluster Configuration
[NDBD DEFAULT]
NoOfReplicas=2
DataMemory=80M
IndexMemory=18M

[MYSQLD DEFAULT]

[NDB_MGMD DEFAULT]
DataDir=/var/lib/mysql-cluster

[TCP DEFAULT]

# No gerenciador (este host)
[NDB_MGMD]
NodeId=1
HostName=192.168.1.1

# Nos de dados
[NDBD]
HostName=192.168.1.2
DataDir=/var/lib/mysql-cluster

[NDBD]
HostName=192.168.1.3
DataDir=/var/lib/mysql-cluster

# Um [MYSQLD] por no de dados
[MYSQLD]
[MYSQLD]
EOF

# 4) Servico de boot do gerenciador.
log "configurando o servico de boot (ndb_mgmd)..."
cat > /etc/init.d/ndb_mgmd <<'EOF'
#!/bin/sh
### BEGIN INIT INFO
# Provides: startup
# Required-Start: $remote_fs $syslog
# Required-Stop: $remote_fs $syslog
# Default-Start: 2 3 4 5
# Default-Stop: 0 1 6
# Short-Description: Start daemon at boot time
# Description: Enable service provided by daemon.
### END INIT INFO
ndb_mgmd -f /var/lib/mysql-cluster/config.ini --configdir=/var/lib/mysql-cluster/
EOF
chmod +x /etc/init.d/ndb_mgmd

# 5) Sobe o gerenciador agora (se ainda nao estiver rodando) e habilita no boot.
if pgrep -x ndb_mgmd >/dev/null 2>&1; then
  log "ndb_mgmd ja esta em execucao."
else
  log "iniciando ndb_mgmd..."
  ndb_mgmd -f /var/lib/mysql-cluster/config.ini --configdir=/var/lib/mysql-cluster/ \
    || erro "ndb_mgmd nao iniciou (confira o config.ini)"
fi
systemctl daemon-reload >/dev/null 2>&1 || true
systemctl enable ndb_mgmd >/dev/null 2>&1 || true

log "concluido. Confira com: ndb_mgm -e show"
