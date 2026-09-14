# Wireless PA (Rust, cross-platform)

A small cross-platform PA application that routes a microphone to a selected speaker with realtime voice DSP.

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

### Automated GitHub releases

After a pull request is approved and merged into `main`, GitHub Actions builds
Linux x64 and Windows x64 (MSVC) archives and attaches them, plus `SHA256SUMS`,
to the [Releases page](https://github.com/tien226anh/wireless-voice-class/releases).
Each archive includes the executable, README, and MIT license. Linux builds use
Ubuntu 22.04 and require a compatible desktop Linux system with ALSA installed.

The workflow checks each push to `main` for its merged PR. It requires an approval
on the final PR revision before merging, with no outstanding change requests.
Direct pushes and merges without that approval skip the release. This controls
publication; it does not configure GitHub branch protection or merge PRs for you.

Tags combine the Cargo version and PR number, for example `v0.5.0-pr.12`, so every
approved merge can release without changing `Cargo.toml`. Both builds must succeed
before publication. A failed upload leaves a draft; rerunning the workflow retries
publication, while an already published release is left intact.

The workflow uses GitHub's built-in token; no additional release secret is needed.
You can also use **Run workflow** on the Actions page to verify both builds without
publishing a release.
