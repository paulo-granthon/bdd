# bdd - Projeto de Banco de Dados Distribuídos (FATEC-SJC)

`bdd` é um programa de linha de comando que executa os passos dos exercícios da
disciplina dentro das máquinas virtuais, no lugar de você digitar os comandos na
mão. Ele também sabe qual máquina é qual, registra o que já foi feito e diz
sempre qual é o próximo passo.

É um binário único, estático (não depende de nada instalado na VM). Depois de
instalado, você usa de qualquer lugar: `bdd 3.1`, `bdd next`, `bdd log`, etc.

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
- **CI/CD**: cada push na `main` compila o binário e publica numa release
  `latest` no GitHub; o instalador (`install.sh`) é servido pelo GitHub Pages em
  `paulo-granthon.github.io/bdd`.
- **Estado** em `/var/lib/bdd/state`: o que já rodou e o que já validou. Por isso
  você pode fechar a sessão e voltar depois que o `bdd` sabe onde parou.
- **Identidade da máquina**: detectada pelo IP interno (`192.168.1.x`) definido
  no EX02. Enquanto esse IP não existe, você diz o papel com `bdd id`. Quando o
  IP passa a existir, ele manda (sobrepõe o que você definiu na mão).

## Comandos

| Comando | O que faz |
|---------|-----------|
| `bdd X.Y` | executa o passo Y do exercício X nesta máquina (ex: `bdd 3.1`) |
| `bdd log` | lista todos os passos, coloridos por estado, com legenda |
| `bdd next` | mostra só o próximo passo, e se é nesta máquina ou em outra |
| `bdd ok` | marca o próximo passo como feito (quando ele é de **outra** máquina) |
| `bdd sync` | adota o progresso já feito antes do bdd, checando o estado real da máquina |
| `bdd check` | roda as validações e diz, por severidade, o que está pendente |
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

No `check`, falhas ganham severidade: amarelo `●` (passo atual, ainda em
andamento), vermelho `✗` (passo que já deveria estar pronto) e vermelho escuro
`✗!` (passo concluído depois de um incompleto, ordem furada).

## Já comecei antes do bdd? (recuperar progresso)

O `bdd` não depende do histórico dele para saber onde você está: ele olha o
**estado real da máquina**. Então, mesmo que você tenha rodado passos na mão
antes de instalar o bdd:

1. instale o bdd (qualquer opção acima);
2. em cada VM, rode `bdd check` para ver o que já está pronto (ele roda as
   validações reais: serviços no ar, cluster conectado, dados presentes, etc.);
3. rode `bdd sync` para adotar esse progresso (marca como feito os passos desta
   máquina que já passam);
4. siga com `bdd next`.

Faça isso em cada uma das três VMs. Passos que rodam em outra máquina você
confirma depois com `bdd ok`.

## Exercícios

| Exercício | Conteúdo | Detalhes |
|-----------|----------|----------|
| EX02 | Rede das VMs (IP interno + hostname + ping) | [EX02/README.md](EX02/README.md) |
| EX03 | Instalação do MySQL Cluster | [EX03/README.md](EX03/README.md) |

Cada exercício tem o seu README com o estado antes/depois, o que ele faz, como
testar e o que capturar como prova.

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

O workflow `.github/workflows/release.yml` compila e publica a release `latest` a
cada push na `main`, e serve o `install.sh` pelo GitHub Pages. Para a Opção B de
instalação funcionar, habilite o Pages uma vez em Settings > Pages > Source:
GitHub Actions. A Opção A (inject pelo host) não depende do Pages.
