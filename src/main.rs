use std::io;
use std::process;

use clap::{Parser, Subcommand};
use mdbook_preprocessor::{MDBOOK_VERSION, Preprocessor, parse_input};
use semver::{Version, VersionReq};

use mdbook_tracey::Tracey;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Check whether this preprocessor supports the given renderer.
    Supports { renderer: String },
}

fn main() {
    let cli = Cli::parse();
    let pre = Tracey;

    match cli.command {
        Some(Command::Supports { renderer }) => {
            let supported = pre.supports_renderer(&renderer).unwrap_or(false);
            process::exit(if supported { 0 } else { 1 });
        }
        None => {
            if let Err(e) = preprocess(&pre) {
                eprintln!("mdbook-tracey: {e:#}");
                process::exit(1);
            }
        }
    }
}

fn preprocess(pre: &dyn Preprocessor) -> anyhow::Result<()> {
    let (ctx, book) = parse_input(io::stdin())?;

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let compiled = VersionReq::parse(MDBOOK_VERSION)?;
    if !compiled.matches(&book_version) {
        eprintln!(
            "mdbook-tracey: warning: compiled against mdbook {MDBOOK_VERSION}, invoked by mdbook {}",
            ctx.mdbook_version
        );
    }

    let out = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &out)?;
    Ok(())
}
