# EX03 - Instalação do MySQL Cluster

Transforma as três VMs cruas do EX02 em um cluster MySQL (NDB) funcionando, com
replicação automática dos dados entre os nós.

## Antes (estado inicial)

Saindo do EX02, temos três VMs Ubuntu 16.04 que só se enxergam pela rede:

| Máquina | IP          | Estado |
|---------|-------------|--------|
| MGM     | 192.168.1.1 | SO limpo, sem software de cluster |
| N1      | 192.168.1.2 | SO limpo, sem software de cluster |
| N2      | 192.168.1.3 | SO limpo, sem software de cluster |

- As três se comunicam por `ping` na rede interna (`192.168.1.0/24`).
- Nenhuma tem MySQL, `ndbd` ou `ndb_mgmd` instalados.
- Não existe banco de dados nem qualquer serviço de cluster rodando.

Resumindo: máquinas em branco, conectadas, mas sem nenhuma noção de "cluster".

## Objetivo (o que o exercício faz)

Instalar o **MySQL Cluster 7.3.26 (NDB)** e dar a cada máquina o seu papel,
montando um banco de dados distribuído com redundância.

Papéis introduzidos:

- **MGM (nó gerenciador, `ndb_mgmd`)**: guarda o `config.ini`, coordena o
  cluster e age como árbitro. É quem conhece a topologia (quais nós existem e
  onde) e por onde os data nodes se descobrem.
- **N1 e N2 (nós de dados, `ndbd` + servidor SQL `mysqld`)**: guardam os dados
  de verdade e respondem consultas SQL. Com `NoOfReplicas=2`, cada linha fica
  gravada **nos dois** nós, ou seja, replicação completa.

Capacidades novas que passam a existir:

- **Armazenamento distribuído**: os dados ficam espalhados/replicados entre N1 e N2.
- **Replicação automática**: o que é escrito por um nó SQL aparece no outro,
  sem nenhuma cópia manual, porque a tabela usa `ENGINE=NDBCLUSTER`.
- **Tolerância a falha**: com a réplica em dois nós, perder um data node não
  perde os dados.
- **Acesso SQL em cada nó de dados**: dá para conectar o `mysql` no N1 ou no N2.
- **Subida automática no boot**: `ndb_mgmd` (MGM) e `ndbd` (N1/N2) viram serviços
  que ligam sozinhos quando a VM reinicia.

Mudança de comportamento principal: as três VMs deixam de ser máquinas isoladas
e passam a se comportar como **um único banco de dados**, visto e escrito por
qualquer um dos nós SQL.

## Depois (estado final)

- **MGM**: rodando `ndb_mgmd`. O comando `ndb_mgm -e show` lista os dois data
  nodes (N1 e N2) e os nós SQL conectados.
- **N1 e N2**: rodando `ndbd` (registrados no gerenciador) e `mysqld` (servidor
  SQL no ar, root sem senha, conforme o exercício).
- **Replicação validada**: existe o banco `clusterdb` com a tabela
  `funcionarios` (`ENGINE=NDBCLUSTER`); os registros inseridos no N1 aparecem ao
  consultar no N2.
- **Persistência de boot**: os serviços do cluster sobem sozinhos no reinício.

Esquema final:

```
                 [ MGM 192.168.1.1 ]
                    ndb_mgmd (gerenciador / árbitro)
                          |
              rede interna 192.168.1.0/24
                /                      \
   [ N1 192.168.1.2 ]            [ N2 192.168.1.3 ]
     ndbd + mysqld                 ndbd + mysqld
        (réplica A)  <-- mesmos dados -->  (réplica B)
```

## Scripts

Ordem, máquina e o que cada um faz estão no
[README do repositório](../README.md#ex03---instalação-do-mysql-cluster).
Resumo: `3.1` (MGM) instala o gerenciador, `3.2` (N1 e N2) instala os nós de
dados, `3.3`/`3.4`/`3.5` verificam e testam a replicação.
