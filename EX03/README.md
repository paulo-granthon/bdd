# EX03 - Instalação do MySQL Cluster

Transforma as três VMs cruas do EX02 em um cluster MySQL (NDB) funcionando, com
replicação automática dos dados entre os nós.

## Como testar (resumo)

Depois de instalar (passos `3.1` na MGM e `3.2` no N1 e no N2):

```
bash run 3.3   # na MGM: deve listar N1 e N2 conectados
bash run 3.4   # no N1:  cria e insere; deve mostrar Ana, Bruno, Carla
bash run 3.5   # no N2:  deve mostrar os MESMOS Ana, Bruno, Carla (replicou)
```

Passou se: o `3.3` mostra os dois data nodes conectados e o `3.5` (no N2) lista
os dados que o `3.4` inseriu no N1. Detalhes e o que olhar no fim deste arquivo.

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

## Como testar (detalhado)

### 1. Os nós estão todos conectados? (na MGM)

```
bash run 3.3
```

Roda `ndb_mgm -e show`. Esperado: os dois `[ndbd]` aparecem **connected** (com
IP e nodegroup, não "not connected"), o `[ndb_mgmd]` no ar e os `[mysqld]`
(API) conectados. Algo assim:

```
[ndbd(NDB)]     2 node(s)
id=2    @192.168.1.2  (mysql-5.6.x ndb-7.3.26, Nodegroup: 0)
id=3    @192.168.1.3  (mysql-5.6.x ndb-7.3.26, Nodegroup: 0)

[ndb_mgmd(MGM)] 1 node(s)
id=1    @192.168.1.1  (mysql-5.6.x ndb-7.3.26)

[mysqld(API)]   2 node(s)
id=4    @192.168.1.2  (...)
id=5    @192.168.1.3  (...)
```

Se um data node sai como "not connected": confira que o `3.2` rodou naquela
máquina e que o gerenciador (`3.1`) subiu antes.

### 2. Escreve no N1 e lê no N2 (a replicação)

No **N1**:

```
bash run 3.4
```

Cria o banco `clusterdb`, a tabela `funcionarios` (`ENGINE=NDBCLUSTER`), insere
3 linhas e mostra:

```
+----+-------+
| id | nome  |
+----+-------+
|  1 | Ana   |
|  2 | Bruno |
|  3 | Carla |
+----+-------+
```

No **N2**:

```
bash run 3.5
```

Sem inserir nada, só lê. Passou se aparecerem **as mesmas** Ana/Bruno/Carla:
foram escritas no N1 e replicadas para o N2 pelo NDB.

### 3. (Opcional) Tolerância a falha

Com `NoOfReplicas=2`, derrubar um data node não perde dado. No **N2** pare o
`ndbd` (`sudo /etc/init.d/ndbd stop` ou mate o processo) e, no N1, rode de novo
a leitura (`bash run 3.4` mostra o SELECT, ou `mysql -u root -e "SELECT * FROM
clusterdb.funcionarios;"`): os dados continuam lá. Suba o nó de volta com
`bash run 3.2` (ou `sudo /etc/init.d/ndbd start`) e ele ressincroniza.

### Checklist

- [ ] `3.3` (MGM) mostra N1 e N2 **connected**.
- [ ] `3.4` (N1) insere e mostra Ana/Bruno/Carla.
- [ ] `3.5` (N2) mostra os mesmos dados sem ter inserido.
- [ ] (opcional) derrubar um data node não perde os dados.

## Provas para anexar

A saída destes comandos é a validação do exercício, é isso que se captura e anexa
(rode os passos de instalação antes; no N1 rode o `3.4` para popular a tabela):

```
# MGM: os dois data nodes connected
hostname -I && ndb_mgm -e show

# N1: os dados inseridos
hostname -I && mysql -u root -e "SELECT * FROM clusterdb.funcionarios;"

# N2: os mesmos dados (replicados)
hostname -I && mysql -u root -e "SELECT * FROM clusterdb.funcionarios;"
```

(`hostname -I` mostra o IP da VM junto da saída, identificando a máquina.)
