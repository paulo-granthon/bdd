# EX04 - Utilização do MySQL Cluster (estados degradados)

Observa como o cluster do EX03 se comporta quando um nó (ou mais) está
desligado. O entregável são as **suas observações/comentários**; o `bdd`
executa cada cenário e imprime uma análise curta para te orientar.

## Como testar (resumo)

Pré-requisito: EX03 pronto (cluster no ar). Cada cenário tem uma **pré-condição
manual**: desligue a VM/serviço indicado **antes** de rodar o passo.

```
bdd 4.1   # ... (cada passo diz a pré-condição e roda a ação)
bdd 4.11
```

## Antes

Cluster MySQL (NDB) completo e funcionando (EX03): MGM + N1 + N2, com a tabela
`clusterdb.funcionarios` replicada.

## Objetivo

Entender, na prática, o papel de cada peça do cluster observando o que acontece
ao derrubar nós:

- **NoOfReplicas=2**: com um data node fora, o outro mantém os dados.
- **MGM**: fora do caminho dos dados (consultas seguem), mas essencial na
  inicialização/descoberta e como **árbitro** (evita split-brain).
- **CREATE DATABASE** é local ao nó SQL; **CREATE TABLE NDBCLUSTER** depende do
  cluster operante.
- Um único data node de dois, sem árbitro, se protege e para.
- Religar um nó dispara **ressincronização automática**.

## Depois

Você tem, para cada cenário, a saída real + a sua conclusão sobre por que o
cluster aceitou ou recusou a operação.

## Cenários (passos)

| Passo | Máquina | Cenário |
|-------|---------|---------|
| 4.1 | N1 | inserir com N2 desligado |
| 4.2 | N1 | inserir com MGM desligado |
| 4.3 | N1 | criar tabela com N2 desligado |
| 4.4 | N1 | criar tabela com MGM desligado |
| 4.5 | N1 | criar database com N2 desligado |
| 4.6 | N1 | criar database com MGM desligado |
| 4.7 | N1 | inserir com todo o resto desligado |
| 4.8 | N2 | criar tabela com todo o resto desligado |
| 4.9 | N2 | criar database com todo o resto desligado |
| 4.10 | N1 | inserir 1000 registros com N2 desligado, depois religar o N2 |
| 4.11 | MGM | descrever a necessidade do MGM |

## Provas para anexar

A prova de cada cenário é a **saída ao rodar `bdd 4.x`** (com a pré-condição
montada). Capture a tela de cada passo. `bdd validate` não tem o que imprimir
aqui porque o resultado depende do estado que você montou manualmente.
