#!/usr/bin/env bash
#
# EX05 - 01 - Máquina: N1
# Cria a tabela ALUNO com fragmentação horizontal por chave (PARTITION BY KEY)
# usando ENGINE=NDBCLUSTER e insere 15 registros.
#
set -uo pipefail

echo "[EX05/01][N1] criando a tabela ALUNO particionada e inserindo 15 registros..."
mysql -u root <<'SQL'
CREATE DATABASE IF NOT EXISTS clusterdb;
USE clusterdb;
CREATE TABLE IF NOT EXISTS aluno (
  id    INT NOT NULL AUTO_INCREMENT PRIMARY KEY,
  nome  VARCHAR(20),
  idade INT
) ENGINE=NDBCLUSTER PARTITION BY KEY (id);
SQL

n=$(mysql -u root -Nse "SELECT COUNT(*) FROM clusterdb.aluno" 2>/dev/null || echo 0)
if [ "${n:-0}" -lt 15 ]; then
  { for i in $(seq 1 15); do echo "INSERT INTO clusterdb.aluno(nome,idade) VALUES('aluno$i', $((18 + i)));"; done; } | mysql -u root
fi

mysql -u root -e "SELECT COUNT(*) AS total FROM clusterdb.aluno;"
echo "[EX05/01][N1] feito."
