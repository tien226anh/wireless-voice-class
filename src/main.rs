#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aec;
mod audio;
mod dsp;
mod preset;

use std::sync::{Arc, atomic::Ordering};

use audio::{
    AudioEngine, Controls, DeviceInfo, Stats, list_input_devices, list_output_devices, load_f32,
    store_f32,
};
use eframe::egui;
use preset::{AecPreset, PresetKind};

struct PaApp {
    inputs: Vec<DeviceInfo>,
    outputs: Vec<DeviceInfo>,
    selected_input: usize,
    selected_output: usize,
    preset: PresetKind,
    gain: f32,
    gate: f32,
    limit: f32,
    high_pass_hz: f32,
    compressor_threshold_db: f32,
    compressor_ratio: f32,
    compressor_attack_ms: f32,
    compressor_release_ms: f32,
    feedback_enabled: bool,
    feedback_min_hz: f32,
    feedback_max_hz: f32,
    feedback_tonal_ratio: f32,
    feedback_required_hits: u32,
    feedback_notch_q: f32,
    feedback_release_seconds: f32,
    adaptive_target_buffer_ms: f32,
    adaptive_correction_strength: f32,
    adaptive_max_correction: f32,
    aec_enabled: bool,
    aec_preset: AecPreset,
    controls: Arc<Controls>,
    stats: Arc<Stats>,
    engine: Option<AudioEngine>,
    status: String,
}

impl PaApp {
    fn new() -> Self {
        let preset = PresetKind::SmallClassroom;
        let p = preset.config();
        let mut app = Self {
            inputs: Vec::new(),
            outputs: Vec::new(),
            selected_input: 0,
            selected_output: 0,
            preset,
            gain: p.gain,
            gate: p.gate,
            limit: p.limiter,
            high_pass_hz: p.high_pass_hz,
            compressor_threshold_db: p.compressor_threshold_db,
            compressor_ratio: p.compressor_ratio,
            compressor_attack_ms: p.compressor_attack_ms,
            compressor_release_ms: p.compressor_release_ms,
            feedback_enabled: p.feedback_enabled,
            feedback_min_hz: p.feedback.min_hz,
            feedback_max_hz: p.feedback.max_hz,
            feedback_tonal_ratio: p.feedback.tonal_ratio,
            feedback_required_hits: p.feedback.required_hits as u32,
            feedback_notch_q: p.feedback.notch_q,
            feedback_release_seconds: p.feedback.release_seconds,
            adaptive_target_buffer_ms: p.adaptive.target_buffer_ms,
            adaptive_correction_strength: p.adaptive.correction_strength,
            adaptive_max_correction: p.adaptive.max_correction,
            aec_enabled: p.aec_enabled,
            aec_preset: p.aec,
            controls: Arc::new(Controls::default()),
            stats: Arc::new(Stats::default()),
            engine: None,
            status: "Stopped".to_owned(),
        };
        app.sync_controls();
        app.refresh_devices();
        app
    }

    fn apply_preset(&mut self, kind: PresetKind) {
        let was_running = self.engine.is_some();
        if was_running {
            self.stop();
        }

        self.preset = kind;
        let p = kind.config();
        self.gain = p.gain;
        self.gate = p.gate;
        self.limit = p.limiter;
        self.high_pass_hz = p.high_pass_hz;
        self.compressor_threshold_db = p.compressor_threshold_db;
        self.compressor_ratio = p.compressor_ratio;
        self.compressor_attack_ms = p.compressor_attack_ms;
        self.compressor_release_ms = p.compressor_release_ms;
        self.feedback_enabled = p.feedback_enabled;
        self.feedback_min_hz = p.feedback.min_hz;
        self.feedback_max_hz = p.feedback.max_hz;
        self.feedback_tonal_ratio = p.feedback.tonal_ratio;
        self.feedback_required_hits = p.feedback.required_hits as u32;
        self.feedback_notch_q = p.feedback.notch_q;
        self.feedback_release_seconds = p.feedback.release_seconds;
        self.adaptive_target_buffer_ms = p.adaptive.target_buffer_ms;
        self.adaptive_correction_strength = p.adaptive.correction_strength;
        self.adaptive_max_correction = p.adaptive.max_correction;
        self.aec_enabled = p.aec_enabled;
        self.aec_preset = p.aec;
        self.sync_controls();

        if was_running {
            self.start();
        } else {
            self.status = format!("Preset loaded: {}", kind.label());
        }
    }

