#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetKind {
    SmallClassroom,
    LargeRoom,
    BluetoothMic,
}

impl PresetKind {
    pub const ALL: [Self; 3] = [
        Self::SmallClassroom,
        Self::LargeRoom,
        Self::BluetoothMic,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::SmallClassroom => "Small classroom",
            Self::LargeRoom => "Large room",
            Self::BluetoothMic => "Bluetooth microphone",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::SmallClassroom => "Low latency and natural speech for a nearby laptop/portable speaker.",
            Self::LargeRoom => "More dynamics control, stronger feedback rejection, and a slightly deeper safety buffer.",
            Self::BluetoothMic => "Extra buffering and AEC search range for wireless transport latency and clock drift.",
        }
    }

    pub const fn config(self) -> PresetConfig {
        match self {
            Self::SmallClassroom => PresetConfig {
                high_pass_hz: 100.0,
                gate: 0.008,
                compressor_threshold_db: -18.0,
                compressor_ratio: 3.0,
                compressor_attack_ms: 8.0,
                compressor_release_ms: 120.0,
                gain: 1.50,
                limiter: 0.92,
                aec_enabled: true,
                aec: AecPreset {
                    max_echo_delay_ms: 350,
                    max_search_delay_ms: 1_500,
                    tail_ms: 220,
                },
                feedback_enabled: true,
                feedback: FeedbackPreset {
                    min_hz: 300.0,
                    max_hz: 5_000.0,
                    tonal_ratio: 18.0,
                    required_hits: 3,
                    notch_q: 18.0,
                    release_seconds: 3.0,
                },
                adaptive: AdaptiveResamplingPreset {
                    target_buffer_ms: 45.0,
                    correction_strength: 0.003,
                    max_correction: 0.015,
                },
            },
            Self::LargeRoom => PresetConfig {
                high_pass_hz: 120.0,
                gate: 0.010,
                compressor_threshold_db: -20.0,
                compressor_ratio: 4.0,
                compressor_attack_ms: 5.0,
                compressor_release_ms: 180.0,
                gain: 1.65,
                limiter: 0.90,
                aec_enabled: true,
                aec: AecPreset {
                    max_echo_delay_ms: 500,
                    max_search_delay_ms: 1_800,
                    tail_ms: 300,
                },
                feedback_enabled: true,
                feedback: FeedbackPreset {
                    min_hz: 250.0,
                    max_hz: 6_000.0,
                    tonal_ratio: 14.0,
                    required_hits: 2,
                    notch_q: 22.0,
                    release_seconds: 5.0,
                },
                adaptive: AdaptiveResamplingPreset {
                    target_buffer_ms: 65.0,
                    correction_strength: 0.004,
                    max_correction: 0.020,
                },
            },
            Self::BluetoothMic => PresetConfig {
                high_pass_hz: 90.0,
                gate: 0.007,
                compressor_threshold_db: -16.0,
                compressor_ratio: 2.5,
                compressor_attack_ms: 12.0,
                compressor_release_ms: 160.0,
                gain: 1.55,
                limiter: 0.92,
                aec_enabled: true,
                aec: AecPreset {
                    max_echo_delay_ms: 450,
                    max_search_delay_ms: 2_000,
                    tail_ms: 260,
                },
                feedback_enabled: true,
                feedback: FeedbackPreset {
                    min_hz: 300.0,
                    max_hz: 5_000.0,
                    tonal_ratio: 16.0,
                    required_hits: 3,
                    notch_q: 16.0,
                    release_seconds: 4.0,
                },
                adaptive: AdaptiveResamplingPreset {
                    target_buffer_ms: 90.0,
                    correction_strength: 0.005,
                    max_correction: 0.025,
                },
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PresetConfig {
    pub high_pass_hz: f32,
    pub gate: f32,
    pub compressor_threshold_db: f32,
    pub compressor_ratio: f32,
    pub compressor_attack_ms: f32,
    pub compressor_release_ms: f32,
    pub gain: f32,
    pub limiter: f32,
    pub aec_enabled: bool,
    pub aec: AecPreset,
    pub feedback_enabled: bool,
    pub feedback: FeedbackPreset,
    pub adaptive: AdaptiveResamplingPreset,
}

#[derive(Clone, Copy, Debug)]
pub struct AecPreset {
    pub max_echo_delay_ms: u16,
    pub max_search_delay_ms: u16,
    pub tail_ms: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct FeedbackPreset {
    pub min_hz: f32,
    pub max_hz: f32,
    pub tonal_ratio: f32,
    pub required_hits: u8,
    pub notch_q: f32,
    pub release_seconds: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct AdaptiveResamplingPreset {
    pub target_buffer_ms: f32,
    pub correction_strength: f32,
    pub max_correction: f32,
}
