use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct DspParams {
    pub high_pass_hz: f32,
    pub gate_threshold: f32,
    pub compressor_threshold_db: f32,
    pub compressor_ratio: f32,
    pub compressor_attack_ms: f32,
    pub compressor_release_ms: f32,
    pub gain: f32,
    pub limiter: f32,
    pub feedback_enabled: bool,
    pub feedback_min_hz: f32,
    pub feedback_max_hz: f32,
    pub feedback_tonal_ratio: f32,
    pub feedback_required_hits: u8,
    pub feedback_notch_q: f32,
    pub feedback_release_seconds: f32,
}

pub struct InputDsp {
    high_pass: HighPass,
    compressor: Compressor,
    feedback: FeedbackSuppressor,
}

impl InputDsp {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        Self {
            high_pass: HighPass::new(sr, 100.0),
            compressor: Compressor::new(sr),
            feedback: FeedbackSuppressor::new(sr),
        }
    }

    #[inline]
    pub fn process(&mut self, sample: f32, p: DspParams) -> f32 {
        self.high_pass.set_cutoff(p.high_pass_hz);

        let mut x = self.high_pass.process(sample);
        if x.abs() < p.gate_threshold {
            x = 0.0;
        }

        x = self.compressor.process(
            x,
            p.compressor_threshold_db,
            p.compressor_ratio,
            p.compressor_attack_ms,
            p.compressor_release_ms,
        );

        x *= p.gain;

        if p.feedback_enabled {
            x = self.feedback.process(
                x,
                FeedbackParams {
                    min_hz: p.feedback_min_hz,
                    max_hz: p.feedback_max_hz,
                    tonal_ratio: p.feedback_tonal_ratio,
                    required_hits: p.feedback_required_hits,
                    notch_q: p.feedback_notch_q,
                    release_seconds: p.feedback_release_seconds,
                },
            );
        } else {
            self.feedback.reset_detection();
        }

        soft_limit(x, p.limiter)
    }

    pub fn feedback_frequency_hz(&self) -> f32 {
        self.feedback.active_frequency_hz()
    }
}

#[inline]
fn soft_limit(x: f32, limit: f32) -> f32 {
    let limit = limit.clamp(0.05, 1.0);
    let drive = x / limit;
    limit * drive.tanh()
}

struct HighPass {
    sample_rate: f32,
    cutoff_hz: f32,
    alpha: f32,
    prev_x: f32,
    prev_y: f32,
}

impl HighPass {
    fn new(sample_rate: f32, cutoff_hz: f32) -> Self {
        let mut s = Self {
            sample_rate,
            cutoff_hz: 0.0,
            alpha: 0.0,
            prev_x: 0.0,
            prev_y: 0.0,
        };
        s.set_cutoff(cutoff_hz);
        s
    }

    #[inline]
    fn set_cutoff(&mut self, cutoff_hz: f32) {
        let cutoff_hz = cutoff_hz.clamp(20.0, (self.sample_rate * 0.45).max(20.0));
        if (cutoff_hz - self.cutoff_hz).abs() < 0.1 {
            return;
        }
        self.cutoff_hz = cutoff_hz;
        let rc = 1.0 / (2.0 * PI * cutoff_hz);
        let dt = 1.0 / self.sample_rate;
        self.alpha = rc / (rc + dt);
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.alpha * (self.prev_y + x - self.prev_x);
        self.prev_x = x;
        self.prev_y = y;
        y
    }
}

struct Compressor {
    sample_rate: f32,
    envelope: f32,
}

impl Compressor {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            envelope: 0.0,
        }
    }

    #[inline]
    fn process(
        &mut self,
        x: f32,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
    ) -> f32 {
        let detector = x.abs();
        let attack = time_coeff(self.sample_rate, attack_ms.clamp(0.2, 200.0));
        let release = time_coeff(self.sample_rate, release_ms.clamp(5.0, 2000.0));
        let coeff = if detector > self.envelope { attack } else { release };
        self.envelope = coeff * self.envelope + (1.0 - coeff) * detector;

        let env_db = linear_to_db(self.envelope.max(1.0e-8));
        let threshold_db = threshold_db.clamp(-60.0, 0.0);
        let ratio = ratio.clamp(1.0, 20.0);

        if env_db <= threshold_db || ratio <= 1.001 {
            return x;
        }

        let compressed_db = threshold_db + (env_db - threshold_db) / ratio;
        let gain_reduction_db = compressed_db - env_db;
        x * db_to_linear(gain_reduction_db)
    }
}

#[inline]
fn time_coeff(sample_rate: f32, ms: f32) -> f32 {
    (-1.0 / (0.001 * ms * sample_rate)).exp()
}

#[inline]
fn linear_to_db(x: f32) -> f32 {
    20.0 * x.log10()
}

#[inline]
fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

struct BiquadNotch {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
    frequency_hz: f32,
}

