#!/usr/bin/env bash
#
# EX03 - 05 - Maquina: N2 (192.168.1.3)
# Verifica que os dados criados no N1 estao replicados.
# Obs: o CREATE DATABASE nao e replicado entre os nos SQL (e local), mas a tabela
# NDBCLUSTER e descoberta automaticamente. Por isso recriamos so o schema aqui.
#
set -uo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

echo "[EX03/05][N2] verificando a replicacao dos dados..."
if ! mysql -u root <<'SQL'
CREATE DATABASE IF NOT EXISTS clusterdb;
USE clusterdb;
SELECT * FROM funcionarios;
SQL
then
  echo "[EX03/05][N2][ERRO] o mysql falhou (servidor SQL no ar? passo 3.4 rodou no N1?)." >&2
  exit 1
fi

echo "[EX03/05][N2] se a tabela acima mostra Ana/Bruno/Carla, a replicacao esta OK."
