#!/usr/bin/env bash
#
# EX03 - 05 - Maquina: N2 (192.168.1.3)
# Verifica que os dados criados no N1 estao replicados.
# Obs: o CREATE DATABASE nao e replicado entre os nos SQL (e local), mas a tabela
# NDBCLUSTER e descoberta automaticamente. Por isso recriamos so o schema aqui.
#
set -euo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

echo "[EX03/05][N2] verificando replicacao dos dados..."
mysql -u root <<'SQL'
CREATE DATABASE IF NOT EXISTS clusterdb;
USE clusterdb;
SELECT * FROM funcionarios;
SQL

echo "[EX03/05][N2] se a tabela acima mostra Ana/Bruno/Carla, a replicacao esta OK."
