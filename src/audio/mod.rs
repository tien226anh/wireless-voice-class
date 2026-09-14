use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};

use cpal::{
    Device, SampleFormat, Stream, StreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use crossbeam_queue::ArrayQueue;

use crate::{
    aec::{AcousticEchoCanceller, ReferenceResampler},
    dsp::{DspParams, InputDsp},
    preset::AecPreset,
};

pub fn atomic_f32(v: f32) -> AtomicU32 {
    AtomicU32::new(v.to_bits())
}

pub fn load_f32(v: &AtomicU32) -> f32 {
    f32::from_bits(v.load(Ordering::Relaxed))
}

pub fn store_f32(dst: &AtomicU32, v: f32) {
    dst.store(v.to_bits(), Ordering::Relaxed);
}

#[derive(Default)]
pub struct Stats {
    pub input_overruns: AtomicU64,
    pub output_underruns: AtomicU64,
    pub peak_bits: AtomicU32,
    pub queue_fill_permille: AtomicU32,
    pub feedback_hz_bits: AtomicU32,
    pub aec_latency_ms: AtomicU32,
    // pub aec_reference_overruns: AtomicU64,
}

pub struct Controls {
    pub gain: AtomicU32,
    pub gate: AtomicU32,
    pub limit: AtomicU32,
    pub high_pass_hz: AtomicU32,
    pub compressor_threshold_db: AtomicU32,
    pub compressor_ratio: AtomicU32,
    pub compressor_attack_ms: AtomicU32,
    pub compressor_release_ms: AtomicU32,
    pub feedback_enabled: AtomicBool,
    pub feedback_min_hz: AtomicU32,
    pub feedback_max_hz: AtomicU32,
    pub feedback_tonal_ratio: AtomicU32,
    pub feedback_required_hits: AtomicU32,
    pub feedback_notch_q: AtomicU32,
    pub feedback_release_seconds: AtomicU32,
    pub adaptive_target_buffer_ms: AtomicU32,
    pub adaptive_correction_strength: AtomicU32,
    pub adaptive_max_correction: AtomicU32,
    pub aec_enabled: AtomicBool,
    pub enabled: AtomicBool,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            gain: atomic_f32(1.5),
            gate: atomic_f32(0.008),
            limit: atomic_f32(0.92),
            high_pass_hz: atomic_f32(100.0),
            compressor_threshold_db: atomic_f32(-18.0),
            compressor_ratio: atomic_f32(3.0),
            compressor_attack_ms: atomic_f32(8.0),
            compressor_release_ms: atomic_f32(120.0),
            feedback_enabled: AtomicBool::new(true),
            feedback_min_hz: atomic_f32(300.0),
            feedback_max_hz: atomic_f32(5000.0),
            feedback_tonal_ratio: atomic_f32(18.0),
            feedback_required_hits: AtomicU32::new(3),
            feedback_notch_q: atomic_f32(18.0),
            feedback_release_seconds: atomic_f32(3.0),
            adaptive_target_buffer_ms: atomic_f32(45.0),
            adaptive_correction_strength: atomic_f32(0.003),
            adaptive_max_correction: atomic_f32(0.015),
            aec_enabled: AtomicBool::new(true),
            enabled: AtomicBool::new(true),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub name: String,
    pub index: usize,
}

pub fn list_input_devices() -> Vec<DeviceInfo> {
    cpal::default_host()
        .input_devices()
        .map(|it| {
            it.enumerate()
                .filter_map(|(index, d)| d.name().ok().map(|name| DeviceInfo { name, index }))
                .collect()
        })
        .unwrap_or_default()
}

pub fn list_output_devices() -> Vec<DeviceInfo> {
    cpal::default_host()
        .output_devices()
        .map(|it| {
            it.enumerate()
                .filter_map(|(index, d)| d.name().ok().map(|name| DeviceInfo { name, index }))
                .collect()
        })
        .unwrap_or_default()
}

fn nth_input_device(index: usize) -> Result<Device, String> {
    cpal::default_host()
        .input_devices()
        .map_err(|e| e.to_string())?
        .nth(index)
        .ok_or_else(|| "Input device disappeared. Refresh devices and retry.".to_owned())
}

fn nth_output_device(index: usize) -> Result<Device, String> {
    cpal::default_host()
        .output_devices()
        .map_err(|e| e.to_string())?
        .nth(index)
        .ok_or_else(|| "Output device disappeared. Refresh devices and retry.".to_owned())
}

pub struct AudioEngine {
    _input_stream: Stream,
    _output_stream: Stream,
    pub input_rate: u32,
    pub output_rate: u32,
    pub input_channels: u16,
    pub output_channels: u16,
}

impl AudioEngine {
    pub fn start(
        input_index: usize,
        output_index: usize,
        controls: Arc<Controls>,
        stats: Arc<Stats>,
        aec_preset: AecPreset,
    ) -> Result<Self, String> {
        let input = nth_input_device(input_index)?;
        let output = nth_output_device(output_index)?;

        let input_supported = input.default_input_config().map_err(|e| e.to_string())?;
        let output_supported = output.default_output_config().map_err(|e| e.to_string())?;

        let input_rate = input_supported.sample_rate().0;
        let output_rate = output_supported.sample_rate().0;
        let input_channels = input_supported.channels();
        let output_channels = output_supported.channels();

        let input_config: StreamConfig = input_supported.clone().into();
        let output_config: StreamConfig = output_supported.clone().into();

        // ~250 ms max queue capacity, but the adaptive controller targets ~45 ms.
        let capacity = ((input_rate as f32 * 0.25) as usize).max(4096);
        let queue = Arc::new(ArrayQueue::<f32>::new(capacity));

        // Speaker render reference for AEC, resampled by the output callback to the
        // microphone sample rate. Keep enough history for Bluetooth + OS latency.
        let reference_capacity = ((input_rate as f32 * 2.0) as usize).max(8192);
        let reference_queue = Arc::new(ArrayQueue::<f32>::new(reference_capacity));

        // Fail early if AEC cannot operate at this capture rate. Users can still
        // disable AEC in the UI for unusual >48 kHz devices after choosing a supported
        // format in the OS; CPAL default configs are normally <=48 kHz for microphones.
        let aec_probe = AcousticEchoCanceller::new(input_rate, aec_preset)?;
        stats.aec_latency_ms.store(
            ((aec_probe.latency_samples() as f64 / input_rate as f64) * 1000.0).round() as u32,
            Ordering::Relaxed,
        );
        drop(aec_probe);

        let q_in = Arc::clone(&queue);
        let c_in = Arc::clone(&controls);
        let s_in = Arc::clone(&stats);
        let r_in = Arc::clone(&reference_queue);

        let input_stream = match input_supported.sample_format() {
            SampleFormat::F32 => input
                .build_input_stream(
                    &input_config,
                    {
                        let mut dsp = InputDsp::new(input_rate);
                        let mut aec = AcousticEchoCanceller::new(input_rate, aec_preset).expect("validated AEC config");
                        move |data: &[f32], _| push_input_f32(data, input_channels, &q_in, &r_in, &c_in, &s_in, &mut aec, &mut dsp)
                    },
                    |err| eprintln!("input stream error: {err}"),
                    None,
                )
                .map_err(|e| e.to_string())?,
            SampleFormat::I16 => {
                let q = Arc::clone(&queue);
                let c = Arc::clone(&controls);
                let s = Arc::clone(&stats);
                let r = Arc::clone(&reference_queue);
                input
                    .build_input_stream(
                        &input_config,
                        {
                            let mut dsp = InputDsp::new(input_rate);
                            let mut aec = AcousticEchoCanceller::new(input_rate, aec_preset).expect("validated AEC config");
                            move |data: &[i16], _| push_input_i16(data, input_channels, &q, &r, &c, &s, &mut aec, &mut dsp)
                        },
                        |err| eprintln!("input stream error: {err}"),
                        None,
                    )
                    .map_err(|e| e.to_string())?
            }
            SampleFormat::U16 => {
                let q = Arc::clone(&queue);
                let c = Arc::clone(&controls);
                let s = Arc::clone(&stats);
                let r = Arc::clone(&reference_queue);
                input
                    .build_input_stream(
                        &input_config,
                        {
                            let mut dsp = InputDsp::new(input_rate);
                            let mut aec = AcousticEchoCanceller::new(input_rate, aec_preset).expect("validated AEC config");
                            move |data: &[u16], _| push_input_u16(data, input_channels, &q, &r, &c, &s, &mut aec, &mut dsp)
                        },
                        |err| eprintln!("input stream error: {err}"),
                        None,
                    )
                    .map_err(|e| e.to_string())?
            }
            fmt => return Err(format!("Unsupported input sample format: {fmt:?}")),
        };

        let q_out = Arc::clone(&queue);
        let c_out = Arc::clone(&controls);
        let s_out = Arc::clone(&stats);
        let r_out = Arc::clone(&reference_queue);

        let output_stream = match output_supported.sample_format() {
            SampleFormat::F32 => output
                .build_output_stream(
                    &output_config,
                    make_output_f32(q_out, r_out, c_out, s_out, input_rate, output_rate, output_channels),
                    |err| eprintln!("output stream error: {err}"),
                    None,
                )
                .map_err(|e| e.to_string())?,
            SampleFormat::I16 => output
                .build_output_stream(
                    &output_config,
                    make_output_i16(q_out, r_out, c_out, s_out, input_rate, output_rate, output_channels),
                    |err| eprintln!("output stream error: {err}"),
                    None,
                )
                .map_err(|e| e.to_string())?,
            SampleFormat::U16 => output
                .build_output_stream(
                    &output_config,
                    make_output_u16(q_out, r_out, c_out, s_out, input_rate, output_rate, output_channels),
                    |err| eprintln!("output stream error: {err}"),
                    None,
                )
                .map_err(|e| e.to_string())?,
            fmt => return Err(format!("Unsupported output sample format: {fmt:?}")),
        };

        // Start capture first so the output callback has data available.
        input_stream.play().map_err(|e| e.to_string())?;
        output_stream.play().map_err(|e| e.to_string())?;

        Ok(Self {
            _input_stream: input_stream,
            _output_stream: output_stream,
            input_rate,
            output_rate,
            input_channels,
            output_channels,
        })
    }
}

fn dsp_params(controls: &Controls) -> DspParams {
    DspParams {
        high_pass_hz: load_f32(&controls.high_pass_hz),
        gate_threshold: load_f32(&controls.gate),
        compressor_threshold_db: load_f32(&controls.compressor_threshold_db),
        compressor_ratio: load_f32(&controls.compressor_ratio),
        compressor_attack_ms: load_f32(&controls.compressor_attack_ms),
        compressor_release_ms: load_f32(&controls.compressor_release_ms),
        gain: load_f32(&controls.gain),
        limiter: load_f32(&controls.limit),
        feedback_enabled: controls.feedback_enabled.load(Ordering::Relaxed),
        feedback_min_hz: load_f32(&controls.feedback_min_hz),
        feedback_max_hz: load_f32(&controls.feedback_max_hz),
        feedback_tonal_ratio: load_f32(&controls.feedback_tonal_ratio),
        feedback_required_hits: controls.feedback_required_hits.load(Ordering::Relaxed).clamp(1, 255) as u8,
        feedback_notch_q: load_f32(&controls.feedback_notch_q),
        feedback_release_seconds: load_f32(&controls.feedback_release_seconds),
    }
}

fn process_and_push(
    sample: f32,
    queue: &ArrayQueue<f32>,
    reference_queue: &ArrayQueue<f32>,
    controls: &Controls,
    stats: &Stats,
    aec: &mut AcousticEchoCanceller,
    dsp: &mut InputDsp,
) {
    if !controls.enabled.load(Ordering::Relaxed) {
        return;
    }

    let aec_enabled = controls.aec_enabled.load(Ordering::Relaxed);
    let echo_cancelled = aec.process_sample(sample, reference_queue, aec_enabled);
    let x = dsp.process(echo_cancelled, dsp_params(controls));
    stats.feedback_hz_bits.store(dsp.feedback_frequency_hz().to_bits(), Ordering::Relaxed);

    let peak = x.abs();
    let old = load_f32(&stats.peak_bits);
    if peak > old {
        stats.peak_bits.store(peak.to_bits(), Ordering::Relaxed);
    }

    if queue.push(x).is_err() {
        let _ = queue.pop();
        if queue.push(x).is_err() {
            stats.input_overruns.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn push_input_f32(
    data: &[f32],
    channels: u16,
    queue: &ArrayQueue<f32>,
    reference_queue: &ArrayQueue<f32>,
    controls: &Controls,
    stats: &Stats,
    aec: &mut AcousticEchoCanceller,
    dsp: &mut InputDsp,
) {
    for frame in data.chunks(channels as usize) {
        let mono = frame.iter().copied().sum::<f32>() / frame.len() as f32;
        process_and_push(mono, queue, reference_queue, controls, stats, aec, dsp);
    }
}

fn push_input_i16(
    data: &[i16],
    channels: u16,
    queue: &ArrayQueue<f32>,
    reference_queue: &ArrayQueue<f32>,
    controls: &Controls,
    stats: &Stats,
    aec: &mut AcousticEchoCanceller,
    dsp: &mut InputDsp,
) {
    for frame in data.chunks(channels as usize) {
        let mono = frame.iter().map(|&x| x as f32 / i16::MAX as f32).sum::<f32>() / frame.len() as f32;
        process_and_push(mono, queue, reference_queue, controls, stats, aec, dsp);
    }
}

fn push_input_u16(
    data: &[u16],
    channels: u16,
    queue: &ArrayQueue<f32>,
    reference_queue: &ArrayQueue<f32>,
    controls: &Controls,
    stats: &Stats,
    aec: &mut AcousticEchoCanceller,
    dsp: &mut InputDsp,
) {
    for frame in data.chunks(channels as usize) {
        let mono = frame
            .iter()
            .map(|&x| (x as f32 / u16::MAX as f32) * 2.0 - 1.0)
            .sum::<f32>()
            / frame.len() as f32;
        process_and_push(mono, queue, reference_queue, controls, stats, aec, dsp);
    }
}

struct AdaptiveLinearResampler {
    nominal_step: f64,
    step: f64,
    phase: f64,
    a: f32,
    b: f32,
    initialized: bool,
    started: bool,
}

impl AdaptiveLinearResampler {
    fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            nominal_step: input_rate as f64 / output_rate as f64,
            step: input_rate as f64 / output_rate as f64,
            phase: 0.0,
            a: 0.0,
            b: 0.0,
            initialized: false,
            started: false,
        }
    }

    fn adapt(&mut self, queue: &ArrayQueue<f32>, controls: &Controls, target_frames: usize) {
        let target = target_frames.max(1) as f64;
        let error = (queue.len() as f64 - target) / target;
        let strength = load_f32(&controls.adaptive_correction_strength).clamp(0.0001, 0.02) as f64;
        let max_correction = load_f32(&controls.adaptive_max_correction).clamp(0.001, 0.05) as f64;
        let correction = (error * strength).clamp(-max_correction, max_correction);
        self.step = self.nominal_step * (1.0 + correction);
    }

    fn next(&mut self, queue: &ArrayQueue<f32>, stats: &Stats, controls: &Controls, input_rate: u32) -> f32 {
        let fill = if queue.capacity() == 0 {
            0
        } else {
            ((queue.len() as f32 / queue.capacity() as f32) * 1000.0) as u32
        };
        stats.queue_fill_permille.store(fill.min(1000), Ordering::Relaxed);

        let target_ms = load_f32(&controls.adaptive_target_buffer_ms).clamp(10.0, 250.0);
        let target_frames = ((input_rate as f32 * target_ms / 1000.0) as usize).max(128);
        if !self.started {
            if queue.len() < target_frames {
                return 0.0;
            }
            self.started = true;
        }

        self.adapt(queue, controls, target_frames);

        if !self.initialized {
            let Some(a) = queue.pop() else {
                stats.output_underruns.fetch_add(1, Ordering::Relaxed);
                self.started = false;
                return 0.0;
            };
            let b = queue.pop().unwrap_or(a);
            self.a = a;
            self.b = b;
            self.initialized = true;
        }

        let y = self.a + (self.b - self.a) * self.phase as f32;
        self.phase += self.step;

        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.a = self.b;
            match queue.pop() {
                Some(v) => self.b = v,
                None => {
                    self.b = self.a;
                    self.started = false;
                    self.initialized = false;
                    stats.output_underruns.fetch_add(1, Ordering::Relaxed);
                    return 0.0;
                }
            }
        }

        y
    }
}

fn make_output_f32(
    queue: Arc<ArrayQueue<f32>>,
    reference_queue: Arc<ArrayQueue<f32>>,
    controls: Arc<Controls>,
    stats: Arc<Stats>,
    input_rate: u32,
    output_rate: u32,
    channels: u16,
) -> impl FnMut(&mut [f32], &cpal::OutputCallbackInfo) + Send + 'static {
    let mut rs = AdaptiveLinearResampler::new(input_rate, output_rate);
    let mut reference_rs = ReferenceResampler::new(output_rate, input_rate);
    move |data, _| {
        for frame in data.chunks_mut(channels as usize) {
            let x = if controls.enabled.load(Ordering::Relaxed) { rs.next(&queue, &stats, &controls, input_rate) } else { 0.0 };
            reference_rs.push(x, &reference_queue);
            frame.fill(x);
        }
    }
}

