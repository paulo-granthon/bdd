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
        },
        Step {
            ex: 2, st: 2, roles: ALL,
            title: "Verificar conectividade entre as máquinas (ping)",
            script: include_str!("../EX02/02_ALL_verifica-rede.bash"),
            validate: r#"case "${BDD_ROLE:-}" in mgm) P="192.168.1.2 192.168.1.3";; n1) P="192.168.1.1 192.168.1.3";; n2) P="192.168.1.1 192.168.1.2";; *) exit 1;; esac; for ip in $P; do ping -c1 -W2 "$ip" >/dev/null 2>&1 || exit 1; done"#,
            proof: r#"case "${BDD_ROLE:-}" in mgm) P="192.168.1.2 192.168.1.3";; n1) P="192.168.1.1 192.168.1.3";; n2) P="192.168.1.1 192.168.1.2";; *) P="192.168.1.1 192.168.1.2 192.168.1.3";; esac; for ip in $P; do ping -c2 -W2 "$ip"; done"#,
        },
        // ---- EX03: MySQL Cluster ----
        Step {
            ex: 3, st: 1, roles: MGM,
            title: "Instalar o gerenciador do cluster",
            script: include_str!("../EX03/01_MGM_instala-gerenciador.bash"),
            validate: "pgrep -x ndb_mgmd >/dev/null 2>&1",
            proof: "ndb_mgm -e show",
        },
        Step {
            ex: 3, st: 2, roles: DATA,
            title: "Instalar o nó de dados (ndbd + mysqld)",
            script: include_str!("../EX03/02_N1-N2_instala-no-de-dados.bash"),
            validate: "pgrep -x ndbd >/dev/null 2>&1 && { pgrep -x mysqld >/dev/null 2>&1 || pgrep -x mysqld_safe >/dev/null 2>&1; }",
            proof: "pgrep -al ndbd; pgrep -al mysqld 2>/dev/null || pgrep -al mysqld_safe",
        },
        Step {
            ex: 3, st: 3, roles: MGM,
            title: "Verificar o cluster (nós conectados)",
            script: include_str!("../EX03/03_MGM_verifica-cluster.bash"),
            validate: "n=$(ndb_mgm -e show 2>/dev/null | grep -cE '@192\\.168\\.1\\.[23] '); [ \"${n:-0}\" -ge 2 ]",
            proof: "ndb_mgm -e show",
        },
        Step {
            ex: 3, st: 4, roles: N1,
            title: "Criar banco e inserir dados",
            script: include_str!("../EX03/04_N1_cria-banco-e-insere.bash"),
            validate: "mysql -u root -Nse 'SELECT count(*) FROM clusterdb.funcionarios' 2>/dev/null | grep -q '^3$'",
            proof: "mysql -u root -e 'SELECT * FROM clusterdb.funcionarios;'",
        },
        Step {
            ex: 3, st: 5, roles: N2,
            title: "Verificar a replicação no N2",
            script: include_str!("../EX03/05_N2_verifica-replicacao.bash"),
            validate: "mysql -u root -Nse 'SELECT count(*) FROM clusterdb.funcionarios' 2>/dev/null | grep -q '^3$'",
            proof: "mysql -u root -e 'SELECT * FROM clusterdb.funcionarios;'",
        },
    ]
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
