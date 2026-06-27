//! Modelo: papéis das máquinas, passos dos exercícios e o manifesto embutido.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Mgm,
    N1,
    N2,
}

#[allow(dead_code)]
impl Role {
    pub fn code(self) -> &'static str {
        match self {
            Role::Mgm => "mgm",
            Role::N1 => "n1",
            Role::N2 => "n2",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Role::Mgm => "MGM",
            Role::N1 => "N1",
            Role::N2 => "N2",
        }
    }
    pub fn ip(self) -> &'static str {
        match self {
            Role::Mgm => "192.168.1.1",
            Role::N1 => "192.168.1.2",
            Role::N2 => "192.168.1.3",
        }
    }
    pub fn from_str(s: &str) -> Option<Role> {
        match s.trim().to_lowercase().as_str() {
            "mgm" | "1" => Some(Role::Mgm),
            "n1" | "2" => Some(Role::N1),
            "n2" | "3" => Some(Role::N2),
            _ => None,
        }
    }
    pub fn all() -> [Role; 3] {
        [Role::Mgm, Role::N1, Role::N2]
    }
}

pub struct Step {
    pub ex: u8,
    pub st: u8,
    pub roles: &'static [Role],
    pub title: &'static str,
    pub script: &'static str,
    pub validate: &'static str,
    pub proof: &'static str,
    /// Passo opcional: o `next` não o cobra e o `check`/`log` o tratam à parte.
    pub optional: bool,
}

impl Step {
    pub fn id(&self) -> String {
        format!("{}.{}", self.ex, self.st)
    }
    pub fn for_role(&self, r: Role) -> bool {
        self.roles.contains(&r)
    }
    /// Texto curto das máquinas do passo: "MGM", "N1 e N2", "todas".
    pub fn machines_label(&self) -> String {
        if self.roles.len() == 3 {
            return "todas".to_string();
        }
        let names: Vec<&str> = self.roles.iter().map(|r| r.name()).collect();
        names.join(" e ")
    }
}

const ALL: &[Role] = &[Role::Mgm, Role::N1, Role::N2];
const MGM: &[Role] = &[Role::Mgm];
const DATA: &[Role] = &[Role::N1, Role::N2];
const N1: &[Role] = &[Role::N1];
const N2: &[Role] = &[Role::N2];

