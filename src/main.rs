mod args;
mod boundaries;
mod gui;
mod patterns;
mod pipeline;
mod sections;
mod source;
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

    let src = source::Source::parse(&args.file);

    // For file sources: open to get size and detect local/remote.
    // For URL sources: size is unknown until the HTTP response arrives;
    //                  the pipeline will update net_bytes_total once connected.
    let (file_size, remote) = match src.as_path() {
        Some(path) => {
            let file = std::fs::File::open(path)?;
            let size = file.metadata()?.len();
            let remote = if args.force_remote {
                true
            } else if args.force_local {
                false
            } else {
                storage::is_remote(&file)?
            };
            (size, remote)
        }
        None => (0u64, true), // URL — always remote, size learned later
    };

    let workers        = args.workers.unwrap_or_else(available_threads);
    let silent         = args.gui || args.quiet;
    let transfer_label = src.transfer_label();

    let state = Arc::new(state::AppState::new(file_size, remote, transfer_label, silent));

    state.log(&format!(
        "file-parser: {} | {} | {} | {} worker{}",
        src.display(),
        if file_size > 0 { format!("{:.2} GB", file_size as f64 / 1e9) }
                         else { "size unknown".to_string() },
        if remote { "remote" } else { "local" },
        workers,
        if workers == 1 { "" } else { "s" },
    ));

    // Spawn the parser pipeline in a background thread
    {
        let state = Arc::clone(&state);

        std::thread::spawn(move || {
            let result = if remote {
                pipeline::remote::run(src, Arc::clone(&state), workers)
            } else {
                // src is guaranteed to be a File here (URLs are always remote)
                let path = src.as_path().unwrap().to_path_buf();
                pipeline::local::run(&path, Arc::clone(&state), workers)
            };
            if let Err(e) = result {
                state.log(&format!("pipeline error: {e}"));
            }
            state.set_complete();
        });
    }

    // Ctrl-C handler for non-GUI modes
    if !args.gui {
        let state = Arc::clone(&state);
        ctrlc::set_handler(move || {
            state.cancel();
            state.set_complete();
        })
        .ok();
    }

    // Run the chosen UI
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
    } else if args.quiet {
        while !state.is_complete() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
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
