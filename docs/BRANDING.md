# Application logo and icons

The editable source is [assets/logo.svg](../assets/logo.svg): a white microphone
with mint wireless waves on an indigo tile. It uses vector paths, with no external
fonts or images. The logo uses the repository's MIT license.

![Wireless PA logo](../assets/logo.svg)

The same artwork appears in the welcome screen, the app header, and the native
window icon. Windows builds also embed a multi-resolution ICO in the executable,
so Explorer and shortcuts use the logo. File/product version metadata follows
the Cargo version, including the version stamped by the release workflow.

Both release archives contain `assets/logo.svg` and `assets/icons/`.
The Linux archive additionally includes `wireless-pa.desktop` for desktop menus.
Linux desktop integration uses this launcher and its installed icon; an ELF
executable does not provide an Explorer-style file icon.

## Regenerate the PNG and ICO

On Ubuntu/Debian:

```bash
sudo apt-get install python3-gi python3-gi-cairo gir1.2-rsvg-2.0 python3-pil
/usr/bin/python3 scripts/generate-icons.py
```

The script renders the SVG into a 512px RGBA PNG and an ICO containing 16, 24,
32, 48, 64, 128, and 256px images. Commit both generated files when changing the
SVG. Ordinary Cargo builds use these committed assets and do not need the icon
generation tools.

`src/branding.rs` embeds the PNG in the application. `build.rs` embeds the ICO
when compiling for Windows using [winresource](https://docs.rs/winresource/0.1.31/winresource/).
Native Windows builds require the Windows SDK resource compiler, included on
the Windows release runner. GNU cross-builds use MinGW's resource compiler.

## Linux application-menu entry

From an extracted Linux release, install the executable, icon, and launcher:

```bash
install -Dm755 wireless-pa "$HOME/.local/bin/wireless-pa"
install -Dm644 assets/logo.svg "$HOME/.local/share/icons/hicolor/scalable/apps/wireless-pa.svg"
install -Dm644 wireless-pa.desktop "$HOME/.local/share/applications/wireless-pa.desktop"
```

Ensure `$HOME/.local/bin` is in your desktop session's `PATH`; sign out and back
in after changing it. The launcher then appears as **Wireless PA** in the app
menu. Opening the executable directly still uses the embedded window icon.
