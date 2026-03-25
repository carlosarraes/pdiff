mod annotations;
mod app;
mod diff;
mod ui;
mod vim;

use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;

use clap::Parser;

use annotations::output;
use app::App;
use diff::parser::parse_unified_diff;

#[derive(Parser)]
#[command(name = "pdiff", about = "Terminal diff reviewer with vim motions")]
struct Cli {
    /// Output file path (default: pdiff-review.md)
    #[arg(short, long, default_value = "pdiff-review.md")]
    output: PathBuf,

    /// Print annotations to stdout instead of file
    #[arg(long)]
    stdout: bool,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let mut input = String::new();
    if io::stdin().is_terminal() {
        eprintln!("Usage: git diff | pdiff");
        eprintln!("       pdiff < diff.patch");
        std::process::exit(1);
    }
    io::stdin().read_to_string(&mut input)?;

    if input.trim().is_empty() {
        eprintln!("No diff input received.");
        std::process::exit(0);
    }

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
    } else {
        output::write_markdown(&annotations, &cli.output)?;
        if annotations.is_empty() {
            eprintln!("No comments. Wrote empty review to {}.", cli.output.display());
        } else {
            eprintln!(
                "Wrote {} comment(s) to {}",
                annotations.len(),
                cli.output.display()
            );
        }
    }

    Ok(())
}
