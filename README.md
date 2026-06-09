# Scripts de automacao - Projeto de Banco de Dados Distribuidos (FATEC-SJC)

Este repositorio automatiza os passos dos exercicios da disciplina, para nao
precisar digitar todos os comandos na mao dentro das maquinas virtuais.

Cada exercicio tem o seu diretorio. Dentro dele ficam os scripts daquele
exercicio, numerados na ordem em que devem ser executados.

## As maquinas

O ambiente (montado no EX02) tem tres VMs:

| Maquina | Papel                  | IP (rede interna) |
|---------|------------------------|-------------------|
| MGM     | Gerenciador do cluster | 192.168.1.1       |
| N1      | No de dados 1          | 192.168.1.2       |
| N2      | No de dados 2          | 192.168.1.3       |

## Convencao de nomes

```
<ordem>_<maquina>_<o-que-faz>.bash
```

- **ordem**: 01, 02, 03 ... ordem de execucao dentro do exercicio.
- **maquina**: onde rodar o script. Pode ser `MGM`, `N1`, `N2` ou `N1-N2`
  (este ultimo significa "rode o mesmo script nos dois nos de dados").
- **o-que-faz**: descricao curta.

## Como usar

### Jeito facil: o script `run`

Baixe so o `run` uma vez em cada maquina e chame os passos por numero
(`<exercicio>.<passo>`). O `run` acha o script certo: usa o arquivo local se o
repo estiver clonado, ou baixa do GitHub se voce so pegou o `run`.

```
# baixe o run (uma vez por maquina)
wget https://raw.githubusercontent.com/paulo-granthon/bdd-cluster-scripts/main/run -O run

# rode cada passo NA MAQUINA INDICADA:
bash run 3.1     # MGM:     instala o gerenciador
bash run 3.2     # N1 e N2: instala o no de dados (rode nas duas)
bash run 3.3     # MGM:     verifica o cluster
bash run 3.4     # N1:      cria banco e insere
bash run 3.5     # N2:      verifica a replicacao
```

Ou tudo em uma linha so (baixa e executa):

```
wget -qO- https://raw.githubusercontent.com/paulo-granthon/bdd-cluster-scripts/main/run | bash -s 3.1
```

### Jeito manual: baixar o script direto

```
wget https://raw.githubusercontent.com/paulo-granthon/bdd-cluster-scripts/main/EX03/01_MGM_instala-gerenciador.bash -O script.bash
bash script.bash
```

### Ou clonar o repositorio

```
git clone https://github.com/paulo-granthon/bdd-cluster-scripts.git
cd bdd-cluster-scripts
bash run 3.1                              # na MGM
# ou direto:
bash EX03/01_MGM_instala-gerenciador.bash # na MGM
```

Os scripts pedem privilegio de root automaticamente (re-executam com `sudo`),
entao basta chamar com `bash`.

## Seguranca / progresso parcial

Os scripts sao **idempotentes**: podem ser rodados de novo sem quebrar. Eles
checam o que ja existe (diretorios, usuarios, pacotes, binarios, servicos) e so
fazem o que falta. Como nao da pra adivinhar "onde voce parou", cada script se
vira sozinho com o que encontrar na maquina.

## Pre-requisitos

- Ambiente de rede do EX02 pronto (as tres VMs se enxergam por `ping`).
- Acesso a internet nas VMs (download do MySQL Cluster e pacotes `apt`).
- Ubuntu 16.04 (alvo dos exercicios), MySQL Cluster 7.3.26.

## EX03 - Instalacao do MySQL Cluster

Ordem de execucao:

| Ordem | Maquina | Script                                | O que faz |
|-------|---------|---------------------------------------|-----------|
| 01    | MGM     | `01_MGM_instala-gerenciador.bash`     | Instala ndb_mgm/ndb_mgmd, cria o config.ini, sobe o gerenciador e habilita no boot |
| 02    | N1 e N2 | `02_N1-N2_instala-no-de-dados.bash`   | Instala o MySQL Cluster como no de dados, cria o my.cnf, sobe o ndbd e o mysqld |
| 03    | MGM     | `03_MGM_verifica-cluster.bash`        | Mostra os nos conectados (`ndb_mgm -e show`) |
| 04    | N1      | `04_N1_cria-banco-e-insere.bash`      | Cria banco + tabela NDBCLUSTER e insere dados |
| 05    | N2      | `05_N2_verifica-replicacao.bash`      | Confirma que os dados criados no N1 aparecem no N2 |

Rode o **02** nas duas maquinas de dados (N1 e N2). O gerenciador (passo 01)
precisa estar no ar antes dos nos de dados subirem (passo 02).
