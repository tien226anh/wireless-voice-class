#!/usr/bin/env python3
"""Capture the actual app using a private X display and virtual audio devices."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/wireless-pa")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/images")
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
            run("xdotool", "windowsize", "--sync", window, "840", "700")

            def capture(name):
                time.sleep(1)
                run("ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "x11grab",
                    "-draw_mouse", "0", "-framerate", "1", "-window_id", window,
                    "-i", env["DISPLAY"], "-frames:v", "1", "-update", "1",
                    str(args.output / f"{name}.png"))
                print(f"Captured {name}.png", flush=True)

            capture("01-setup")

            def click(x, y):
                run("xdotool", "mousemove", "--window", window, str(x), str(y), "click", "1")
                time.sleep(0.4)

            click(70, 68)
            capture("02-presets")
            run("xdotool", "key", "--window", window, "Escape")
            click(65, 597)
            time.sleep(3)
            if not run("pactl", "list", "short", "source-outputs"):
                raise RuntimeError("Start did not create an audio capture stream")
            if not run("pactl", "list", "short", "sink-inputs"):
                raise RuntimeError("Start did not create an audio playback stream")
            capture("03-running")
            run("xdotool", "windowsize", "--sync", window, "840", "900")
            click(90, 434)
            capture("04-feedback")
            # Switching presets while running exercises the stream restart path.
            click(70, 68)
            click(85, 135)
            click(90, 587)
            capture("05-bluetooth-buffer")
            if not run("pactl", "list", "short", "source-outputs"):
                raise RuntimeError("Preset change did not restart audio capture")
            click(30, 765)
            wait_for(lambda: not run("pactl", "list", "short", "source-outputs"),
                     "capture stream to stop")
            capture("06-stopped")
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
