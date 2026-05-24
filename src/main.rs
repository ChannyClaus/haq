use rustyline::DefaultEditor;

fn main() -> rustyline::Result<()> {
    let mut rl = DefaultEditor::new()?;
    println!("haq-repl: Hack (HHVM) REPL (echo mode)");
    println!("Type \\q to exit");

    loop {
        let line = rl.readline("haq> ")?;
        let line = line.trim();
        match line {
            ":quit" | "\\q" => break,
            "" => continue,
            _ => println!("{}", line),
        }
    }

    Ok(())
}
