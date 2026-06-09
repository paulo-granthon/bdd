#!/usr/bin/env bash
#
# EX03 - 01 - Maquina: MGM (192.168.1.1)
# Instala o no gerenciador (ndb_mgm / ndb_mgmd), cria o config.ini, configura o
# servico de boot e sobe o gerenciador do cluster.
# Idempotente: pode rodar de novo sem quebrar progresso ja feito.
#
set -euo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

VER="7.3.26"
PKG="mysql-cluster-gpl-${VER}-linux-glibc2.12-x86_64"
URL="https://downloads.mysql.com/archives/get/p/14/file/${PKG}.tar.gz"

echo "[EX03/01][MGM] instalando o gerenciador do cluster..."

# 1) Binarios do gerenciador (pula se ja estiverem instalados).
if command -v ndb_mgmd >/dev/null 2>&1 && command -v ndb_mgm >/dev/null 2>&1; then
  echo "[i] ndb_mgm/ndb_mgmd ja instalados, pulando download."
else
  mkdir -p /usr/src/mysql-mgm
  cd /usr/src/mysql-mgm
  [ -f "${PKG}.tar.gz" ] || wget -O "${PKG}.tar.gz" "$URL"
  tar -zxf "${PKG}.tar.gz"
  cp "${PKG}/bin/ndb_mgm"  /usr/bin/
  cp "${PKG}/bin/ndb_mgmd" /usr/bin/
  chmod 755 /usr/bin/ndb_mg*
  cd /
  rm -rf /usr/src/mysql-mgm
  echo "[i] binarios copiados para /usr/bin."
fi

# 2) Diretorio de dados/config do gerenciador.
mkdir -p /var/lib/mysql-cluster

# 3) config.ini (sobrescreve com o conteudo correto).
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
echo "[i] config.ini gravado."

# 4) Servico de boot do gerenciador.
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
  echo "[i] ndb_mgmd ja esta em execucao."
else
  ndb_mgmd -f /var/lib/mysql-cluster/config.ini --configdir=/var/lib/mysql-cluster/
  echo "[i] ndb_mgmd iniciado."
fi
systemctl daemon-reload >/dev/null 2>&1 || true
systemctl enable ndb_mgmd >/dev/null 2>&1 || true

echo "[EX03/01][MGM] concluido. Confira depois com: ndb_mgm -e show"