    fn sync_controls(&self) {
        store_f32(&self.controls.gain, self.gain);
        store_f32(&self.controls.gate, self.gate);
        store_f32(&self.controls.limit, self.limit);
        store_f32(&self.controls.high_pass_hz, self.high_pass_hz);
        store_f32(
            &self.controls.compressor_threshold_db,
            self.compressor_threshold_db,
        );
        store_f32(&self.controls.compressor_ratio, self.compressor_ratio);
        store_f32(
            &self.controls.compressor_attack_ms,
            self.compressor_attack_ms,
        );
        store_f32(
            &self.controls.compressor_release_ms,
            self.compressor_release_ms,
        );
        self.controls
            .feedback_enabled
            .store(self.feedback_enabled, Ordering::Relaxed);
        store_f32(&self.controls.feedback_min_hz, self.feedback_min_hz);
        store_f32(&self.controls.feedback_max_hz, self.feedback_max_hz);
        store_f32(
            &self.controls.feedback_tonal_ratio,
            self.feedback_tonal_ratio,
        );
        self.controls
            .feedback_required_hits
            .store(self.feedback_required_hits, Ordering::Relaxed);
        store_f32(&self.controls.feedback_notch_q, self.feedback_notch_q);
        store_f32(
            &self.controls.feedback_release_seconds,
            self.feedback_release_seconds,
        );
        store_f32(
            &self.controls.adaptive_target_buffer_ms,
            self.adaptive_target_buffer_ms,
        );
        store_f32(
            &self.controls.adaptive_correction_strength,
            self.adaptive_correction_strength,
        );
        store_f32(
            &self.controls.adaptive_max_correction,
            self.adaptive_max_correction,
        );
        self.controls
            .aec_enabled
            .store(self.aec_enabled, Ordering::Relaxed);
    }

    fn refresh_devices(&mut self) {
        let old_input = self.inputs.get(self.selected_input).map(|x| x.name.clone());
        let old_output = self
            .outputs
            .get(self.selected_output)
            .map(|x| x.name.clone());
        self.inputs = list_input_devices();
        self.outputs = list_output_devices();
        self.selected_input = old_input
            .and_then(|name| self.inputs.iter().position(|x| x.name == name))
            .unwrap_or(0)
            .min(self.inputs.len().saturating_sub(1));
        self.selected_output = old_output
            .and_then(|name| self.outputs.iter().position(|x| x.name == name))
            .unwrap_or(0)
            .min(self.outputs.len().saturating_sub(1));
    }

    fn start(&mut self) {
        self.stop();
        let Some(input) = self.inputs.get(self.selected_input) else {
            self.status = "No input device available".to_owned();
            return;
        };
        let Some(output) = self.outputs.get(self.selected_output) else {
            self.status = "No output device available".to_owned();
            return;
        };

        self.sync_controls();
        self.controls.enabled.store(true, Ordering::Relaxed);
        self.stats.input_overruns.store(0, Ordering::Relaxed);
        self.stats.output_underruns.store(0, Ordering::Relaxed);

        match AudioEngine::start(
            input.index,
            output.index,
            Arc::clone(&self.controls),
            Arc::clone(&self.stats),
            self.aec_preset,
        ) {
            Ok(engine) => {
                self.status = format!(
                    "Running · {} · {} Hz/{}ch → {} Hz/{}ch",
                    self.preset.label(),
                    engine.input_rate,
                    engine.input_channels,
                    engine.output_rate,
                    engine.output_channels
                );
                self.engine = Some(engine);
            }
            Err(err) => self.status = format!("Failed: {err}"),
        }
    }

    fn stop(&mut self) {
        self.controls.enabled.store(false, Ordering::Relaxed);
        self.engine = None;
        self.status = "Stopped".to_owned();
    }
}

