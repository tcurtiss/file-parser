use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use eframe::egui;

use crate::state::{AppState, WorkerStatus};

pub struct App {
    state: Arc<AppState>,
}

impl App {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("File Parser");
            ui.separator();

            // ── File transfer progress ─────────────────────────────────────
            ui.label(egui::RichText::new("File Transfer").strong());

            let net_done  = self.state.net_bytes_done.load(Ordering::Relaxed);
            let net_total = self.state.net_bytes_total.load(Ordering::Relaxed);
            let net_pct   = self.state.net_progress();

            ui.add(
                egui::ProgressBar::new(net_pct)
                    .text(format!(
                        "{:.1} / {:.1} MB",
                        net_done  as f64 / 1e6,
                        net_total as f64 / 1e6,
                    ))
                    .animate(!self.state.is_complete()),
            );

            ui.add_space(8.0);
            ui.separator();

            // ── Workers ───────────────────────────────────────────────────
            ui.label(egui::RichText::new("Workers").strong());

            let workers = self.state.workers.lock().unwrap();
            if workers.is_empty() {
                ui.label(egui::RichText::new("Waiting for section boundaries…").italics());
            } else {
                for worker in workers.iter() {
                    let status  = *worker.status.lock().unwrap();
                    let matches = worker.matches.load(Ordering::Relaxed);
                    let color   = match status {
                        WorkerStatus::Running => egui::Color32::from_rgb(80, 200, 80),
                        WorkerStatus::Done    => egui::Color32::GRAY,
                        WorkerStatus::Failed  => egui::Color32::RED,
                        WorkerStatus::Waiting => egui::Color32::YELLOW,
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, format!("[{:<10}]", worker.section_name));
                        ui.add(
                            egui::ProgressBar::new(worker.progress())
                                .text(format!("{matches} matches"))
                                .animate(status == WorkerStatus::Running),
                        );
                    });
                }
            }
            drop(workers);

            ui.add_space(8.0);
            ui.separator();

            // ── Results table ─────────────────────────────────────────────
            ui.label(egui::RichText::new("Results").strong());

            let results = self.state.results.lock().unwrap();
            if results.is_empty() {
                ui.label(egui::RichText::new("No results yet…").italics());
            } else {
                ui.label(format!("{} total matches", results.len()));
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        egui::Grid::new("results_grid")
                            .striped(true)
                            .spacing([16.0, 4.0])
                            .show(ui, |ui| {
                                ui.strong("Section");
                                ui.strong("Label");
                                ui.strong("Offset");
                                ui.strong("Value");
                                ui.end_row();

                                for result in results.iter() {
                                    ui.label(&result.section);
                                    ui.label(&result.label);
                                    ui.label(format!("{}", result.offset));
                                    ui.label(&result.value);
                                    ui.end_row();
                                }
                            });
                    });
            }
        });

        // Keep repainting while parsing is in progress
        if !self.state.is_complete() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}
