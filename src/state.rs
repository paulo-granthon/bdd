//! Estado persistente em /var/lib/bdd/state (gravado por qualquer usuário;
//! o instalador deixa o diretório com permissão de escrita).

use crate::model::Role;
use std::fs;

/// Diretório do estado. Padrão /var/lib/bdd; pode ser trocado via BDD_DIR (testes).
fn dir() -> String {
    std::env::var("BDD_DIR").unwrap_or_else(|_| "/var/lib/bdd".to_string())
}
fn file() -> String {
    format!("{}/state", dir())
}

pub struct State {
    pub user_role: Option<Role>,
    pub ran: Vec<String>,
    pub checked: Vec<String>,
}

impl State {
    pub fn load() -> State {
        let mut st = State {
            user_role: None,
            ran: Vec::new(),
            checked: Vec::new(),
        };
        if let Ok(txt) = fs::read_to_string(file()) {
            for line in txt.lines() {
                let line = line.trim();
                if let Some(v) = line.strip_prefix("user_role=") {
                    st.user_role = Role::from_str(v);
                } else if let Some(v) = line.strip_prefix("ran=") {
                    st.ran = split_csv(v);
                } else if let Some(v) = line.strip_prefix("checked=") {
                    st.checked = split_csv(v);
                }
            }
        }
        st
    }

    pub fn save(&self) {
        let _ = fs::create_dir_all(dir());
        let body = format!(
            "user_role={}\nran={}\nchecked={}\n",
            self.user_role.map(|r| r.code()).unwrap_or(""),
            self.ran.join(","),
            self.checked.join(","),
        );
        if fs::write(file(), body).is_err() {
            eprintln!(
                "[bdd] aviso: não consegui gravar o estado em {}. Rode com sudo, ou reinstale.",
                file()
            );
        }
    }

    pub fn mark_ran(&mut self, id: &str) {
        if !self.ran.iter().any(|x| x == id) {
            self.ran.push(id.to_string());
            self.save();
        }
    }

    pub fn mark_checked(&mut self, id: &str) {
        if !self.checked.iter().any(|x| x == id) {
            self.checked.push(id.to_string());
            self.save();
        }
    }

    pub fn has_ran(&self, id: &str) -> bool {
        self.ran.iter().any(|x| x == id)
    }
    pub fn has_checked(&self, id: &str) -> bool {
        self.checked.iter().any(|x| x == id)
    }
}

fn split_csv(v: &str) -> Vec<String> {
    v.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