impl eframe::App for PaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.sync_controls();
        let peak = load_f32(&self.stats.peak_bits);
        self.stats
            .peak_bits
            .store((peak * 0.90).to_bits(), Ordering::Relaxed);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Wireless PA");
            ui.label("Wireless headset microphone → computer speaker");
            ui.add_space(10.0);

            let mut chosen = self.preset;
            egui::ComboBox::from_label("Room / microphone preset")
                .selected_text(self.preset.label())
                .show_ui(ui, |ui| {
                    for p in PresetKind::ALL {
                        ui.selectable_value(&mut chosen, p, p.label());
                    }
                });
            if chosen != self.preset {
                self.apply_preset(chosen);
            }
            ui.small(self.preset.description());

            ui.add_enabled_ui(self.engine.is_none(), |ui| {
                egui::ComboBox::from_label("Microphone")
                    .selected_text(
                        self.inputs
                            .get(self.selected_input)
                            .map(|x| x.name.as_str())
                            .unwrap_or("No microphone"),
                    )
                    .show_ui(ui, |ui| {
                        for (idx, item) in self.inputs.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_input, idx, &item.name);
                        }
                    });
                egui::ComboBox::from_label("Speaker")
                    .selected_text(
                        self.outputs
                            .get(self.selected_output)
                            .map(|x| x.name.as_str())
                            .unwrap_or("No speaker"),
                    )
                    .show_ui(ui, |ui| {
                        for (idx, item) in self.outputs.iter().enumerate() {
                            ui.selectable_value(&mut self.selected_output, idx, &item.name);
                        }
                    });
                if ui.button("Refresh devices").clicked() {
                    self.refresh_devices();
                }
            });

            ui.separator();
            ui.heading("Voice processing");
            ui.add(egui::Slider::new(&mut self.high_pass_hz, 40.0..=220.0).text("High-pass (Hz)"));
            ui.add(egui::Slider::new(&mut self.gate, 0.0..=0.05).text("Noise gate"));
            ui.add(
                egui::Slider::new(&mut self.compressor_threshold_db, -40.0..=-3.0)
                    .text("Compressor threshold (dB)"),
            );
            ui.add(
                egui::Slider::new(&mut self.compressor_ratio, 1.0..=10.0).text("Compressor ratio"),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::Slider::new(&mut self.compressor_attack_ms, 1.0..=80.0).text("Attack ms"),
                );
                ui.add(
                    egui::Slider::new(&mut self.compressor_release_ms, 30.0..=600.0)
                        .text("Release ms"),
                );
            });
            ui.add(egui::Slider::new(&mut self.gain, 0.0..=4.0).text("Mic gain"));
            ui.add(egui::Slider::new(&mut self.limit, 0.20..=1.0).text("Limiter"));

            ui.separator();
            ui.checkbox(&mut self.aec_enabled, "Acoustic echo cancellation (AEC)");
            ui.small(format!(
                "AEC preset: echo {} ms · search {} ms · tail {} ms{}",
                self.aec_preset.max_echo_delay_ms,
                self.aec_preset.max_search_delay_ms,
                self.aec_preset.tail_ms,
                if self.engine.is_some() {
                    " (restart via preset change to alter AEC topology)"
                } else {
                    ""
                }
            ));
            let aec_latency = self.stats.aec_latency_ms.load(Ordering::Relaxed);
            ui.label(if self.aec_enabled {
                format!("AEC: active · algorithmic latency ~{aec_latency} ms")
            } else {
                "AEC: bypassed".to_owned()
            });

            ui.separator();
            ui.checkbox(&mut self.feedback_enabled, "Basic feedback suppression");
            ui.collapsing("Feedback parameters", |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.feedback_min_hz)
                            .range(80.0..=3000.0)
                            .suffix(" Hz min"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.feedback_max_hz)
                            .range(500.0..=10000.0)
                            .suffix(" Hz max"),
                    );
                });
                ui.add(
                    egui::Slider::new(&mut self.feedback_tonal_ratio, 4.0..=30.0)
                        .text("Detection tonal ratio"),
                );
                ui.add(
                    egui::Slider::new(&mut self.feedback_required_hits, 1..=8)
                        .text("Persistent scans"),
                );
                ui.add(egui::Slider::new(&mut self.feedback_notch_q, 4.0..=40.0).text("Notch Q"));
                ui.add(
                    egui::Slider::new(&mut self.feedback_release_seconds, 0.5..=10.0)
                        .text("Release seconds"),
                );
            });
            let feedback_hz = load_f32(&self.stats.feedback_hz_bits);
            if self.feedback_enabled && feedback_hz > 0.0 {
                ui.label(format!("Feedback notch active: {feedback_hz:.0} Hz"));
            } else if self.feedback_enabled {
                ui.label("Feedback notch: monitoring");
            }

            ui.separator();
            ui.collapsing("Adaptive resampling", |ui| {
                ui.add(
                    egui::Slider::new(&mut self.adaptive_target_buffer_ms, 10.0..=180.0)
                        .text("Target buffer (ms)"),
                );
                ui.add(
                    egui::Slider::new(&mut self.adaptive_correction_strength, 0.0005..=0.010)
                        .logarithmic(true)
                        .text("Clock correction strength"),
                );
                ui.add(
                    egui::Slider::new(&mut self.adaptive_max_correction, 0.003..=0.050)
                        .logarithmic(true)
                        .text("Max rate correction"),
                );
            });

            ui.add_space(8.0);
            ui.label("Input level");
            ui.add(egui::ProgressBar::new(peak.clamp(0.0, 1.0)).show_percentage());
            let queue_fill = self.stats.queue_fill_permille.load(Ordering::Relaxed) as f32 / 1000.0;
            ui.label("Audio buffer");
            ui.add(egui::ProgressBar::new(queue_fill.clamp(0.0, 1.0)).show_percentage());

            ui.add_space(10.0);
            if self.engine.is_none() {
                if ui.button("▶ Start microphone").clicked() {
                    self.start();
                }
            } else if ui.button("■ Stop").clicked() {
                self.stop();
            }

            ui.add_space(8.0);
            ui.label(&self.status);
            ui.small(format!(
                "Underruns: {} · Overruns: {} · Platform: {}",
                self.stats.output_underruns.load(Ordering::Relaxed),
                self.stats.input_overruns.load(Ordering::Relaxed),
                std::env::consts::OS
            ));
        });

        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Wireless PA")
            .with_inner_size([720.0, 900.0])
            .with_min_inner_size([560.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Wireless PA",
        options,
        Box::new(|_| Ok(Box::new(PaApp::new()))),
    )
}
