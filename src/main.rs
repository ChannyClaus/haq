use rustyline::completion::{Completer, Pair};
use rustyline::config::{CompletionType, Config};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};
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

    println!("haq-repl: Hack (HHVM) REPL (echo mode)");
    println!("Type \\q to exit");

    loop {
        let line = rl.readline("haq> ")?;
        let line = line.trim();
        match line {
            ":quit" | "\\q" => break,
            "" => continue,
            _ => {
                rl.add_history_entry(line)?;
                rl.helper_mut().unwrap().add_from_line(line);
                println!("{}", line);
            }
        }
    }

    rl.save_history(&hist_path)?;
    Ok(())
}
