mod annotations;
mod app;
mod clipboard;
mod diff;
mod pi_extension;
mod tmux;
mod ui;
mod vim;

use std::fs;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use annotations::output;
use app::App;
use diff::parser::parse_unified_diff;

#[derive(Parser)]
#[command(name = "pdiff", about = "Terminal diff reviewer with vim motions")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Read diff from file instead of stdin
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Write annotations to this file (skips interactive prompt)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Print annotations to stdout instead of file
    #[arg(long)]
    stdout: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Install pdiff integration
    Install {
        /// Target: "pi"
        target: String,
    },
    /// Uninstall pdiff integration
    Uninstall {
        /// Target: "pi"
        target: String,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    if let Some(cmd) = &cli.command {
        return match cmd {
            Command::Install { target } => pi_extension::install(target),
            Command::Uninstall { target } => pi_extension::uninstall(target),
        };
    }

    let input = read_diff_input(&cli)?;

    let files = parse_unified_diff(&input);
    if files.is_empty() {
        eprintln!("No parseable diff found.");
        std::process::exit(0);
    }

    let app = App::new(files);
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    let annotations = result?;

    if cli.stdout {
        output::print_markdown(&annotations);
        return Ok(());
    } else if let Some(path) = &cli.output {
        output::write_markdown(&annotations, path)?;
        eprintln!("Wrote {} comment(s) to {}", annotations.len(), path.display());
        return Ok(());
    }

    if annotations.is_empty() {
        eprintln!("No comments.");
        return Ok(());
    }

    match prompt_save_tty(annotations.len()) {
        Ok(true) => {
            output::write_markdown(&annotations, &PathBuf::from("pdiff-review.md"))?;
            eprintln!("Saved to pdiff-review.md.");
        }
        Ok(false) => {
            eprintln!("\n{}", output::format_markdown(&annotations));
        }
        Err(_) => {
            output::write_markdown(&annotations, &PathBuf::from("pdiff-review.md"))?;
            eprintln!("Wrote {} comment(s) to pdiff-review.md", annotations.len());
        }
    }

    Ok(())
}

fn read_diff_input(cli: &Cli) -> io::Result<String> {
    if let Some(path) = &cli.input {
        return fs::read_to_string(path);
    }

    if io::stdin().is_terminal() {
        eprintln!("Usage: git diff | pdiff");
        eprintln!("       pdiff --input diff.patch");
        std::process::exit(1);
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;

    if input.trim().is_empty() {
        eprintln!("No diff input received.");
        std::process::exit(0);
    }

    Ok(input)
}

fn prompt_save_tty(count: usize) -> io::Result<bool> {
    let tty = fs::OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    let mut writer = io::BufWriter::new(&tty);
    let mut reader = io::BufReader::new(&tty);

    write!(writer, "Save {} comment(s) to pdiff-review.md? [y/N] ", count)?;
    writer.flush()?;

    let mut answer = String::new();
    reader.read_line(&mut answer)?;
    Ok(answer.trim().eq_ignore_ascii_case("y"))
}
