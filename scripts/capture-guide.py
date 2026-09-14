#!/usr/bin/env python3
"""Capture the actual app using a private X display and virtual audio devices."""

import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/wireless-pa")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/images")
    parser.add_argument("--first-screen-only", action="store_true", help="Capture just the language chooser")
    parser.add_argument("--setup-only", action="store_true", help="Capture onboarding and English setup")
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f"Missing {binary}; run cargo build --locked --release first")
    for tool in ("Xvfb", "xdotool", "ffmpeg", "pulseaudio", "pactl"):
        if not shutil.which(tool):
            parser.error(f"Missing tool: {tool}; see docs/CAPTURE.md")
    args.output.mkdir(parents=True, exist_ok=True)
    children = []
    with tempfile.TemporaryDirectory(prefix="wireless-pa-guide-") as temporary:
        temp = Path(temporary)
        env = dict(os.environ)
        env.update({
            "XDG_RUNTIME_DIR": temporary,
            "XDG_DATA_HOME": str(temp / "data"),
            "XDG_CONFIG_HOME": str(temp / "config"),
            "PULSE_STATE_PATH": str(temp / "pulse-state"),
            "PULSE_SERVER": f"unix:{temp}/pulse.sock",
            "ALSA_CONFIG_PATH": str(temp / "alsa.conf"),
            "LIBGL_ALWAYS_SOFTWARE": "1",
            "WINIT_UNIX_BACKEND": "x11",
            "WINIT_X11_SCALE_FACTOR": "1",
        })
        env.pop("WAYLAND_DISPLAY", None)

        def run(*command, **kwargs):
            return subprocess.run(command, env=env, check=True, text=True,
                                  capture_output=True, timeout=20, **kwargs).stdout.strip()

        def launch(command, name, **kwargs):
            with (temp / f"{name}.log").open("w") as log:
                process = subprocess.Popen(command, env=env, stdout=log, stderr=log, **kwargs)
            children.append(process)
            return process

        def wait_for(probe, description):
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                if any(p.poll() is not None for p in children):
                    raise RuntimeError(f"A process exited while waiting for {description}")
                try:
                    result = probe()
                    if result:
                        return result
                except subprocess.CalledProcessError:
                    pass
                time.sleep(0.2)
            raise TimeoutError(f"Timed out waiting for {description}")

        try:
            display_file = temp / "display"
            with display_file.open("w") as descriptor:
                launch(["Xvfb", "-displayfd", str(descriptor.fileno()), "-screen", "0",
                        "1000x1200x24", "-nolisten", "tcp"], "display",
                       pass_fds=(descriptor.fileno(),))
            display = wait_for(lambda: display_file.read_text().strip(), "virtual display")
            env["DISPLAY"] = f":{display}"
            (temp / "pulse.pa").write_text(
                f"load-module module-native-protocol-unix socket={temp}/pulse.sock\n"
                "load-module module-null-sink sink_name=guide_input rate=48000 channels=1\n"
                "load-module module-null-sink sink_name=guide_output rate=48000 channels=2\n"
                "set-default-source guide_input.monitor\n"
                "set-default-sink guide_output\n"
            )
            launch(["pulseaudio", "-n", "--daemonize=no", "--use-pid-file=no",
                    "--exit-idle-time=-1", "--file", str(temp / "pulse.pa")], "audio")
            wait_for(lambda: run("pactl", "info"), "virtual audio server")
            (temp / "alsa.conf").write_text(
                'pcm.default {\n'
                '  type pulse\n'
                f'  server "unix:{temp}/pulse.sock"\n'
                '}\n'
            )
            app = launch([str(binary)], "app")
            window = wait_for(
                lambda: run("xdotool", "search", "--onlyvisible", "--pid", str(app.pid),
                            "--name", "^Wireless PA$").splitlines()[0], "app window")
            run("xdotool", "windowsize", "--sync", window, "960", "1000")

            def capture(name, hover=False):
                if not hover:
                    run("xdotool", "mousemove", "--window", window, "5", "5")
                time.sleep(1)
                run("ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "x11grab",
                    "-draw_mouse", "0", "-framerate", "1", "-window_id", window,
                    "-i", env["DISPLAY"], "-frames:v", "1", "-update", "1",
                    str(args.output / f"{name}.png"))
                print(f"Captured {name}.png", flush=True)

            capture("00-language")
            if args.first_screen_only:
                return

            def click(x, y):
                run("xdotool", "mousemove", "--window", window, str(x), str(y), "click", "1")
                time.sleep(0.4)

            def streams():
                return tuple(tuple(line.split()[0] for line in run("pactl", "list", "short", kind).splitlines())
                             for kind in ("source-outputs", "sink-inputs"))

            def assert_running():
                state = streams()
                if not all(state):
                    raise RuntimeError("Expected both microphone capture and speaker playback")
                return state

            def saved_language(code):
                preferences = temp / "data/wireless-pa/app.ron"
                return preferences.is_file() and re.search(
                    rf'"wireless-pa.language"\s*:\s*"{code}"', preferences.read_text())

            click(350, 580)
            wait_for(lambda: saved_language("en"), "English preference to save")
            capture("01-setup")
            if args.setup_only:
                return

            run("xdotool", "mousemove", "--window", window, "180", "556")
            capture("02-presets", hover=True)
            run("xdotool", "mousemove", "--window", window, "800", "955")
            capture("09-start-help", hover=True)
            click(800, 955)
            wait_for(lambda: all(streams()), "audio capture and playback")
            capture("03-running")

            # A live language change must preserve the same capture/playback streams.
            before_language = assert_running()
            click(866, 44)
            click(855, 115)
            wait_for(lambda: saved_language("vi"), "Vietnamese preference to save")
            if assert_running() != before_language:
                raise RuntimeError("Changing language interrupted the audio streams")
            capture("07-vietnamese")
            click(866, 44)
            click(855, 81)
            wait_for(lambda: saved_language("en"), "English preference to save again")

            # Changing a profile while running must reopen both streams.
            before_profile = assert_running()
            click(480, 579)
            wait_for(lambda: all(streams()) and all(new != old for new, old in zip(streams(), before_profile)),
                     "profile change to reopen audio")
            click(180, 579)
            assert_running()
            click(745, 44)  # Hide quick help to make room for advanced controls.
            click(140, 791)  # Advanced sound settings.
            run("xdotool", "mousemove", "--window", window, "900", "800",
                "click", "--repeat", "12", "--delay", "50", "5")
            click(145, 743)  # Feedback fine-tuning.
            run("xdotool", "mousemove", "--window", window, "900", "800",
                "click", "--repeat", "24", "--delay", "30", "5")
            capture("04-feedback")
            click(180, 787)  # Wireless stability & delay stays near the bottom.
            run("xdotool", "mousemove", "--window", window, "900", "800",
                "click", "--repeat", "24", "--delay", "30", "5")
            capture("05-bluetooth-buffer")
            click(800, 955)
            wait_for(lambda: not any(streams()), "capture and playback to stop")
            capture("06-stopped")

            click(866, 44)
            click(855, 115)
            wait_for(lambda: saved_language("vi"), "final Vietnamese preference")
            run("xdotool", "windowsize", "--sync", window, "600", "620")
            run("xdotool", "mousemove", "--window", window, "560", "300",
                "click", "--repeat", "30", "--delay", "30", "4")
            capture("08-small-window")
            click(450, 575)
            wait_for(lambda: all(streams()), "playback in the small window")
            click(450, 575)
            wait_for(lambda: not any(streams()), "small-window playback stop")

            # Restart with the same isolated preferences. Audio should start directly
            # from the main screen, proving the first-launch chooser was skipped.
            app.terminate()
            app.wait(timeout=5)
            children.remove(app)
            app = launch([str(binary)], "app-reopened")
            window = wait_for(
                lambda: run("xdotool", "search", "--onlyvisible", "--pid", str(app.pid),
                            "--name", "^Wireless PA$").splitlines()[0], "reopened app window")
            run("xdotool", "windowsize", "--sync", window, "960", "1000")
            capture("10-language-restored")
            click(800, 955)
            wait_for(lambda: all(streams()), "playback after restoring language")
            click(800, 955)
            wait_for(lambda: not any(streams()), "final playback stop")
            print("Verified language persistence, live language switching, profile restart, and Start/Stop.", flush=True)
        except Exception:
            for log in temp.glob("*.log"):
                print(f"--- {log.name} ---\n{log.read_text()[-4000:]}")
            raise
        finally:
            for process in reversed(children):
                if process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


if __name__ == "__main__":
    main()
