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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // ── Bottom button bar — must be declared before central content ────
        egui::Panel::bottom("controls")
            .show_separator_line(true)
            .show_inside(ui, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let complete = self.state.is_complete();
                    let (label, tooltip) = if complete {
                        ("Dismiss", "Close this window")
                    } else {
                        ("Cancel", "Cancel parsing and exit")
                    };
                    if ui.button(label).on_hover_text(tooltip).clicked() {
                        if !complete {
                            self.state.cancel();
                        }
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.add_space(6.0);
            });

        ui.heading("File Parser");
        ui.separator();

        // ── File transfer progress ─────────────────────────────────────
        ui.label(egui::RichText::new(self.state.transfer_label).strong());

        if self.state.remote {
            let net_done  = self.state.net_bytes_done.load(Ordering::Relaxed);
            let net_total = self.state.net_bytes_total.load(Ordering::Relaxed);
            let animating = !self.state.is_complete();

            match self.state.net_progress() {
                Some(pct) => {
                    // Known size — show percentage and bytes
                    ui.add(
                        egui::ProgressBar::new(pct)
                            .text(format!(
                                "{:.1} / {:.1} MB",
                                net_done  as f64 / 1e6,
                                net_total as f64 / 1e6,
                            ))
                            .animate(animating),
                    );
                }
                None => {
                    // Unknown size (e.g. URL without Content-Length) — indeterminate
                    ui.add(
                        egui::ProgressBar::new(0.0)
                            .text(format!("{:.1} MB downloaded", net_done as f64 / 1e6))
                            .animate(animating),
                    );
                }
            }
        } else {
            ui.add(
                egui::ProgressBar::new(1.0)
                    .text("local file, skipped")
                    .fill(egui::Color32::from_rgb(100, 100, 100)),
            );
        }

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
                            ui.strong("Line");
                            ui.strong("Value");
                            ui.end_row();

                            for result in results.iter() {
                                ui.label(&result.section);
                                ui.label(&result.label);
                                ui.label(format!("{}", result.offset));
                                ui.label(format!("{}", result.line));
                                ui.label(&result.value);
                                ui.end_row();
                            }
                        });
                });
        }

        // Keep repainting while parsing is in progress
        if !self.state.is_complete() {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
    }
}
