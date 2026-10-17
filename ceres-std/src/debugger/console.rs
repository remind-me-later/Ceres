//! The console: reads commands in the terminal while the game runs in its
//! window, with history and completion of commands and symbols. Ctrl-C
//! stops the game; Ctrl-D closes the console and lets the game run on.

use {
    super::{Debugger, commands},
    ceres_core::AudioCallback,
    rustyline::{
        Context, Editor, ExternalPrinter as _, Helper, completion::Completer, error::ReadlineError,
        highlight::Highlighter, hint::Hinter, history::DefaultHistory, validate::Validator,
    },
    std::{io, path::PathBuf, sync::Arc, thread::JoinHandle},
};

/// Completes command names, then symbols.
struct Completion<A: AudioCallback> {
    debugger: Arc<Debugger<A>>,
}

impl<A: AudioCallback> Completer for Completion<A> {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        let before = line.get(..pos).unwrap_or(line);
        let start = before
            .rfind(|c: char| c.is_whitespace() || "([{+-*=,!~<>&|^".contains(c))
            .map_or(0, |at| at + 1);
        let word = before.get(start..).unwrap_or_default();
        let candidates = if start == 0 {
            // The command: whatever comes before modifiers.
            commands::names()
                .filter(|name| name.starts_with(word))
                .map(str::to_owned)
                .collect()
        } else {
            let state = self.debugger.state();
            state.symbols.as_ref().map_or_else(Vec::new, |symbols| {
                symbols
                    .complete(word)
                    .take(200)
                    .map(str::to_owned)
                    .collect()
            })
        };
        Ok((start, candidates))
    }
}

impl<A: AudioCallback> Hinter for Completion<A> {
    type Hint = String;
}

impl<A: AudioCallback> Highlighter for Completion<A> {}

impl<A: AudioCallback> Validator for Completion<A> {}

impl<A: AudioCallback> Helper for Completion<A> {}

const BANNER: &str = "Ceres debugger: `help` lists the commands, Ctrl-C stops the game, \
                      Ctrl-D closes the console.";

/// Starts the console on the terminal, in its own thread. `history` is the
/// file that keeps the commands between runs.
///
/// # Errors
///
/// Returns an error if the terminal can't be used or the thread can't
/// start.
#[inline]
pub fn spawn<A: AudioCallback + Send + 'static>(
    debugger: Arc<Debugger<A>>,
    history: Option<PathBuf>,
) -> io::Result<JoinHandle<()>> {
    let mut editor: Editor<Completion<A>, DefaultHistory> =
        Editor::new().map_err(io::Error::other)?;
    editor.set_helper(Some(Completion {
        debugger: Arc::clone(&debugger),
    }));
    if let Some(history) = &history
        && editor.load_history(history).is_err()
    {
        // The first run has none.
    }
    // Stop messages are printed above the prompt, from the emulator thread.
    if let Ok(mut printer) = editor.create_external_printer() {
        debugger.set_printer(move |message| {
            if let Err(e) = printer.print(format!("{message}\n")) {
                eprintln!("debugger: {e}");
            }
        });
    }

    std::thread::Builder::new()
        .name("debugger".to_owned())
        .spawn(move || {
            println!("{BANNER}");
            run(&debugger, &mut editor, history.as_ref());
        })
}

fn run<A: AudioCallback>(
    debugger: &Debugger<A>,
    editor: &mut Editor<Completion<A>, DefaultHistory>,
    history: Option<&PathBuf>,
) {
    loop {
        match editor.readline("> ") {
            Ok(line) => {
                if !line.trim().is_empty() {
                    let saved = editor.add_history_entry(line.as_str()).and_then(|_added| {
                        history.map_or(Ok(()), |history| editor.save_history(history))
                    });
                    if let Err(e) = saved {
                        eprintln!("debugger: history: {e}");
                    }
                }
                let output = debugger.execute(&line);
                if !output.is_empty() {
                    println!("{output}");
                }
            }
            Err(ReadlineError::Interrupted) => {
                if debugger.is_stopped() {
                    println!("the game is already stopped (Ctrl-D closes the console)");
                } else {
                    debugger.interrupt();
                }
            }
            Err(ReadlineError::Eof) => {
                debugger.detach();
                println!("console closed: the game runs on");
                return;
            }
            Err(e) => {
                eprintln!("debugger: {e}");
                debugger.detach();
                return;
            }
        }
    }
}
