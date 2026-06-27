# bdd - Projeto de Banco de Dados Distribuídos (FATEC-SJC)

`bdd` é um programa de linha de comando que executa os passos dos exercícios da
disciplina dentro das máquinas virtuais, no lugar de você digitar os comandos na
mão. Ele também sabe qual máquina é qual, registra o que já foi feito e diz
sempre qual é o próximo passo.

É um binário único, estático (não depende de nada instalado na VM). Depois de
instalado, você usa de qualquer lugar: `bdd 3.1`, `bdd next`, `bdd log`, etc.

> **Aviso.** O `bdd` **não é uma boa forma de aprender** os conceitos da
> disciplina e **não testa o seu conhecimento** de jeito nenhum. Ele só
> automatiza a execução para quem **já domina** o assunto e quer agilizar a
> parte mecânica. Se a sua intenção é aprender ou se autoavaliar, **não use**.
> Use por sua conta e risco e bom senso.

## As máquinas

O cluster tem três VMs (montadas no EX02):

| Papel | IP interno   | Função |
|-------|--------------|--------|
| MGM   | 192.168.1.1  | Gerenciador do cluster |
| N1    | 192.168.1.2  | Nó de dados 1 |
| N2    | 192.168.1.3  | Nó de dados 2 |

Cada VM tem duas placas de rede (EX02): uma em **bridge** (DHCP, é por ela que o
host alcança a VM por SSH) e uma **interna** com o IP estático acima.

## Como instalar

Duas opções. A primeira não exige digitar nada dentro da VM.

### Opção A (recomendada): injetar do host via SSH

Roda no **seu computador (host)**, não na VM, pelo comando `bdd inject`. O host
acha as VMs, instala o binário em cada uma por SSH e já define o papel. Funciona
assim que as VMs estão ligadas com OpenSSH (vem do EX02).

Pré-requisitos no host: `ssh`, `scp`, `sshpass` e `curl` ou `wget`.

```
git clone https://github.com/paulo-granthon/bdd.git
cd bdd
./inject.sh
```

O `inject.sh` só baixa o binário e abre o injetor (`bdd inject`).

O `bdd inject` é uma interface interativa (TUI) que:

1. pede as credenciais SSH numa grade (até 3 pares usuário/senha; cada VM é
   testada com todos os pares, então serve para logins iguais ou diferentes);
2. procura as VMs na rede (scan de porta SSH, ignorando o próprio host);
3. lista as VMs e, quando o EX02 já foi feito, **sugere** quem é MGM / N1 / N2
   pelo hostname e pelo IP interno (`192.168.1.x`);
4. você confirma/ajusta os papéis com as setas e o Enter (e pode adicionar um IP
   na mão com `a` se o scan perder alguma);
5. ao confirmar (F2), instala em todas de uma vez e define o papel de cada uma.

Pronto: `bdd` está em todas, cada uma já sabe quem é.

> **Se o scan não achar todas as VMs (ou aparecer "sem acesso"):** o host só
> alcança a VM pela placa em **bridge** (`enp0s3`, DHCP); a rede interna
> `192.168.1.x` (`enp0s8`) é só entre as VMs. VMs **clonadas** costumam ficar com
> o **mesmo MAC** na placa bridge, e o DHCP dá o **mesmo IP** às duas (conflito),
> então só uma responde. Conserte regenerando o MAC: no VirtualBox, Settings >
> Network > Adapter 1 > Advanced > MAC Address > botão de regenerar (↻), em cada
> VM. Depois reinicie a VM, ou renove o DHCP nela:
> ```
> sudo dhclient -r enp0s3 && sudo dhclient enp0s3
> ```
> Confira com `ip -4 addr show enp0s3` que cada VM tem um `192.168.0.x` distinto.
> Se faltar alguma, dá para adicionar o IP na mão na própria tela do `bdd inject`.

### Opção B: instalar na própria VM (on-box)

Dentro da VM. Tente primeiro a linha curta; se der erro de SSL, use a de baixo
(o `wget` velho do Ubuntu 16.04 às vezes falha o TLS; o `curl` resolve).

```
wget -qO- paulo-granthon.github.io/bdd | sh
```

```
sudo apt-get install -y curl && curl -L paulo-granthon.github.io/bdd | sh
```

Depois, diga qual máquina é esta:

```
bdd id
```

## Como funciona (arquitetura)

- **Binário Rust estático (musl)**, sem dependências de runtime, então roda no
  Ubuntu 16.04 das VMs sem instalar mais nada.
- Os **scripts dos passos** (bash) ficam **embutidos** no binário. `bdd 3.1`
  extrai e executa o script certo. Depois de instalado, não precisa baixar mais
  nada (bom para a rede instável das VMs).
- **CI/CD**: cada push na `main` (que mexa no código ou no instalador) compila o
  binário e publica no GitHub Pages: o binário em `paulo-granthon.github.io/bdd/bin`
  e o instalador em `paulo-granthon.github.io/bdd`. O deploy do Pages é
  **atômico**: no redeploy o site segue servindo a versão antiga e só troca
  quando a nova fica pronta, então a URL nunca cai.
- **Estado** em `/var/lib/bdd/state`: o que já rodou e o que já validou. Por isso
  você pode fechar a sessão e voltar depois que o `bdd` sabe onde parou.
