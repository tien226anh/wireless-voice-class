# Regenerate the guide screenshots

The capture script launches the compiled Linux app with fresh preferences,
chooses English, starts audio, opens the advanced panels, switches profiles and
languages, and stops. It also reopens the app to check saved language selection.
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
its app, sound server, and virtual display when it exits. Language preferences
are stored in its temporary data directory; your saved language stays unchanged.

## Output and verification

| File | Captured state |
| --- | --- |
| `00-language.png` | First-launch bilingual language chooser |
| `01-setup.png` | English setup, Bluetooth profile selected by default |
| `02-presets.png` | Sound profiles with a translated explanation tooltip |
| `03-running.png` | Audio streams started, Stop button visible |
| `04-feedback.png` | Feedback parameters expanded |
| `05-bluetooth-buffer.png` | Bluetooth preset applied while running; adaptive settings expanded |
| `06-stopped.png` | Audio stopped after the walkthrough |
| `07-vietnamese.png` | Vietnamese interface and help while audio keeps running |
| `08-small-window.png` | Vietnamese layout at the minimum window size |
| `09-start-help.png` | Start button with its getting-started tooltip |
| `10-language-restored.png` | Reopened app remembers Vietnamese and skips onboarding |

The script checks that Start creates audio capture and playback streams, that
changing the preset restarts capture, that switching languages preserves the
running streams, and that Stop removes capture and playback, including at the
minimum window size. It checks persisted
language preferences and starts audio again after relaunch to confirm onboarding
was skipped. Inspect all
screenshots after running: clicks use window-relative coordinates for the current
layout at 100% scale, so layout changes may require updating those coordinates.

These are Linux UI and stream-lifecycle checks with silent virtual devices. They
do not test Windows UI, microphone sound quality, Bluetooth transport, echo
cancellation quality, or real-room feedback performance. The sample-rate labels
and buffer values can vary slightly between machines and runs.

Tool references: [FFmpeg X11 capture](https://ffmpeg.org/ffmpeg-devices.html#x11grab)
and [ALSA PCM configuration](https://www.alsa-project.org/alsa-doc/alsa-lib/pcm_plugins.html).
