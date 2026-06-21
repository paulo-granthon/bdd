# EX02 - Rede das máquinas virtuais

Dá a cada VM a sua identidade de rede no cluster: IP interno fixo e hostname, de
forma que as três passem a se enxergar pela rede interna.

## Como testar (resumo)

Em cada máquina (depois de definir o papel com `bdd id`):

```
bdd 2.1   # configura o IP interno + hostname desta máquina
bdd 2.2   # confirma que esta máquina pinga as outras duas
```

Passou se o `2.2` pinga as outras duas VMs com sucesso.

## Antes (estado inicial)

As três VMs foram criadas no VirtualBox e tiveram o Ubuntu 16.04 instalado (com
OpenSSH Server). Cada uma tem duas placas de rede:

- **enp0s3 (bridge, DHCP)**: pega um IP da sua rede; é por ela que o host alcança
  a VM por SSH.
- **enp0s8 (rede interna)**: ainda **sem IP**.

Ou seja: as VMs ligam e aceitam SSH pela bridge, mas não têm IP interno e não se
enxergam entre si pela rede do cluster. Os hostnames ainda são os do install.

## Objetivo (o que o exercício faz)

Configurar a rede interna do cluster em cada VM:

- **IP estático** em `enp0s8`: MGM `192.168.1.1`, N1 `192.168.1.2`, N2 `192.168.1.3`;
- **hostname** coerente: `mgm`, `n1`, `n2`;
- manter `enp0s3` em DHCP (acesso pelo host segue funcionando).

Capacidade nova: as três máquinas passam a se comunicar por uma rede interna
estável e previsível (`192.168.1.0/24`), que é a base para o cluster dos próximos
exercícios. A partir daqui o `bdd` também consegue **detectar sozinho** qual
máquina é qual, pelo IP interno (antes disso, o papel vem do `bdd id`).

## Depois (estado final)

- Cada VM com IP interno fixo (`.1` / `.2` / `.3`) em `enp0s8`.
- Hostnames `mgm` / `n1` / `n2`.
- As três se pingam pela rede interna.
- `bdd id` passa a mostrar o papel como **detectado** (pelo IP), não só o que você
  tinha definido na mão.

Esquema:

```
   [ mgm 192.168.1.1 ] --- [ n1 192.168.1.2 ] --- [ n2 192.168.1.3 ]
                  rede interna 192.168.1.0/24 (enp0s8)
        (cada uma também com enp0s3 em DHCP para o host/SSH)
```

## Como testar (detalhado)

Em **cada** máquina, antes de tudo, diga o papel dela:

```
bdd id        # escolha 1=MGM, 2=N1, 3=N2
```

Depois:

```
bdd 2.1       # grava o IP interno e o hostname conforme o papel
bdd 2.2       # pinga as outras duas VMs
```

O `2.2` passa quando esta máquina responde ao `ping` das outras duas. Se falhar:
confira que o `2.1` rodou nas três e que as placas internas estão na mesma rede.

## Provas para anexar

A saída destes comandos é a validação, é isso que se captura e anexa (rode em
cada máquina):

```
# mostra o IP interno e o hostname desta máquina
hostname && hostname -I

# mostra que esta máquina enxerga as outras duas
ping -c2 192.168.1.1 ; ping -c2 192.168.1.2 ; ping -c2 192.168.1.3
```
