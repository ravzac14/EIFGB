//! EIFGB compiler: source (YAML + Lua) → Story IR.
//!
//! ```no_run
//! use eifgb_compiler::{compile, DirFiles};
//! let out = compile(&DirFiles { root: "my_game".into() }, "game.yaml");
//! for d in &out.diagnostics { eprintln!("{d}"); }
//! let story = out.story.expect("compile errors");
//! ```

mod diag;
mod files;
mod lower;
mod pattern;
pub mod source;
mod template;
mod util;

pub use diag::{Diagnostic, Severity};
pub use files::{DirFiles, FileProvider, MemFiles};
pub use lower::visit_code;

use eifgb_ir::Story;

#[derive(Debug)]
pub struct Output {
    /// Present only when there were no errors.
    pub story: Option<Story>,
    /// Errors first, then warnings, each group in source order.
    pub diagnostics: Vec<Diagnostic>,
}

impl Output {
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.severity == Severity::Warning)
    }
}

/// Compiles the game whose entry file is `entry`. Script paths are
/// resolved relative to the entry file's directory.
pub fn compile(files: &dyn FileProvider, entry: &str) -> Output {
    let mut d = diag::Diagnostics::default();
    let story = match files.read(entry) {
        Err(e) => {
            d.error("", e);
            None
        }
        Ok(text) => match serde_yaml::from_str::<source::Source>(&text) {
            Err(e) => {
                d.error("", format!("{entry}: {e}"));
                None
            }
            Ok(src) => lower::Lowerer::new(&mut d, files, entry).lower(src),
        },
    };
    let mut diagnostics = d.0;
    diagnostics.sort_by_key(|d| d.severity); // stable: keeps source order within each group
    Output { story, diagnostics }
}

/// Compiles a single YAML document that uses no external script files.
pub fn compile_str(yaml: &str) -> Output {
    compile(&MemFiles::new().with("game.yaml", yaml), "game.yaml")
}
