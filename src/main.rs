mod args;
mod boundaries;
mod gui;
mod patterns;
mod pipeline;
mod sections;
mod state;
mod storage;
mod tui;
mod worker;

use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use eframe::egui;

fn main() -> Result<()> {
    let args = args::Args::parse();

    let file      = std::fs::File::open(&args.file)?;
    let file_size = file.metadata()?.len();
    let workers   = args.workers.unwrap_or_else(available_threads);

    // Detect whether the file lives on network storage
    let remote = if args.force_remote {
        true
    } else if args.force_local {
        false
    } else {
        storage::is_remote(&file)?
    };

    drop(file); // pipeline opens the file itself

    let state = Arc::new(state::AppState::new(file_size, remote, args.gui));

    state.log(&format!(
        "file-parser: {} | {:.2} GB | {} | {} worker{}",
        args.file.display(),
        file_size as f64 / 1e9,
        if remote { "remote" } else { "local" },
        workers,
        if workers == 1 { "" } else { "s" },
    ));

    // Spawn the parser pipeline in a background thread so the UI stays responsive
    {
        let state = Arc::clone(&state);
        let path  = args.file.clone();

        std::thread::spawn(move || {
            let result = if remote {
                pipeline::remote::run(&path, Arc::clone(&state), workers)
            } else {
                pipeline::local::run(&path, Arc::clone(&state), workers)
            };
            if let Err(e) = result {
                state.log(&format!("pipeline error: {e}"));
            }
            state.set_complete();
        });
    }

    // Run the chosen UI — both block until the user exits or parsing completes
    if args.gui {
        let state_gui = Arc::clone(&state);
        eframe::run_native(
            "File Parser",
            eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_title("File Parser")
                    .with_inner_size([800.0, 600.0]),
                ..Default::default()
            },
            Box::new(move |_cc| Ok(Box::new(gui::App::new(state_gui)))),
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    } else {
        tui::run(Arc::clone(&state));
    }

    Ok(())
}

fn available_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
