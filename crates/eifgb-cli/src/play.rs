//! `eifgb play`: a plain terminal host for the engine.

use eifgb_engine::ir::Story;
use eifgb_engine::{Game, LineKind, Output};
use eifgb_lua::LuaHost;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

const WIDTH: usize = 78;

fn wrap(text: &str, width: usize) -> String {
    let mut out = String::new();
    for (i, para) in text.split("\n\n").enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        let mut col = 0;
        for word in para.split_whitespace() {
            let len = word.chars().count();
            if col > 0 && col + 1 + len > width {
                out.push('\n');
                col = 0;
            } else if col > 0 {
                out.push(' ');
                col += 1;
            }
            out.push_str(word);
            col += len;
        }
    }
    out
}

fn show(out: &Output, color: bool) {
    let (bold, dim, red, reset) =
        if color { ("\x1b[1m", "\x1b[2m", "\x1b[31m", "\x1b[0m") } else { ("", "", "", "") };
    for line in &out.lines {
        let text = wrap(&line.text, WIDTH);
        match line.kind {
            LineKind::Banner => println!("\n{bold}{}{reset}\n", text.to_uppercase()),
            LineKind::Title => println!("\n{bold}{text}{reset}"),
            LineKind::Text => println!("{text}"),
            LineKind::Notice => println!("{dim}{text}{reset}"),
            LineKind::Choice => println!("  {text}"),
            LineKind::Ending => println!("\n{bold}{text}{reset}"),
            LineKind::Error => println!("{red}{text}{reset}"),
        }
    }
}

pub fn play(story: Story, commands: Option<&Path>, seed: Option<u64>, save_file: Option<PathBuf>) -> Result<(), String> {
    let save_path = save_file.unwrap_or_else(|| PathBuf::from(format!("{}.sav", story.meta.id)));
    let seed = seed.unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    });
    let host = LuaHost::new().map_err(|e| e.to_string())?;
    let mut game = Game::new(story, host, seed).map_err(|e| e.to_string())?;

    let scripted: Option<Vec<String>> = match commands {
        Some(p) => Some(
            std::fs::read_to_string(p)
                .map_err(|e| format!("{}: {e}", p.display()))?
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(String::from)
                .collect(),
        ),
        None => None,
    };
    let color = scripted.is_none() && io::stdout().is_terminal();

    show(&game.start(), color);
    let stdin = io::stdin();
    let mut keyboard = stdin.lock().lines();
    let mut script = scripted.map(|v| v.into_iter());

    loop {
        print!("\n> ");
        io::stdout().flush().ok();
        let line = match &mut script {
            Some(it) => match it.next() {
                Some(l) => {
                    println!("{l}");
                    l
                }
                None => break,
            },
            None => match keyboard.next() {
                Some(Ok(l)) => l,
                _ => break,
            },
        };

        let mut out = game.input(&line);
        if let Some(data) = out.save.take()
            && let Err(e) = std::fs::write(&save_path, data)
        {
            let text = format!("[couldn't write {}: {e}]", save_path.display());
            out.lines.push(eifgb_engine::Line { kind: LineKind::Error, text });
        }
        show(&out, color);
        if out.restore_requested {
            match std::fs::read_to_string(&save_path) {
                Ok(data) => show(&game.restore(&data), color),
                Err(_) => println!("[no saved game at {}]", save_path.display()),
            }
        }
        if out.quit_requested {
            break;
        }
    }
    let s = &game.world().state;
    let turns = if s.turn == 1 { "turn" } else { "turns" };
    match game.world().clock() {
        Some(c) => println!("\n[{} {turns}, score {}, {} on day {}]", s.turn, s.score, c.time, c.day),
        None => println!("\n[{} {turns}, score {}]", s.turn, s.score),
    }
    Ok(())
}
