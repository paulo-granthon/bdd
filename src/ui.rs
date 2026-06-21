//! Cores ANSI e símbolos usados na saída.

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";

pub const GREEN: &str = "\x1b[32m";
pub const CYAN: &str = "\x1b[36m";
pub const YELLOW: &str = "\x1b[33m";
pub const BLUE: &str = "\x1b[34m"; // azul escuro
pub const RED: &str = "\x1b[31m";
pub const DARK_RED: &str = "\x1b[38;5;88m"; // vermelho mais escuro
pub const FADED: &str = "\x1b[2m"; // texto apagado (default apagado)
pub const FADED_RED: &str = "\x1b[2;31m";

pub const CHECK: &str = "✓";
pub const CROSS: &str = "✗";
pub const ARROW: &str = "←";
pub const BALL: &str = "●";
pub const BANG: &str = "!";

pub fn paint(color: &str, s: &str) -> String {
    format!("{}{}{}", color, s, RESET)
}

/// Cabeçalho de seção.
pub fn header(s: &str) {
    println!("{}{}{}", BOLD, s, RESET);
}

/// Bloco de "próximos passos" no fim de cada comando.
pub fn proximo(linhas: &[String]) {
    println!();
    println!("{}{}Próximo:{}", BOLD, CYAN, RESET);
    for l in linhas {
        println!("  {}", l);
    }
}
