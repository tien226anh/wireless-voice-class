# Regenerate the guide screenshots

The capture script launches the compiled Linux app, selects a preset, starts
audio, opens the advanced panels, switches to the Bluetooth preset, and stops.
It captures the real application window with FFmpeg. It does not modify the UI,
invent device names, or read your microphone or desktop session.

## Requirements and command

On Ubuntu/Debian, install the capture tools and build dependencies:

```bash
sudo apt-get install python3 xvfb xdotool ffmpeg pulseaudio pulseaudio-utils libasound2-plugins libasound2-dev pkg-config
cargo build --locked --release
python3 scripts/capture-guide.py
```

Run as a normal user with permission to start local graphical processes. Xvfb
chooses a free display. Software rendering needs Mesa's runtime drivers (install
`libgl1-mesa-dri` if your machine does not already have them).

To capture a different binary or keep the committed screenshots untouched:

```bash
python3 scripts/capture-guide.py --binary target/release/wireless-pa --output /tmp/wireless-pa-images
```

The script requires only Python's standard library plus the commands above. It
creates a private PulseAudio server with a silent virtual microphone and a null
speaker. A process-local ALSA configuration routes both app selectors (`default`)
to that server. It leaves the system audio configuration unchanged and terminates
its app, sound server, and virtual display when it exits.

## Output and verification

| File | Captured state |
| --- | --- |
| `01-setup.png` | Stopped, default preset and device selectors |
| `02-presets.png` | Preset choices open |
| `03-running.png` | Audio streams started, Stop button visible |
| `04-feedback.png` | Feedback parameters expanded |
| `05-bluetooth-buffer.png` | Bluetooth preset applied while running; adaptive settings expanded |
| `06-stopped.png` | Audio stopped after the walkthrough |

The script checks that Start creates audio capture and playback streams, that
changing the preset restarts capture, and that Stop removes capture. Inspect all
screenshots after running: clicks use window-relative coordinates for the current
layout at 100% scale, so layout changes may require updating those coordinates.

These are Linux UI and stream-lifecycle checks with silent virtual devices. They
do not test Windows UI, microphone sound quality, Bluetooth transport, echo
cancellation quality, or real-room feedback performance. The sample-rate labels
and buffer values can vary slightly between machines and runs.

Tool references: [FFmpeg X11 capture](https://ffmpeg.org/ffmpeg-devices.html#x11grab)
and [ALSA PCM configuration](https://www.alsa-project.org/alsa-doc/alsa-lib/pcm_plugins.html).
