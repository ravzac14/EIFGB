use clap::{Parser, Subcommand};
use eifgb_compiler::{compile, DirFiles, Output};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod play;

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
    /// Play a game in the terminal.
    Play {
        game: PathBuf,
        /// Run the commands in this file (one per line) instead of reading
        /// the keyboard, echoing each one. Useful for walkthroughs.
        #[arg(long)]
        commands: Option<PathBuf>,
        /// Random seed. The same seed and commands always give the same game.
        #[arg(long)]
        seed: Option<u64>,
        /// Where SAVE and RESTORE keep the saved game. Defaults to <game id>.sav.
        #[arg(long)]
        save_file: Option<PathBuf>,
    },
}

pub(crate) fn run_compile(game: &Path) -> Result<Output, String> {
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
        Command::Play { game, commands, seed, save_file } => run_compile(&game).and_then(|o| {
            let Some(story) = o.story else { return Ok(false) };
            play::play(story, commands.as_deref(), seed, save_file).map(|()| true)
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
