//! Detecção do papel da máquina pelo IP interno (192.168.1.1/.2/.3),
//! definido no EX02. Quando detectável, vale sobre o `bdd id` do usuário.

use crate::model::Role;
use std::process::Command;

/// Papel detectado pelo ambiente (IP interno), se houver.
pub fn detected() -> Option<Role> {
    let out = Command::new("hostname").arg("-I").output().ok()?;
    let ips = String::from_utf8_lossy(&out.stdout);
    for tok in ips.split_whitespace() {
        match tok {
            "192.168.1.1" => return Some(Role::Mgm),
            "192.168.1.2" => return Some(Role::N1),
            "192.168.1.3" => return Some(Role::N2),
            _ => {}
        }
    }
    None
}

/// (papel efetivo, origem). origem: "detectado" | "você definiu" | "".
pub fn effective(user: Option<Role>) -> (Option<Role>, &'static str) {
    if let Some(r) = detected() {
        (Some(r), "detectado")
    } else if let Some(r) = user {
        (Some(r), "você definiu")
    } else {
        (None, "")
    }
}