pub fn manifest() -> Vec<Step> {
    vec![
        // ---- EX02: rede ----
        Step {
            ex: 2, st: 1, roles: ALL,
            title: "Configurar rede interna (IP estático + hostname)",
            script: include_str!("../EX02/01_ALL_configura-rede.bash"),
            validate: r#"case "${BDD_ROLE:-}" in mgm) E=192.168.1.1;; n1) E=192.168.1.2;; n2) E=192.168.1.3;; *) exit 1;; esac; ip -4 addr 2>/dev/null | grep -q "inet ${E}/""#,
            proof: "hostname; ip -4 addr show enp0s8 2>/dev/null | grep -w inet",
            optional: false,
        },
        Step {
            ex: 2, st: 2, roles: ALL,
            title: "Verificar conectividade entre as máquinas (ping)",
            script: include_str!("../EX02/02_ALL_verifica-rede.bash"),
            validate: r#"case "${BDD_ROLE:-}" in mgm) P="192.168.1.2 192.168.1.3";; n1) P="192.168.1.1 192.168.1.3";; n2) P="192.168.1.1 192.168.1.2";; *) exit 1;; esac; for ip in $P; do ping -c1 -W2 "$ip" >/dev/null 2>&1 || exit 1; done"#,
            proof: r#"case "${BDD_ROLE:-}" in mgm) P="192.168.1.2 192.168.1.3";; n1) P="192.168.1.1 192.168.1.3";; n2) P="192.168.1.1 192.168.1.2";; *) P="192.168.1.1 192.168.1.2 192.168.1.3";; esac; for ip in $P; do ping -c2 -W2 "$ip"; done"#,
            optional: false,
        },
        // ---- EX03: MySQL Cluster ----
        Step {
            ex: 3, st: 1, roles: MGM,
            title: "Instalar o gerenciador do cluster",
            script: include_str!("../EX03/01_MGM_instala-gerenciador.bash"),
            validate: "pgrep -x ndb_mgmd >/dev/null 2>&1",
            proof: "ndb_mgm -e show",
            optional: false,
        },
        Step {
            ex: 3, st: 2, roles: DATA,
            title: "Instalar o nó de dados (ndbd + mysqld)",
            script: include_str!("../EX03/02_N1-N2_instala-no-de-dados.bash"),
            validate: "pgrep -x ndbd >/dev/null 2>&1 && { pgrep -x mysqld >/dev/null 2>&1 || pgrep -x mysqld_safe >/dev/null 2>&1; }",
            proof: "pgrep -al ndbd; pgrep -al mysqld 2>/dev/null || pgrep -al mysqld_safe",
            optional: false,
        },
        Step {
            ex: 3, st: 3, roles: MGM,
            title: "Verificar o cluster (nós conectados)",
            script: include_str!("../EX03/03_MGM_verifica-cluster.bash"),
            validate: "n=$(ndb_mgm -e show 2>/dev/null | grep -cE '@192\\.168\\.1\\.[23] '); [ \"${n:-0}\" -ge 2 ]",
            proof: "ndb_mgm -e show",
            optional: false,
        },
        Step {
            ex: 3, st: 4, roles: N1,
            title: "Criar banco e inserir dados",
            script: include_str!("../EX03/04_N1_cria-banco-e-insere.bash"),
            validate: "mysql -u root -Nse 'SELECT count(*) FROM clusterdb.funcionarios' 2>/dev/null | grep -q '^3$'",
            proof: "mysql -u root -e 'SELECT * FROM clusterdb.funcionarios;'",
            optional: false,
        },
        Step {
            ex: 3, st: 5, roles: N2,
            title: "Verificar a replicação no N2",
            script: include_str!("../EX03/05_N2_verifica-replicacao.bash"),
            validate: "mysql -u root -Nse 'SELECT count(*) FROM clusterdb.funcionarios' 2>/dev/null | grep -q '^3$'",
            proof: "mysql -u root -e 'SELECT * FROM clusterdb.funcionarios;'",
            optional: false,
        },
        // ---- EX04: uso do cluster em estados degradados (observacional) ----
        ex04(1, N1, "Inserir no N1 com o N2 desligado"),
        ex04(2, N1, "Inserir no N1 com o MGM desligado"),
        ex04(3, N1, "Criar tabela no N1 com o N2 desligado"),
        ex04(4, N1, "Criar tabela no N1 com o MGM desligado"),
        ex04(5, N1, "Criar database no N1 com o N2 desligado"),
        ex04(6, N1, "Criar database no N1 com o MGM desligado"),
        ex04(7, N1, "Inserir no N1 com todo o resto desligado"),
        ex04(8, N2, "Criar tabela no N2 com todo o resto desligado"),
        ex04(9, N2, "Criar database no N2 com todo o resto desligado"),
        ex04(10, N1, "Inserir 1000 registros no N1 com o N2 desligado, depois religar o N2"),
        ex04(11, MGM, "Descrever a necessidade do MGM"),
        // ---- EX05: fragmentação horizontal ----
        Step {
            ex: 5, st: 1, roles: N1,
            title: "Criar tabela ALUNO particionada (PARTITION BY KEY) e inserir 15 registros",
            script: include_str!("../EX05/01_N1_cria-tabela-particionada.bash"),
            validate: "c=$(mysql -u root -Nse 'SELECT COUNT(*) FROM clusterdb.aluno' 2>/dev/null); [ \"${c:-0}\" -ge 15 ]",
            proof: "mysql -u root -e 'SELECT * FROM clusterdb.aluno;'",
            optional: false,
        },
        Step {
            ex: 5, st: 2, roles: N1,
            title: "Ver a distribuição dos registros entre as partições",
            script: include_str!("../EX05/02_N1_distribuicao-particoes.bash"),
            validate: "c=$(mysql -u root -Nse 'SELECT COUNT(*) FROM clusterdb.aluno' 2>/dev/null); [ \"${c:-0}\" -ge 15 ]",
            proof: r#"mysql -u root -e "SELECT partition_name, table_rows FROM information_schema.PARTITIONS WHERE table_schema='clusterdb' AND table_name='aluno';""#,
            optional: false,
        },
        // ---- EX08: Cassandra (cluster separado; node1=.1 seed=MGM, node2=.2=N1, node3=.3=N2) ----
        Step {
            ex: 8, st: 0, roles: ALL,
            title: "(opcional) Liberar memória parando o MySQL Cluster",
            script: include_str!("../EX08/00_ALL_libera-memoria.bash"),
            validate: "",
            proof: "",
            optional: true,
        },
        Step {
            ex: 8, st: 1, roles: ALL,
            title: "Configurar rede interna + hostname (node1/node2/node3)",
            script: include_str!("../EX08/01_ALL_configura-rede.bash"),
            validate: r#"case "${BDD_ROLE:-}" in mgm) E=192.168.1.1;; n1) E=192.168.1.2;; n2) E=192.168.1.3;; *) exit 1;; esac; ip -4 addr 2>/dev/null | grep -q "inet ${E}/""#,
            proof: "hostname; ip -4 addr show enp0s8 2>/dev/null | grep -w inet",
            optional: false,
        },
        Step {
            ex: 8, st: 2, roles: ALL,
            title: "Configurar o cassandra.yaml e subir o serviço (node1 primeiro)",
            script: include_str!("../EX08/02_ALL_configura-cassandra.bash"),
            validate: "nodetool status >/dev/null 2>&1",
            proof: "nodetool status",
            optional: false,
        },
        ex08(3, MGM, "Verificar o cluster (nodetool status, 3 nós UN)", "n=$(nodetool status 2>/dev/null | grep -c '^UN'); [ \"${n:-0}\" -ge 3 ]", "nodetool status"),
        ex08(4, MGM, "Criar keyspace (RF=3), tabela e inserir dados", "cqlsh 192.168.1.1 -e 'SELECT count(*) FROM classe.aluno;' 2>/dev/null | grep -qE '[1-9]'", "cqlsh 192.168.1.1 -e 'SELECT * FROM classe.aluno;'"),
        ex08(5, N1, "Verificar a replicação lendo no node2", "cqlsh 192.168.1.2 -e 'SELECT count(*) FROM classe.aluno;' 2>/dev/null | grep -qE '[1-9]'", "cqlsh 192.168.1.2 -e 'SELECT * FROM classe.aluno;'"),
        ex08(6, N2, "Verificar a replicação lendo no node3", "cqlsh 192.168.1.3 -e 'SELECT count(*) FROM classe.aluno;' 2>/dev/null | grep -qE '[1-9]'", "cqlsh 192.168.1.3 -e 'SELECT * FROM classe.aluno;'"),
        ex08(7, MGM, "Testar a consistência (QUORUM vs ONE com nós off)", "", ""),
    ]
}

fn ex04(st: u8, roles: &'static [Role], title: &'static str) -> Step {
    Step {
        ex: 4, st, roles, title,
        script: include_str!("../EX04/cenarios.bash"),
        validate: "", // observacional: sem validação automática; "feito" = você rodou
        proof: "",
        optional: false,
    }
}

fn ex08(st: u8, roles: &'static [Role], title: &'static str, validate: &'static str, proof: &'static str) -> Step {
    Step {
        ex: 8, st, roles, title,
        script: include_str!("../EX08/cassandra-uso.bash"),
        validate,
        proof,
        optional: false,
    }
}

/// Números dos exercícios em ordem.
pub fn exercises(steps: &[Step]) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::new();
    for s in steps {
        if !v.contains(&s.ex) {
            v.push(s.ex);
        }
    }
    v
}

pub fn find<'a>(steps: &'a [Step], id: &str) -> Option<&'a Step> {
    steps.iter().find(|s| s.id() == id)
}
