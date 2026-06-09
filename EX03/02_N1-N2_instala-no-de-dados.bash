#!/usr/bin/env bash
#
# EX03 - 02 - Maquina: N1 e N2 (rodar nos DOIS nos de dados)
#   N1 = 192.168.1.2 , N2 = 192.168.1.3
# Instala o MySQL Cluster como no de dados (ndbd + servidor SQL), cria o my.cnf
# apontando para o gerenciador, sobe o no e configura o boot.
# Idempotente e tolerante a progresso parcial.
#
set -uo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"
export DEBIAN_FRONTEND=noninteractive

log()  { echo "[EX03/02][N1/N2] $*"; }
erro() { echo "[EX03/02][N1/N2][ERRO] $*" >&2; exit 1; }

# apt tolerante ao lock (apt-daily/unattended-upgrades roda no boot) e a falhas
# transitorias de rede: espera e tenta de novo por ~2.5 min.
apt_seguro() {
  local i
  for i in $(seq 1 30); do
    if apt-get -o Acquire::Retries=3 "$@"; then return 0; fi
    log "apt ocupado (lock) ou falhou. Nova tentativa em 5s (${i}/30)..."
    sleep 5
  done
  erro "apt nao concluiu 'apt-get $*' apos varias tentativas."
}

VER="7.3.26"
PKG="mysql-cluster-gpl-${VER}-linux-glibc2.12-x86_64"
URL="https://downloads.mysql.com/archives/get/p/14/file/${PKG}.tar.gz"
MGM_IP="192.168.1.1"

log "iniciando instalacao do no de dados..."

# 1) Grupo e usuario mysql (so cria se nao existir).
getent group mysql >/dev/null 2>&1 || groupadd mysql
id -u mysql       >/dev/null 2>&1 || useradd -g mysql mysql

# 2) Download e extracao em /usr/local (pula se ja extraido).
cd /usr/local || erro "nao consegui entrar em /usr/local"
if [ ! -d "/usr/local/${PKG}" ]; then
  if [ ! -f "${PKG}.tar.gz" ]; then
    log "baixando o MySQL Cluster (arquivo grande, pode demorar)..."
    wget --tries=3 -O "${PKG}.tar.gz" "$URL" || erro "falha no download do MySQL Cluster"
  fi
  log "extraindo o pacote..."
  tar -zxf "${PKG}.tar.gz" || erro "falha ao extrair o pacote"
fi

# 3) Link simbolico /usr/local/mysql -> pasta extraida.
ln -sfn "/usr/local/${PKG}" /usr/local/mysql

# 4) Bibliotecas necessarias (precisa de internet).
log "atualizando o apt e instalando as bibliotecas..."
apt_seguro update
apt_seguro install -y libdata-dump-perl libaio1 libaio-dev

# 5) Inicializa o data dir do MySQL (so na primeira vez; usa scripts/ ainda nao movidos).
if [ ! -d /usr/local/mysql/data/mysql ]; then
  log "inicializando o data dir do MySQL..."
  ( cd /usr/local/mysql && scripts/mysql_install_db --user=mysql --datadir=/usr/local/mysql/data ) \
    || erro "mysql_install_db falhou (libaio instalada?)"
fi

# 6) Donos da instalacao.
cd /usr/local/mysql/ || erro "nao consegui entrar em /usr/local/mysql"
chown -R root:mysql .
chown -R mysql data

# 7) Servico mysql.server (servidor SQL).
log "instalando o servico mysql.server..."
cp -f support-files/mysql.server /etc/init.d/
chmod 755 /etc/init.d/mysql.server
update-rc.d mysql.server defaults >/dev/null 2>&1 || true

# 8) Binarios no PATH. So na primeira vez: depois /usr/local/mysql/bin vira link
#    para /usr/bin, entao a checagem de symlink evita repetir/quebrar.
if [ ! -L /usr/local/mysql/bin ]; then
  log "movendo os binarios para /usr/bin..."
  ( cd /usr/local/mysql/bin && mv -f ./* /usr/bin/ ) || erro "falha ao mover os binarios"
  rm -rf /usr/local/mysql/bin
  ln -s /usr/bin /usr/local/mysql/bin
fi

# 9) my.cnf apontando para o gerenciador.
log "gravando /etc/my.cnf ..."
cat > /etc/my.cnf <<EOF
[mysqld]
ndbcluster
# IP do no gerenciador do cluster
ndb-connectstring=${MGM_IP}
# Descomente se o ndbd der erro relacionado ao servico Angel:
#innodb_buffer_pool_size=8M
#innodb_use_sys_malloc=1

[mysql_cluster]
# IP do no gerenciador do cluster
ndb-connectstring=${MGM_IP}
EOF

# 10) Diretorio de dados do no (DataDir referenciado no config.ini do gerenciador).
mkdir -p /var/lib/mysql-cluster

# 11) Sobe o no de dados (ndbd). O --initial zera os dados do no, entao so roda na
#     primeira vez; nas proximas usa ndbd normal (marcador controla isso).
if pgrep -x ndbd >/dev/null 2>&1; then
  log "ndbd ja esta em execucao."
elif [ -f /var/lib/mysql-cluster/.ndbd-inicializado ]; then
  log "iniciando ndbd..."
  ndbd || erro "ndbd nao iniciou (o gerenciador na MGM esta no ar?)"
else
  log "iniciando ndbd --initial (primeira vez)..."
  ndbd --initial || erro "ndbd --initial nao iniciou (rode o passo 3.1 na MGM antes)"
  touch /var/lib/mysql-cluster/.ndbd-inicializado
fi

# 12) Sobe o servidor SQL (mysqld).
log "subindo o servidor SQL (mysqld)..."
/etc/init.d/mysql.server start || /etc/init.d/mysql.server restart || erro "mysqld nao subiu"

# 13) Hardening basico do MySQL, conforme o exercicio (root sem senha).
#     Respostas: [enter] senha atual vazia, 'n' nao definir senha de root, [enter] no resto.
#     E opcional: se falhar, o script segue.
if command -v mysql_secure_installation >/dev/null 2>&1; then
  log "rodando o hardening basico do MySQL..."
  printf '\nn\n\n\n\n\n\n' | mysql_secure_installation >/dev/null 2>&1 || log "hardening pulado (segue o jogo)."
fi

# 14) Servico de boot do ndbd.
log "configurando o servico de boot (ndbd)..."
cat > /etc/init.d/ndbd <<'EOF'
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
ndbd
EOF
chmod +x /etc/init.d/ndbd
systemctl daemon-reload >/dev/null 2>&1 || true
systemctl enable ndbd >/dev/null 2>&1 || true

log "concluido. Na MGM rode 'ndb_mgm -e show' (ou bash run 3.3) para ver este no conectado."