fn make_output_i16(
    queue: Arc<ArrayQueue<f32>>,
    reference_queue: Arc<ArrayQueue<f32>>,
    controls: Arc<Controls>,
    stats: Arc<Stats>,
    input_rate: u32,
    output_rate: u32,
    channels: u16,
) -> impl FnMut(&mut [i16], &cpal::OutputCallbackInfo) + Send + 'static {
    let mut rs = AdaptiveLinearResampler::new(input_rate, output_rate);
    let mut reference_rs = ReferenceResampler::new(output_rate, input_rate);
    move |data, _| {
        for frame in data.chunks_mut(channels as usize) {
            let x = if controls.enabled.load(Ordering::Relaxed) { rs.next(&queue, &stats, &controls, input_rate) } else { 0.0 };
            reference_rs.push(x, &reference_queue);
            let v = (x.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            frame.fill(v);
        }
    }
}

fn make_output_u16(
    queue: Arc<ArrayQueue<f32>>,
    reference_queue: Arc<ArrayQueue<f32>>,
    controls: Arc<Controls>,
    stats: Arc<Stats>,
    input_rate: u32,
    output_rate: u32,
    channels: u16,
) -> impl FnMut(&mut [u16], &cpal::OutputCallbackInfo) + Send + 'static {
    let mut rs = AdaptiveLinearResampler::new(input_rate, output_rate);
    let mut reference_rs = ReferenceResampler::new(output_rate, input_rate);
    move |data, _| {
        for frame in data.chunks_mut(channels as usize) {
            let x = if controls.enabled.load(Ordering::Relaxed) { rs.next(&queue, &stats, &controls, input_rate) } else { 0.0 };
            reference_rs.push(x, &reference_queue);
            let v = (((x.clamp(-1.0, 1.0) + 1.0) * 0.5) * u16::MAX as f32) as u16;
            frame.fill(v);
        }
    }
}
