#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod aec;
mod audio;
mod branding;
mod dsp;
mod i18n;
mod preset;
mod ui;

use std::sync::{Arc, atomic::Ordering};

use audio::{
    AudioEngine, Controls, DeviceInfo, Stats, list_input_devices, list_output_devices, store_f32,
};
use eframe::egui;
use i18n::Language;
use preset::{AecPreset, PresetKind};

#[derive(Debug)]
enum AppStatus {
    Ready,
    NoInput,
    NoOutput,
    Failed(String),
}

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
    status: AppStatus,
    language: Option<Language>,
    show_help: bool,
}

impl PaApp {
    fn new(storage: Option<&dyn eframe::Storage>) -> Self {
        let preset = PresetKind::default();
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
            status: AppStatus::Ready,
            language: Language::load(storage),
            show_help: true,
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
            self.status = AppStatus::Ready;
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
            self.status = AppStatus::NoInput;
            return;
        };
        let Some(output) = self.outputs.get(self.selected_output) else {
            self.status = AppStatus::NoOutput;
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
                self.status = AppStatus::Ready;
                self.engine = Some(engine);
            }
            Err(err) => {
                self.controls.enabled.store(false, Ordering::Relaxed);
                self.status = AppStatus::Failed(err);
            }
        }
    }

    fn stop(&mut self) {
        self.controls.enabled.store(false, Ordering::Relaxed);
        self.engine = None;
        self.status = AppStatus::Ready;
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Wireless PA")
            .with_app_id("wireless-pa")
            .with_icon(branding::window_icon())
            .with_inner_size([960.0, 920.0])
            .with_min_inner_size([600.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Wireless PA",
        options,
        Box::new(|cc| {
            ui::configure(&cc.egui_ctx);
            Ok(Box::new(PaApp::new(cc.storage)))
        }),
    )
}
