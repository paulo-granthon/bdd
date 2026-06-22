#!/usr/bin/env bash
#
# EX04 - Utilização do MySQL Cluster em estados degradados.
# Cada passo executa a ação e imprime uma análise. A PRÉ-CONDIÇÃO (desligar um
# nó) é manual: desligue a VM ou o serviço indicado ANTES de rodar o passo.
# O passo executado é escolhido por BDD_STEP (ex: 4.1).
#
set -uo pipefail
DB=clusterdb
sql() { mysql -u root -e "$1" 2>&1; }
hr() { echo "------------------------------------------------------------"; }

PRE=""; ACAO=""; OUT=""; ANAL=""
case "${BDD_STEP:-}" in
  4.1) PRE="N2 (192.168.1.3) DESLIGADO"; ACAO="inserir no N1"
       OUT=$(sql "USE $DB; INSERT IGNORE INTO funcionarios(id,nome) VALUES (41,'ex04-1'); SELECT COUNT(*) AS total FROM funcionarios;")
       ANAL="Com NoOfReplicas=2 e o outro data node fora, o cluster segue atendendo pela réplica restante; observe se a inserção foi aceita." ;;
  4.2) PRE="MGM (192.168.1.1) DESLIGADO"; ACAO="inserir no N1"
       OUT=$(sql "USE $DB; INSERT IGNORE INTO funcionarios(id,nome) VALUES (42,'ex04-2'); SELECT COUNT(*) AS total FROM funcionarios;")
       ANAL="O MGM não fica no caminho dos dados; com os nós já conectados, observe se a operação continua sendo aceita." ;;
  4.3) PRE="N2 DESLIGADO"; ACAO="criar tabela no N1"
       OUT=$(sql "USE $DB; CREATE TABLE IF NOT EXISTS ex04_tab(id INT PRIMARY KEY) ENGINE=NDBCLUSTER; SHOW TABLES;")
       ANAL="DDL de tabela NDB depende do cluster operante; observe se a criação foi aceita com apenas uma réplica no ar." ;;
  4.4) PRE="MGM DESLIGADO"; ACAO="criar tabela no N1"
       OUT=$(sql "USE $DB; CREATE TABLE IF NOT EXISTS ex04_tab(id INT PRIMARY KEY) ENGINE=NDBCLUSTER; SHOW TABLES;")
       ANAL="Com os data nodes no ar, observe se o DDL acontece mesmo sem o MGM presente." ;;
  4.5) PRE="N2 DESLIGADO"; ACAO="criar database no N1"
       OUT=$(sql "CREATE DATABASE IF NOT EXISTS ex04db; SHOW DATABASES;")
       ANAL="CREATE DATABASE é local ao nó SQL; observe que não depende do estado dos data nodes." ;;
  4.6) PRE="MGM DESLIGADO"; ACAO="criar database no N1"
       OUT=$(sql "CREATE DATABASE IF NOT EXISTS ex04db; SHOW DATABASES;")
       ANAL="Idem: o schema é local ao mysqld; observe a independência em relação ao MGM." ;;
  4.7) PRE="N2 E MGM DESLIGADOS (só o N1 no ar)"; ACAO="inserir no N1"
       OUT=$(sql "USE $DB; INSERT IGNORE INTO funcionarios(id,nome) VALUES (47,'ex04-7'); SELECT COUNT(*) AS total FROM funcionarios;")
       ANAL="Um único data node de dois, sem árbitro (MGM), tende a se proteger contra split-brain; observe se a operação falha." ;;
  4.8) PRE="TODO O RESTO DESLIGADO (só o N2 no ar)"; ACAO="criar tabela no N2"
       OUT=$(sql "USE $DB; CREATE TABLE IF NOT EXISTS ex04_tab2(id INT PRIMARY KEY) ENGINE=NDBCLUSTER; SHOW TABLES;")
       ANAL="Mesma situação do nó isolado sem árbitro; observe o comportamento do DDL." ;;
  4.9) PRE="TODO O RESTO DESLIGADO (só o N2 no ar)"; ACAO="criar database no N2"
       OUT=$(sql "CREATE DATABASE IF NOT EXISTS ex04db2; SHOW DATABASES;")
       ANAL="O CREATE DATABASE local pode até ocorrer, mas note a diferença para objetos NDB quando o cluster não está operante." ;;
  4.10) PRE="N2 DESLIGADO agora (LIGUE o N2 DEPOIS e rode bdd validate / cheque o N2)"; ACAO="inserir 1000 registros no N1"
       mysql -u root -e "USE $DB; CREATE TABLE IF NOT EXISTS ex04_mil(id INT PRIMARY KEY) ENGINE=NDBCLUSTER;" 2>&1
       { for i in $(seq 1 1000); do echo "INSERT IGNORE INTO $DB.ex04_mil(id) VALUES($i);"; done; } | mysql -u root 2>&1
       OUT=$(sql "SELECT COUNT(*) AS total FROM ex04_mil;")
       ANAL="Ao reconectar o N2, observe se ele ressincroniza sozinho e passa a ter os 1000 registros (recuperação automática do NDB)." ;;
  4.11) PRE="(conceitual)"; ACAO="descrever a necessidade do MGM"
       OUT="O MGM (ndb_mgmd) distribui o config.ini, coordena a descoberta/inicialização dos nós e atua como ÁRBITRO."
       ANAL="Em regime, com os nós conectados, as consultas não dependem do MGM; mas sem ele um nó que reinicia não reentra e a proteção de quórum pode derrubar o cluster. Descreva com base no que observou." ;;
  *) echo "[EX04] passo desconhecido: ${BDD_STEP:-}"; exit 1 ;;
esac

echo "[EX04 ${BDD_STEP}] Ação: ${ACAO}"
echo "Pré-condição (faça manualmente antes): ${PRE}"
hr
echo "$OUT"
hr
echo "Análise: ${ANAL}"
