use std::collections::VecDeque;

use crossbeam_queue::ArrayQueue;
use decibri_aec::{Aec, AecConfig};

use crate::preset::AecPreset;

const CAPTURE_BLOCK: usize = 256;
const MAX_REFERENCE_DRAIN: usize = 4096;

/// Re-blocks CPAL capture audio for the block-oriented AEC implementation.
pub struct AcousticEchoCanceller {
    engine: Aec,
    capture_block: Vec<f32>,
    cancelled: VecDeque<f32>,
    process_out: Vec<f32>,
    reference_scratch: Vec<f32>,
    enabled_last: bool,
    algorithmic_latency_samples: usize,
}

impl AcousticEchoCanceller {
    pub fn new(sample_rate: u32, preset: AecPreset) -> Result<Self, String> {
        if !(8_000..=48_000).contains(&sample_rate) {
            return Err(format!(
                "AEC supports 8-48 kHz capture rates, but this microphone uses {sample_rate} Hz"
            ));
        }

        let mut config = AecConfig::default();
        config.sample_rate = sample_rate;
        config.max_echo_delay_ms = preset.max_echo_delay_ms;
        config.max_search_delay_ms = preset.max_search_delay_ms;
        config.tail_ms = preset.tail_ms;

        let engine = Aec::new(config).map_err(|e| e.to_string())?;
        let algorithmic_latency_samples = engine.latency_samples();

        Ok(Self {
            engine,
            capture_block: Vec::with_capacity(CAPTURE_BLOCK),
            cancelled: VecDeque::with_capacity(CAPTURE_BLOCK * 4),
            process_out: Vec::with_capacity(CAPTURE_BLOCK * 2),
            reference_scratch: Vec::with_capacity(MAX_REFERENCE_DRAIN),
            enabled_last: false,
            algorithmic_latency_samples,
        })
    }

    pub fn latency_samples(&self) -> usize {
        self.algorithmic_latency_samples
    }

    pub fn reset(&mut self) {
        self.engine.reset();
        self.capture_block.clear();
        self.cancelled.clear();
        self.process_out.clear();
        self.reference_scratch.clear();
    }

    pub fn process_sample(
        &mut self,
        near: f32,
        reference_queue: &ArrayQueue<f32>,
        enabled: bool,
    ) -> f32 {
        if !enabled {
            if self.enabled_last {
                self.reset();
            }
            self.enabled_last = false;
            while reference_queue.pop().is_some() {}
            return near;
        }

        if !self.enabled_last {
            self.reset();
            self.enabled_last = true;
        }

        self.capture_block.push(near);

        if self.capture_block.len() >= CAPTURE_BLOCK {
            self.feed_pending_reference(reference_queue);
            self.process_out.clear();

            if self.engine.process(&self.capture_block, &mut self.process_out).is_ok() {
                self.cancelled.extend(self.process_out.iter().copied());
            } else {
                self.engine.reset();
                self.cancelled.clear();
            }
            self.capture_block.clear();
        }

        self.cancelled.pop_front().unwrap_or(0.0)
    }

    fn feed_pending_reference(&mut self, reference_queue: &ArrayQueue<f32>) {
        self.reference_scratch.clear();
        while self.reference_scratch.len() < MAX_REFERENCE_DRAIN {
            match reference_queue.pop() {
                Some(v) => self.reference_scratch.push(v),
                None => break,
            }
        }

        if !self.reference_scratch.is_empty() {
            self.engine.feed_reference(&self.reference_scratch);
        }
    }
}

/// Resamples the exact mono render signal from the output callback back to the
/// capture/AEC sample rate.
pub struct ReferenceResampler {
    input_rate: f64,
    output_rate: f64,
    phase: f64,
    previous: f32,
    initialized: bool,
}

impl ReferenceResampler {
    pub fn new(speaker_rate: u32, aec_rate: u32) -> Self {
        Self {
            input_rate: speaker_rate as f64,
            output_rate: aec_rate as f64,
            phase: 0.0,
            previous: 0.0,
            initialized: false,
        }
    }

    pub fn push(&mut self, current: f32, queue: &ArrayQueue<f32>) {
        if !self.initialized {
            self.previous = current;
            self.initialized = true;
        }

        self.phase += self.output_rate;
        while self.phase >= self.input_rate {
            self.phase -= self.input_rate;
            let t = 1.0 - (self.phase / self.output_rate.max(1.0));
            let y = self.previous + (current - self.previous) * t.clamp(0.0, 1.0) as f32;

            if queue.push(y).is_err() {
                let _ = queue.pop();
                let _ = queue.push(y);
            }
        }

        self.previous = current;
    }
}
