use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::state::{AppState, WorkerStatus};

const POLL_MS: u64 = 100;

/// Run the TUI progress display, blocking until parsing is complete or cancelled.
pub fn run(state: Arc<AppState>) {
    let mp = MultiProgress::new();

    // Transfer progress bar — style depends on remote vs local, and whether
    // the total size is known (URLs may not provide Content-Length).
    let net_total = state.net_bytes_total.load(Ordering::Relaxed);
    let net_bar = if state.remote {
        let bar = mp.add(if net_total > 0 {
            ProgressBar::new(net_total)
        } else {
            ProgressBar::new_spinner()
        });
        if net_total > 0 {
            bar.set_style(
                ProgressStyle::with_template(
                    "{spinner:.cyan} {msg} [{bar:45.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})",
                )
                .unwrap()
                .progress_chars("█▉▊▋▌▍▎▏░"),
            );
        } else {
            bar.set_style(
                ProgressStyle::with_template(
                    "{spinner:.cyan} {msg}  {bytes} downloaded ({bytes_per_sec})",
                )
                .unwrap(),
            );
        }
        bar.set_message(state.transfer_label);
        bar.enable_steady_tick(Duration::from_millis(80));
        bar
    } else {
        let bar = mp.add(ProgressBar::new(0));
        bar.set_style(ProgressStyle::with_template("{msg}").unwrap());
        bar.finish_with_message(format!("{}  [local file, skipped]", state.transfer_label));
        bar
    };

    let worker_style = ProgressStyle::with_template(
        "{spinner:.green} {msg:<25} [{bar:45.green/dim}] {percent:>3}%  {pos}/{len} bytes  {wide_msg}",
    )
    .unwrap()
    .progress_chars("█▉░");

    let mut known_workers = 0;
    let mut worker_bars: Vec<ProgressBar> = Vec::new();

    loop {
        // Update transfer bar — tick advances the spinner; set_position updates
        // the bar when the total is known.
        let net_done = state.net_bytes_done.load(Ordering::Relaxed);
        if state.remote {
            let net_total = state.net_bytes_total.load(Ordering::Relaxed);
            if net_total > 0 {
                net_bar.set_length(net_total);
                net_bar.set_position(net_done);
            }
        }

        // Add bars for any newly registered workers
        let workers = state.workers.lock().unwrap();
        while known_workers < workers.len() {
            let w   = &workers[known_workers];
            let bar = mp.add(ProgressBar::new(w.bytes_total.load(Ordering::Relaxed)));
            bar.set_style(worker_style.clone());
            bar.set_message(format!("[{}]", w.section_name));
            bar.enable_steady_tick(Duration::from_millis(80));
            worker_bars.push(bar);
            known_workers += 1;
        }
        drop(workers);

        // Update existing worker bars
        let workers = state.workers.lock().unwrap();
        for (bar, worker) in worker_bars.iter().zip(workers.iter()) {
            bar.set_position(worker.bytes_done.load(Ordering::Relaxed));
            let matches = worker.matches.load(Ordering::Relaxed);
            let status  = *worker.status.lock().unwrap();
            let label   = match status {
                WorkerStatus::Waiting => "waiting".to_string(),
                WorkerStatus::Running => format!("{matches} matches"),
                WorkerStatus::Done    => format!("done — {matches} matches"),
                WorkerStatus::Failed  => "FAILED".to_string(),
            };
            bar.set_message(format!("[{}] {label}", worker.section_name));
            if status == WorkerStatus::Done {
                bar.finish();
            }
        }
        drop(workers);

        if state.is_complete() {
            break;
        }

        std::thread::sleep(Duration::from_millis(POLL_MS));
    }

    // Finish all bars cleanly so indicatif restores the terminal cursor
    if state.is_cancelled() {
        if state.remote {
            net_bar.abandon_with_message("Network transfer  [cancelled]");
        }
        for bar in &worker_bars {
            bar.abandon_with_message("cancelled");
        }
        println!("\nCancelled.");
        return;
    }

    if state.remote {
        net_bar.finish_with_message(format!("{}  [done]", state.transfer_label));
    }

    // Summary
    let results = state.results.lock().unwrap();
    println!("\nTotal matches: {}", results.len());
    for result in results.iter().take(20) {
        println!("  [{:<10}] {:<12} @ {:>10}  line {:>7}  {}", result.section, result.label, result.offset, result.line, result.value);
    }
    if results.len() > 20 {
        println!("  ... and {} more", results.len() - 20);
    }
}
