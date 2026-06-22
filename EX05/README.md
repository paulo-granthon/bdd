# EX05 - Fragmentação horizontal

Cria uma tabela fragmentada horizontalmente por chave (`PARTITION BY KEY`) no
MySQL Cluster e mostra como os registros se distribuem entre as partições.

## Como testar (resumo)

Pré-requisito: EX03 pronto. No **N1**:

```
bdd 5.1   # cria a tabela ALUNO particionada + insere 15 registros
bdd 5.2   # mostra quantos registros caíram em cada partição
```

## Antes

Cluster MySQL (NDB) no ar (EX03), com o banco `clusterdb`.

## Objetivo

Ver a **fragmentação horizontal** na prática: a tabela `ALUNO` usa
`ENGINE=NDBCLUSTER PARTITION BY KEY (id)`, então cada linha é distribuída entre
os fragmentos (partições) por um hash da chave. O `information_schema.PARTITIONS`
mostra quantas linhas ficaram em cada partição, e `EXPLAIN PARTITIONS` mostra em
qual partição um `id` está.

## Depois

- Tabela `clusterdb.aluno` com 15 registros, espalhados entre as partições.
- Consulta de distribuição das partições disponível para inspeção.

## Passos

| Passo | Máquina | O que faz |
|-------|---------|-----------|
| 5.1 | N1 | cria a tabela `ALUNO` (`PARTITION BY KEY`) e insere 15 registros |
| 5.2 | N1 | mostra a contagem de linhas por partição + `EXPLAIN PARTITIONS` |

## Provas para anexar

```
bdd validate
```
Imprime o `SELECT * FROM clusterdb.aluno` e a distribuição por partição
(`information_schema.PARTITIONS`), que é o que comprova a fragmentação.
