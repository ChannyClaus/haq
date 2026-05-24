use rustyline::completion::{Completer, Pair};
use rustyline::config::{CompletionType, Config};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
use std::io::Write;
use std::process::Command;
use std::sync::{Arc, Mutex};

struct HackHelper {
    words: Arc<Mutex<Vec<String>>>,
}

impl HackHelper {
    fn new() -> Self {
        let builtins = vec![
            "echo", "function", "return", "if", "else", "while", "for",
            "foreach", "class", "interface", "trait", "enum", "type",
            "new", "this", "self", "parent", "true", "false", "null",
            "int", "string", "bool", "float", "void", "mixed",
            "array", "Vector", "Map", "Set", "Pair",
            "async", "await", "use", "namespace", "require", "include",
            "define", "isset", "empty", "unset", "die", "exit",
            "var_dump", "print", "invariant", "invariant_violation",
        ];
        HackHelper {
            words: Arc::new(Mutex::new(builtins.into_iter().map(String::from).collect())),
        }
    }

    fn add_word(&self, word: &str) {
        let mut words = self.words.lock().unwrap();
        if !words.contains(&word.to_string()) {
            words.push(word.to_string());
        }
    }

    fn add_from_line(&self, line: &str) {
        for w in line.split(|c: char| {
            c.is_whitespace() || c == '(' || c == ')' || c == '{' || c == '}' || c == ';' || c == ','
        }) {
            let w = w.trim();
            if w.is_empty() {
                continue;
            }
            if w.starts_with('$') {
                self.add_word(w);
            }
        }
    }
}

impl Completer for HackHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>), ReadlineError> {
        let line = &line[..pos];
        let word_start = line.rfind(|c: char| c.is_whitespace()).map(|i| i + 1).unwrap_or(0);
        let partial = &line[word_start..];

        let words = self.words.lock().unwrap();
        let candidates: Vec<Pair> = words
            .iter()
            .filter(|w| w.starts_with(partial) && !w.is_empty())
            .map(|w| Pair {
                display: w.to_string(),
                replacement: w.to_string(),
            })
            .collect();

        Ok((word_start, candidates))
    }
}

impl Hinter for HackHelper {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}

impl Highlighter for HackHelper {}

impl Validator for HackHelper {}

impl Helper for HackHelper {}

fn history_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::Path::new(&home).join(".haq_history")
}

fn write_and_run(code: &str) -> Result<(String, String), String> {
    let mut child = Command::new("docker")
        .args(["exec", "-i", "hhvm", "sh", "-c", "cat > /tmp/repl.php"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("docker exec (write): {}", e))?;
    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(code.as_bytes()).map_err(|e| format!("write code: {}", e))?;
    }
    drop(child.stdin.take());
    child.wait().map_err(|e| format!("wait write: {}", e))?;

    let output = Command::new("docker")
        .args(["exec", "hhvm", "hhvm", "/tmp/repl.php"])
        .output()
        .map_err(|e| format!("docker exec (hhvm): {}", e))?;

    Ok((
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    ))
}

fn is_decl_keyword(line: &str) -> bool {
    let keywords = ["function", "class", "interface", "trait", "enum", "type", "newtype"];
    let first = line.trim().split_whitespace().next().unwrap_or("");
    keywords.contains(&first)
}

fn needs_echo(line: &str) -> bool {
    let line = line.trim();
    if line.is_empty()
        || line.ends_with(';')
        || line.ends_with('{')
        || line.ends_with('}')
        || line.ends_with(',')
    {
        return false;
    }
    if line.starts_with("echo ") || line.starts_with("print ") || line.starts_with("return ") {
        return false;
    }
    if line.contains('=') && !line.starts_with("== ") && !line.starts_with("!= ") {
        return false;
    }
    let stmt_keywords = [
        "if", "else", "while", "for", "foreach",
        "switch", "case", "default", "try", "catch", "finally",
        "return", "break", "continue", "throw",
    ];
    let first = line.split_whitespace().next().unwrap_or("");
    !stmt_keywords.contains(&first)
}

fn main() -> rustyline::Result<()> {
    let helper = HackHelper::new();
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .max_history_size(1000)?
        .build();
    let mut rl = Editor::with_config(config)?;
    rl.set_helper(Some(helper));

    let hist_path = history_path();
    let _ = rl.load_history(&hist_path);

    println!("haq-repl: Hack (HHVM) REPL");
    println!("Type \\q to exit");

    let mut decls = String::new();
    let mut body = String::new();

    loop {
        let line = rl.readline("haq> ")?;
        let line = line.trim();
        match line {
            ":quit" | "\\q" => break,
            "" => continue,
            _ => {
                rl.add_history_entry(line)?;
                rl.helper_mut().unwrap().add_from_line(line);

                let next = if is_decl_keyword(line) {
                    decls.push_str(line);
                    decls.push('\n');
                    continue;
                } else if needs_echo(line) {
                    format!("  echo {} . \"\\n\";\n", line)
                } else {
                    let line = if line.ends_with(';') {
                        line.to_string()
                    } else {
                        format!("{};", line)
                    };
                    format!("  {}\n", line)
                };

                let program = format!(
                    "<?hh\n{}\n<<__EntryPoint>>\nfunction main(): void {{\n{}\n  echo \"---HAQ---\" . \"\\n\";\n{}\n}}\n",
                    decls, body, next
                );

                match write_and_run(&program) {
                    Ok((stdout, stderr)) => {
                        let fresh = stdout
                            .rsplitn(2, "---HAQ---")
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_string();
                        if !fresh.is_empty() {
                            println!("{}", fresh);
                        }
                        if !stderr.is_empty() {
                            eprint!("{}", stderr);
                        }
                        std::io::stdout().flush().ok();
                        std::io::stderr().flush().ok();
                    }
                    Err(msg) => {
                        eprintln!("[hhvm] {}", msg);
                    }
                }

                body.push_str(&next);
            }
        }
    }

    rl.save_history(&hist_path)?;
    Ok(())
}
