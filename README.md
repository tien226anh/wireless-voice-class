# Wireless PA (Rust, cross-platform)

<img src="assets/logo.svg" width="88" height="88" alt="Wireless PA logo">

The logo is embedded in the app and Windows executable. See the
[logo and Linux launcher guide](docs/BRANDING.md) for icon assets and desktop setup.

A small cross-platform PA application that routes a microphone to a selected speaker with realtime voice DSP.

Windows users can download the **`windows-x64-setup.exe` installer** from
[Releases](https://github.com/tien226anh/wireless-voice-class/releases).
Portable Windows ZIP and Linux archives are available on the same release page.

Read the [configuration and usage guide](docs/USER_GUIDE.md) for setup instructions,
screenshots, explanations of the controls, and troubleshooting.
You can also read the [Vietnamese guide](docs/USER_GUIDE_VI.md).

On first launch, choose English or Tiếng Việt. The app remembers that choice and
lets you switch languages in the top bar. Bluetooth microphone is the default
profile. The main screen has three setup steps, translated tooltips, and a Start/Stop
bar that stays visible while advanced sound settings scroll separately.

## Stack

- Rust
- CPAL for native audio I/O
- egui/eframe for desktop UI
- crossbeam `ArrayQueue` for realtime queues
- `decibri-aec` for acoustic echo cancellation

## Signal path

```text
Microphone
  -> AEC
  -> high-pass
  -> noise gate
  -> compressor
  -> gain
  -> feedback suppressor
  -> limiter
  -> lock-free queue
  -> adaptive resampler / clock-drift compensation
  -> speaker
       -> AEC far-end reference
```

## Presets

### Small classroom

Designed for a laptop or portable speaker relatively close to the teacher.

- HPF: 100 Hz
- Compressor: -18 dBFS, 3:1, 8 ms attack, 120 ms release
- AEC: 350 ms fine delay, 1500 ms global search, 220 ms tail
- Feedback: tonal ratio 18, 3 persistent scans, Q 18, 3 s release
- Adaptive buffer: 45 ms, correction strength 0.003, max correction 1.5%

### Large room

More conservative stability settings for louder speakers and longer acoustic decay.

- HPF: 120 Hz
- Compressor: -20 dBFS, 4:1, 5 ms attack, 180 ms release
- AEC: 500 ms fine delay, 1800 ms global search, 300 ms tail
- Feedback: tonal ratio 14, 2 persistent scans, Q 22, 5 s release
- Adaptive buffer: 65 ms, correction strength 0.004, max correction 2.0%

### Bluetooth microphone

Trades some latency for stability with wireless transport and independently clocked devices.

- HPF: 90 Hz
- Compressor: -16 dBFS, 2.5:1, 12 ms attack, 160 ms release
- AEC: 450 ms fine delay, 2000 ms global search, 260 ms tail
- Feedback: tonal ratio 16, 3 persistent scans, Q 16, 4 s release
- Adaptive buffer: 90 ms, correction strength 0.005, max correction 2.5%

Preset changes update realtime controls immediately. If audio is already running, selecting another preset restarts the CPAL streams so the block-constructed AEC engine also receives the new topology parameters.

## Build

`decibri-aec 0.2` requires Rust 1.88 or newer.

```bash
rustup update stable
cargo run --release
```

### Linux dependencies

Ubuntu/Debian:

```bash
sudo apt install build-essential pkg-config libasound2-dev
```

### Release build

```bash
cargo build --release
```

Windows output:

```text
target/release/wireless-pa.exe
```

Linux output:

```text
target/release/wireless-pa
```

### Versioned GitHub releases

Create a PR with an exact label such as `release:v0.5.1`, approve its final revision,
and merge into `main`. GitHub Actions builds Linux x64 and Windows x64, then
publishes both archives and checksums under tag `v0.5.1` on the
[Releases page](https://github.com/tien226anh/wireless-voice-class/releases).
Opening or updating a PR starts no builds.

Use the helpers to get a suggested version when creating a PR or manually
releasing from `main`:

```bash
node scripts/release.mjs pr --title "Describe your change" --body-file /tmp/pr.md
node scripts/release.mjs manual
```

The manual command suggests a tag and asks for confirmation before starting a
real build and release. It works for the repository owner without a PR review.
The Actions page also provides separate **suggest** and **release** operations.

Each archive includes the executable, illustrated user guide, MIT license, and
version/source information. See the [complete release workflow](docs/RELEASES.md)
for version selection, manual runs, approvals, and retry behavior.
