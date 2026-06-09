# Scripts de automação - Projeto de Banco de Dados Distribuídos (FATEC-SJC)

Este repositório automatiza os passos dos exercícios da disciplina, para não
precisar digitar todos os comandos na mão dentro das máquinas virtuais.

Cada exercício tem o seu diretório. Dentro dele ficam os scripts daquele
exercício, numerados na ordem em que devem ser executados.

## As máquinas

O ambiente (montado no EX02) tem três VMs:

| Máquina | Papel                  | IP (rede interna) |
|---------|------------------------|-------------------|
| MGM     | Gerenciador do cluster | 192.168.1.1       |
| N1      | Nó de dados 1          | 192.168.1.2       |
| N2      | Nó de dados 2          | 192.168.1.3       |

## Convenção de nomes

```
<ordem>_<máquina>_<o-que-faz>.bash
```

- **ordem**: 01, 02, 03 ... ordem de execução dentro do exercício.
- **máquina**: onde rodar o script. Pode ser `MGM`, `N1`, `N2` ou `N1-N2`
  (este último significa "rode o mesmo script nos dois nós de dados").
- **o-que-faz**: descrição curta.

## Como usar

### Jeito fácil: o script `run`

Baixe só o `run` uma vez em cada máquina e chame os passos por número
(`<exercício>.<passo>`). O `run` acha o script certo: usa o arquivo local se o
repositório estiver clonado, ou baixa do GitHub se você só pegou o `run`.

```
wget https://raw.githubusercontent.com/paulo-granthon/bdd/main/run -O run
```

> Atenção: use **`-O run`** (O maiúsculo) para salvar o arquivo. Com `-o run`
> (o minúsculo) o wget grava só o *log* dele no arquivo, e o `run` sai **vazio**
> (rodar um arquivo vazio não faz nada). Para conferir que baixou certo:
> `head -1 run` deve mostrar `#!/usr/bin/env bash`.

Depois, rode cada passo **na máquina indicada**:

```
bash run 3.1     # MGM:     instala o gerenciador
bash run 3.2     # N1 e N2: instala o nó de dados (rode nas duas)
bash run 3.3     # MGM:     verifica o cluster
bash run 3.4     # N1:      cria banco e insere
bash run 3.5     # N2:      verifica a replicação
```

Ou tudo em uma linha só (baixa e executa, sem salvar arquivo):

```
wget -qO- https://raw.githubusercontent.com/paulo-granthon/bdd/main/run | bash -s 3.1
```

Aqui o `-O-` (com hífen, mandando para a saída padrão) é o certo, e o `|` joga
no `bash`. Não troque por `-o`.

### Jeito manual: baixar o script direto

```
wget https://raw.githubusercontent.com/paulo-granthon/bdd/main/EX03/01_MGM_instala-gerenciador.bash -O script.bash
bash script.bash
```

### Ou clonar o repositório

```
git clone https://github.com/paulo-granthon/bdd.git
cd bdd
bash run 3.1                              # na MGM
# ou direto:
bash EX03/01_MGM_instala-gerenciador.bash # na MGM
```

Os scripts pedem privilégio de root automaticamente (re-executam com `sudo`),
então basta chamar com `bash`.

## Segurança / progresso parcial

Os scripts são **idempotentes**: podem ser rodados de novo sem quebrar. Eles
checam o que já existe (diretórios, usuários, pacotes, binários, serviços) e só
fazem o que falta. Como não dá para adivinhar "onde você parou", cada script se
vira sozinho com o que encontrar na máquina. Cada passo anuncia o que está
fazendo, então dá para ver na tela onde parou se algo der errado.

## Pré-requisitos

- Ambiente de rede do EX02 pronto (as três VMs se enxergam por `ping`).
- Acesso à internet nas VMs (download do MySQL Cluster e pacotes `apt`).
- Ubuntu 16.04 (alvo dos exercícios), MySQL Cluster 7.3.26.

## EX03 - Instalação do MySQL Cluster

Ordem de execução:

| Ordem | Máquina | Script                                | O que faz |
|-------|---------|---------------------------------------|-----------|
| 01    | MGM     | `01_MGM_instala-gerenciador.bash`     | Instala ndb_mgm/ndb_mgmd, cria o config.ini, sobe o gerenciador e habilita no boot |
| 02    | N1 e N2 | `02_N1-N2_instala-no-de-dados.bash`   | Instala o MySQL Cluster como nó de dados, cria o my.cnf, sobe o ndbd e o mysqld |
| 03    | MGM     | `03_MGM_verifica-cluster.bash`        | Mostra os nós conectados (`ndb_mgm -e show`) |
| 04    | N1      | `04_N1_cria-banco-e-insere.bash`      | Cria banco + tabela NDBCLUSTER e insere dados |
| 05    | N2      | `05_N2_verifica-replicacao.bash`      | Confirma que os dados criados no N1 aparecem no N2 |

Rode o **02** nas duas máquinas de dados (N1 e N2). O gerenciador (passo 01)
precisa estar no ar antes dos nós de dados subirem (passo 02).

## Solução de problemas

- **O `run` (ou o arquivo baixado) não faz nada / sai vazio.** Você provavelmente
  salvou com `-o` no lugar de `-O`. Confira com `head -1 run`; se não aparecer
  `#!/usr/bin/env bash`, baixe de novo usando `-O run`.

- **`Unable to establish SSL connection` no wget (mas o `ping` funciona).** O
  `wget` do Ubuntu 16.04 usa GnuTLS antigo, que às vezes não fecha o TLS com o
  GitHub. O `curl` (OpenSSL) costuma funcionar. Instale o curl (o `apt` usa http,
  então funciona mesmo com o TLS quebrado) e baixe com ele:

  ```
  sudo apt-get update && sudo apt-get install -y curl ca-certificates
  curl -fsSL https://raw.githubusercontent.com/paulo-granthon/bdd/main/run -o run
  ```

  Com o curl instalado, os próprios scripts já caem para o curl sozinhos nos
  downloads. Alternativas: `wget --secure-protocol=TLSv1_2 <url>`, ou conferir o
  relógio da VM (`date`; TLS depende da hora certa). Como o N2 já funcionou, dá
  também para servir os arquivos do N2 para o N1 pela rede interna (sem TLS):
  no N2 `python3 -m http.server 8000` (ou `python -m SimpleHTTPServer 8000`) e no
  N1 `wget http://192.168.1.3:8000/run -O run`.

- **`E: Unable to lock /var/lib/apt/...` ou `Could not get lock`.** Logo após o
  boot, o próprio Ubuntu roda atualizações (`apt-daily`/`unattended-upgrades`) e
  segura o `apt`. O passo 02 já espera o lock liberar e tenta de novo sozinho por
  alguns minutos. Se rodou algum `apt` na mão, é só esperar 1-2 min e rodar de novo.

- **`ndbd` não conecta / `ndb_mgm -e show` mostra nó desconectado.** Garanta que o
  passo 3.1 (gerenciador, na MGM) rodou e está no ar antes do 3.2 (nós de dados),
  e que as três VMs se enxergam por `ping` (rede do EX02).