impl BiquadNotch {
    fn bypass() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
            frequency_hz: 0.0,
        }
    }

    fn set_notch(&mut self, sample_rate: f32, frequency_hz: f32, q: f32) {
        let f = frequency_hz.clamp(80.0, sample_rate * 0.45);
        let w0 = 2.0 * PI * f / sample_rate;
        let alpha = w0.sin() / (2.0 * q.max(0.5));
        let cos_w0 = w0.cos();
        let a0 = 1.0 + alpha;

        self.b0 = 1.0 / a0;
        self.b1 = -2.0 * cos_w0 / a0;
        self.b2 = 1.0 / a0;
        self.a1 = -2.0 * cos_w0 / a0;
        self.a2 = (1.0 - alpha) / a0;
        self.frequency_hz = f;
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

const DETECT_N: usize = 1024;
const DETECT_HOP: usize = 512;

#[derive(Clone, Copy)]
struct FeedbackParams {
    min_hz: f32,
    max_hz: f32,
    tonal_ratio: f32,
    required_hits: u8,
    notch_q: f32,
    release_seconds: f32,
}

struct FeedbackSuppressor {
    sample_rate: f32,
    detector: Vec<f32>,
    write_pos: usize,
    samples_since_scan: usize,
    candidate_hz: f32,
    candidate_hits: u8,
    misses: u16,
    notch: BiquadNotch,
}

impl FeedbackSuppressor {
    fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            detector: vec![0.0; DETECT_N],
            write_pos: 0,
            samples_since_scan: 0,
            candidate_hz: 0.0,
            candidate_hits: 0,
            misses: 0,
            notch: BiquadNotch::bypass(),
        }
    }

    #[inline]
    fn process(&mut self, x: f32, p: FeedbackParams) -> f32 {
        self.detector[self.write_pos] = x;
        self.write_pos = (self.write_pos + 1) % DETECT_N;
        self.samples_since_scan += 1;

        if self.samples_since_scan >= DETECT_HOP {
            self.samples_since_scan = 0;
            self.scan(p);
        }

        self.notch.process(x)
    }

    fn reset_detection(&mut self) {
        self.candidate_hz = 0.0;
        self.candidate_hits = 0;
        self.misses = 0;
        self.notch = BiquadNotch::bypass();
    }

    fn active_frequency_hz(&self) -> f32 {
        self.notch.frequency_hz
    }

    fn scan(&mut self, p: FeedbackParams) {
        let mut rms_acc = 0.0_f64;
        for &s in &self.detector {
            rms_acc += (s as f64) * (s as f64);
        }
        let rms = (rms_acc / DETECT_N as f64).sqrt() as f32;
        if rms < 0.012 {
            self.on_no_feedback(p.release_seconds);
            return;
        }

        let nyquist = self.sample_rate * 0.5;
        let min_hz = p.min_hz.clamp(80.0, nyquist - 200.0);
        let max_hz = p.max_hz.clamp(min_hz + 100.0, nyquist - 80.0);
        if max_hz <= min_hz {
            return;
        }

        let min_bin = ((min_hz * DETECT_N as f32 / self.sample_rate).ceil() as usize).max(1);
        let max_bin = ((max_hz * DETECT_N as f32 / self.sample_rate).floor() as usize)
            .min(DETECT_N / 2 - 1);

        let mut peak_power = 0.0_f64;
        let mut peak_bin = 0usize;
        let mut power_sum = 0.0_f64;
        let mut bins = 0usize;

        for k in min_bin..=max_bin {
            let omega = 2.0 * std::f64::consts::PI * k as f64 / DETECT_N as f64;
            let coeff = 2.0 * omega.cos();
            let mut s1 = 0.0_f64;
            let mut s2 = 0.0_f64;

            for n in 0..DETECT_N {
                let idx = (self.write_pos + n) % DETECT_N;
                let sample = self.detector[idx] as f64;
                let s0 = sample + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }

            let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
            power_sum += power;
            bins += 1;
            if power > peak_power {
                peak_power = power;
                peak_bin = k;
            }
        }

        if bins < 4 || peak_bin == 0 {
            return;
        }

        let mean_power = (power_sum - peak_power) / bins.saturating_sub(1).max(1) as f64;
        let tonal_ratio = peak_power / mean_power.max(1.0e-12);
        let peak_hz = peak_bin as f32 * self.sample_rate / DETECT_N as f32;

        if tonal_ratio > p.tonal_ratio.max(2.0) as f64 {
            if (peak_hz - self.candidate_hz).abs() <= 120.0 {
                self.candidate_hits = self.candidate_hits.saturating_add(1);
            } else {
                self.candidate_hz = peak_hz;
                self.candidate_hits = 1;
            }
            self.misses = 0;

            if self.candidate_hits >= p.required_hits.max(1) {
                self.notch.set_notch(self.sample_rate, self.candidate_hz, p.notch_q);
            }
        } else {
            self.on_no_feedback(p.release_seconds);
        }
    }

    fn on_no_feedback(&mut self, release_seconds: f32) {
        self.candidate_hits = 0;
        self.misses = self.misses.saturating_add(1);
        let scans_per_second = self.sample_rate / DETECT_HOP as f32;
        let release_scans = (scans_per_second * release_seconds.clamp(0.5, 20.0)).max(1.0) as u16;
        if self.misses >= release_scans {
            self.candidate_hz = 0.0;
            self.notch = BiquadNotch::bypass();
            self.misses = 0;
        }
    }
}
