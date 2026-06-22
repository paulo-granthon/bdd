#!/usr/bin/env bash
#
# EX05 - 02 - Máquina: N1
# Mostra como os registros da tabela ALUNO ficaram distribuídos entre as
# partições (fragmentos), e onde um registro específico está armazenado.
#
set -uo pipefail

echo "[EX05/02][N1] quantos registros há em cada partição:"
mysql -u root -e "SELECT partition_name, table_rows FROM information_schema.PARTITIONS WHERE table_schema='clusterdb' AND table_name='aluno';"

echo
echo "[EX05/02][N1] em qual partição está o registro id=1:"
mysql -u root -e "EXPLAIN PARTITIONS SELECT * FROM clusterdb.aluno WHERE id=1;"
