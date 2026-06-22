#!/usr/bin/env bash
#
# EX08 - uso do cluster Cassandra. O passo é escolhido por BDD_STEP (8.3..8.7).
#   8.3 status do cluster | 8.4 cria keyspace+tabela+dados | 8.5/8.6 lê no nó
#   local (replicação) | 8.7 teste de consistência.
#
set -uo pipefail
SEED=192.168.1.1
case "${BDD_ROLE:-}" in
  mgm) OWN=192.168.1.1 ;;
  n1)  OWN=192.168.1.2 ;;
  n2)  OWN=192.168.1.3 ;;
  *)   OWN=127.0.0.1 ;;
esac
cql() { cqlsh "$1" -e "$2" 2>&1; }

case "${BDD_STEP:-}" in
  8.3)
    echo "[EX08/03] status do cluster (procure 3 linhas começando com UN):"
    nodetool status
    ;;
  8.4)
    echo "[EX08/04] criando keyspace, tabela e inserindo dados..."
    cql "$SEED" "CREATE KEYSPACE IF NOT EXISTS classe WITH REPLICATION = {'class':'SimpleStrategy','replication_factor':3};"
    cql "$SEED" "CREATE TABLE IF NOT EXISTS classe.aluno (nome text, sobrenome text, PRIMARY KEY (sobrenome));"
    cql "$SEED" "INSERT INTO classe.aluno(nome,sobrenome) VALUES ('Maria','Santos');"
    cql "$SEED" "INSERT INTO classe.aluno(nome,sobrenome) VALUES ('Mario','Silva');"
    cql "$SEED" "INSERT INTO classe.aluno(nome,sobrenome) VALUES ('Jose','Oliveira');"
    cql "$SEED" "INSERT INTO classe.aluno(nome,sobrenome) VALUES ('Joao','Ferreiro');"
    cql "$SEED" "SELECT * FROM classe.aluno;"
    ;;
  8.5 | 8.6)
    echo "[EX08/${BDD_STEP}] lendo no nó local ($OWN) - prova da replicação (RF=3):"
    cql "$OWN" "SELECT * FROM classe.aluno;"
    ;;
  8.7)
    echo "[EX08/07] teste de consistência:"
    echo "Com 2 nós offline e CONSISTENCY QUORUM, a leitura tende a falhar (NoHostAvailable),"
    echo "pois quórum de RF=3 exige 2 réplicas. Com CONSISTENCY ONE, basta 1 nó no ar."
    cqlsh "$SEED" -e "CONSISTENCY ONE; SELECT * FROM classe.aluno;" 2>&1 || true
    ;;
  *)
    echo "[EX08] passo desconhecido: ${BDD_STEP:-}"
    exit 1
    ;;
esac
