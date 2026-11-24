/// A compact, original hannya-inspired mask for the terminal.
pub const HANNYA: &str = r#"
        ╱╲                 ╱╲
       ╱  ╲_______________╱  ╲
      ╱    ╲             ╱    ╲
     ╱      ╲  ◉     ◉  ╱      ╲
    │        ╲         ╱        │
    │         ╲  ╱─╲  ╱         │
     ╲         ╲╱   ╲╱         ╱
      ╲         ╲   ╱         ╱
       ╲_________╲ ╱_________╱
                ╲╱
"#;

pub const WORDMARK: &str = r#"
  ██████╗ ███╗   ██╗██╗
 ██╔═══██╗████╗  ██║██║
 ██║   ██║██╔██╗ ██║██║
 ██║   ██║██║╚██╗██║██║
 ╚██████╔╝██║ ╚████║██║
  ╚═════╝ ╚═╝  ╚═══╝╚═╝
"#;

pub fn splash() -> String {
    format!("{HANNYA}\n{WORDMARK}\n")
}
