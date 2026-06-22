# EX08 - Cassandra (cluster multi-node)

Configura três VMs com Cassandra em modo cluster, cria um keyspace replicado e
testa a replicação e a consistência.

> Este exercício usa **outro conjunto de VMs** (a OVA do Cassandra, com o
> Cassandra já instalado), não as VMs do MySQL Cluster. Os IPs internos seguem o
> mesmo esquema (`192.168.1.1/.2/.3`), então o `bdd` identifica os papéis pelo
> IP: **node1 = .1 (seed) = papel MGM**, **node2 = .2 = N1**, **node3 = .3 = N2**.
> Injete/instale o `bdd` nessas VMs do mesmo jeito (`./inject.sh` no host).

## Como testar (resumo)

```
bdd 8.1   # (todas) rede + hostname node1/2/3
bdd 8.2   # (todas, node1 primeiro) cassandra.yaml + sobe o serviço
bdd 8.3   # (node1) nodetool status: 3 nós UN
bdd 8.4   # (node1) cria keyspace RF=3 + tabela + dados
bdd 8.5   # (node2) lê os dados (replicação)
bdd 8.6   # (node3) lê os dados (replicação)
bdd 8.7   # (node1) teste de consistência (QUORUM vs ONE)
```

## Antes

Três VMs com Cassandra instalado (OVA), ligadas, mas sem o cluster configurado:
cada uma isolada, sem IP interno definido nem `cassandra.yaml` ajustado.

## Objetivo

Montar um cluster Cassandra de 3 nós:

- IP interno fixo e hostname `node1/node2/node3`;
- `cassandra.yaml` com `cluster_name`, `seeds` (node1), `listen_address` e
  `rpc_address` próprios e `SimpleSnitch`;
- keyspace `classe` com `SimpleStrategy` e `replication_factor: 3` (dado em todos
  os nós), tabela `aluno` e alguns registros;
- entender a consistência: com `QUORUM` precisa de >51% das réplicas; com `ONE`
  basta um nó no ar.

## Depois

- `nodetool status` mostra os 3 nós como `UN` (Up/Normal).
- Os dados inseridos no node1 aparecem ao ler no node2 e no node3 (replicação).

## Passos

| Passo | Máquina | O que faz |
|-------|---------|-----------|
| 8.1 | todas | rede interna + hostname (node1/node2/node3) |
| 8.2 | todas | `cassandra.yaml` + sobe o serviço (rode no node1 primeiro) |
| 8.3 | node1 | `nodetool status` (3 nós UN) |
| 8.4 | node1 | cria keyspace RF=3, tabela `aluno` e insere |
| 8.5 | node2 | lê os dados localmente (replicação) |
| 8.6 | node3 | lê os dados localmente (replicação) |
| 8.7 | node1 | teste de consistência (QUORUM vs ONE) |

## Provas para anexar

```
bdd validate
```
Imprime, conforme a máquina, o `nodetool status` e os `SELECT * FROM
classe.aluno`, que comprovam o cluster no ar e a replicação.
