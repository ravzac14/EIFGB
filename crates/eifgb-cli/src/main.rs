use clap::{Parser, Subcommand};
use eifgb_compiler::{compile, DirFiles, Output};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "eifgb", version, about = "Build interactive fiction games from EIFGB source")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate a game and report errors and warnings.
    Check {
        /// Path to the game's entry file (usually game.yaml).
        game: PathBuf,
    },
    /// Compile a game to Story IR (JSON).
    Build {
        game: PathBuf,
        /// Output file. Defaults to <game id>.story.json next to the entry file.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Pretty-print the JSON.
        #[arg(long)]
        pretty: bool,
    },
}

fn run_compile(game: &Path) -> Result<Output, String> {
    let root = game.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let entry = game
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("bad path: {}", game.display()))?;
    let out = compile(&DirFiles { root: root.to_path_buf() }, entry);
    for d in &out.diagnostics {
        eprintln!("{d}");
    }
    let (e, w) = (out.errors().count(), out.warnings().count());
    if e + w > 0 {
        eprintln!("{e} error(s), {w} warning(s)");
    }
    Ok(out)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Check { game } => run_compile(&game).map(|out| out.story.is_some()),
        Command::Build { game, out, pretty } => run_compile(&game).and_then(|o| {
            let Some(story) = o.story else { return Ok(false) };
            let path = out.unwrap_or_else(|| {
                game.with_file_name(format!("{}.story.json", story.meta.id))
            });
            let json = if pretty {
                serde_json::to_string_pretty(&story)
            } else {
                serde_json::to_string(&story)
            }
            .map_err(|e| e.to_string())?;
            std::fs::write(&path, json).map_err(|e| format!("{}: {e}", path.display()))?;
            eprintln!("wrote {}", path.display());
            Ok(true)
        }),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
