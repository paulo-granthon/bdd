#!/usr/bin/env bash
#
# EX03 - 04 - Maquina: N1 (192.168.1.2)
# Cria um banco e uma tabela NDBCLUSTER, insere dados e mostra o conteudo.
# Os dados devem aparecer automaticamente no N2 (ver passo 3.5).
# Idempotente: usa IF NOT EXISTS / INSERT IGNORE.
#
set -uo pipefail
[ "$(id -u)" -eq 0 ] || exec sudo -E bash "$0" "$@"

echo "[EX03/04][N1] criando banco/tabela e inserindo dados..."
if ! mysql -u root <<'SQL'
CREATE DATABASE IF NOT EXISTS clusterdb;
USE clusterdb;
CREATE TABLE IF NOT EXISTS funcionarios (
  id   INT NOT NULL PRIMARY KEY,
  nome VARCHAR(50)
) ENGINE=NDBCLUSTER;
INSERT IGNORE INTO funcionarios (id, nome) VALUES
  (1, 'Ana'),
  (2, 'Bruno'),
  (3, 'Carla');
SELECT * FROM funcionarios;
SQL
then
  echo "[EX03/04][N1][ERRO] o mysql falhou (servidor SQL no ar? root sem senha?)." >&2
  exit 1
fi

echo "[EX03/04][N1] feito. Rode o passo 3.5 no N2 para confirmar a replicacao."