- **Identidade da máquina**: detectada pelo IP interno (`192.168.1.x`) definido
  no EX02. Enquanto esse IP não existe, você diz o papel com `bdd id`. Quando o
  IP passa a existir, ele manda (sobrepõe o que você definiu na mão).

## Comandos

| Comando | O que faz |
|---------|-----------|
| `bdd X.Y` | executa o passo Y do exercício X nesta máquina (ex: `bdd 3.1`) |
| `bdd run` | executa o próximo passo se for desta máquina (sem digitar o número) |
| `bdd validate [X] [--clean]` | imprime as provas (saída dos comandos); sem `X` = exercício atual, com `X` (ex: `bdd validate 3`) = aquele exercício; `--clean` limpa a tela inteira (e o scrollback) e imprime só as provas, sem o cabeçalho nem a seção "Próximo" (bom para print limpo) |
| `bdd log` | lista todos os passos, coloridos por estado, com legenda |
| `bdd next` | mostra só o próximo passo, e se é nesta máquina ou em outra |
| `bdd ok` | marca o próximo passo como feito (quando ele é de **outra** máquina) |
| `bdd check` | valida a máquina, adota o que já está pronto e ajusta o `next` |
| `bdd id` | mostra/define qual máquina é esta (`bdd id` interativo, `bdd id mgm` direto) |

Depois de qualquer comando, o `bdd` imprime o **próximo passo** a executar.

### O ponteiro `next` e o `ok`

`next` é sempre a próxima coisa a fazer. Ele anda sozinho:

- se o próximo passo é **desta** máquina, você roda `bdd X.Y` aqui e o `next`
  avança;
- se o próximo é de **outra** máquina, você roda lá e, de volta aqui, faz
  `bdd ok` para o `next` avançar (o `ok` recusa se o próximo for desta máquina,
  justamente para você não pular um passo seu).

### Cores do `log` / `check`

- verde + `✓`: feito;
- ciano + `←`: próximo, nesta máquina;
- amarelo + `←`: próximo, em outra máquina;
- vermelho apagado: não roda nesta máquina;
- azul: assumido feito (já passamos dele);
- apagado: ainda não feito.

No `check`, o que falta ganha severidade: amarelo `●` (passo a fazer agora),
apagado (ainda não iniciado) e vermelho escuro `✗!` (passo incompleto antes de
um que já está feito, ordem furada). Os que passam ficam verdes `✓`, com
`(cache)` quando o resultado já tinha sido validado antes.

## Já comecei antes do bdd? (recuperar progresso)

O `bdd` não depende do histórico dele para saber onde você está: ele olha o
**estado real da máquina**. Então, mesmo que você tenha rodado passos na mão
antes de instalar o bdd:

1. instale o bdd (qualquer opção acima);
2. em cada VM, rode `bdd check`: ele roda as validações reais (serviços no ar,
   cluster conectado, dados presentes, etc.), **adota como feito** o que já passa
   e ajusta o `next`;
3. siga com `bdd next`.

Faça isso em cada uma das três VMs. Passos que rodam em outra máquina você
confirma depois com `bdd ok`.

## Mudanças fora do escopo dos exercícios

Para uma experiência mais limpa, o `bdd` faz **uma** alteração que os exercícios
não pedem: na instalação (que já roda como root), ele garante uma linha
`127.0.1.1 <hostname>` em `/etc/hosts`. Isso só serve para silenciar o aviso
`sudo: unable to resolve host <hostname>` que apareceria no primeiro `sudo` de
cada passo. Não afeta a rede do cluster nem o resultado dos exercícios.

## Exercícios

| Exercício | Conteúdo | Detalhes |
|-----------|----------|----------|
| EX02 | Rede das VMs (IP interno + hostname + ping) | [EX02/README.md](EX02/README.md) |
| EX03 | Instalação do MySQL Cluster | [EX03/README.md](EX03/README.md) |
| EX04 | Uso do cluster em estados degradados (nó/MGM off) | [EX04/README.md](EX04/README.md) |
| EX05 | Fragmentação horizontal (PARTITION BY KEY) | [EX05/README.md](EX05/README.md) |
| EX08 | Cassandra (cluster multi-node, OVA separada) | [EX08/README.md](EX08/README.md) |

Cada exercício tem o seu README com o estado antes/depois, o que ele faz, como
testar e o que capturar como prova.

EX01 (teoria de replicação/fragmentação) e EX06/EX07 (programas de
criptografia/socket no host) não são passos de cluster, então não fazem parte do
`bdd`; foram entregues como PDFs em `Resolvidos/`.

## Desenvolvimento

Binário em Rust (`src/`). O núcleo (comandos nas VMs) usa só a biblioteca padrão;
o `bdd inject` (TUI no host) usa o crate `crossterm`. Nada disso vira dependência
nas VMs: elas só recebem o binário estático. Build local:

```
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

Os passos ficam em `EX0N/*.bash` e são embutidos via `include_str!` em
`src/model.rs` (que também guarda papel da máquina e o comando de validação de
cada passo).

O workflow `.github/workflows/release.yml` compila o binário e publica o site do
Pages (binário + instalador) a cada push na `main`. Habilite o Pages uma vez em
Settings > Pages > Source: GitHub Actions. Tanto a Opção A quanto a B baixam o
binário do Pages.
