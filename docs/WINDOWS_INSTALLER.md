# Windows installer

The Windows release job builds both the portable ZIP and
`wireless-pa-vX.Y.Z-windows-x64-setup.exe` from the same `package/` directory.
Both are published alongside the Linux archive and `SHA256SUMS` on the same
version's Releases page. The installer is required for publication.

The setup wizard supports Windows 10/11 x64, installs into the current user's
`%LOCALAPPDATA%\Programs\Wireless PA`, displays the app logo and license, creates
a Start menu shortcut, and offers an optional desktop shortcut. It includes
the executable, guides, licenses, logo assets, and source-version record.
App language selection still happens on first launch. A stable Inno Setup
application ID keeps upgrades in the same installation. Uninstall removes
installed files and shortcuts while retaining the language preference under
`%APPDATA%\wireless-pa\data`.

## Build locally on Windows

Install Rust's MSVC toolchain, the Windows SDK, and Inno Setup 6.3 or newer.
The `windows-2022` GitHub runner already includes Inno Setup. Stage the app with
the same commands as **Package Windows release** in the workflow, including
`VERSION.txt`. The executable's ProductVersion must match the requested tag.

```powershell
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --locked --release --target x86_64-pc-windows-msvc
# After staging package/ with the matching release version:
./scripts/build-windows-installer.ps1 -Version v0.5.2
```

Use the release workflow to stamp the exact version before compilation. The
build helper rejects mismatched executable/package versions and missing files,
then runs `ISCC.exe` with the [installer definition](https://github.com/tien226anh/wireless-voice-class/blob/main/packaging/windows/wireless-pa.iss).
The MSVC runtime is linked statically to avoid a separate VC++ prerequisite.

## Validation before publication

On a clean disposable Windows account without Wireless PA installed:

```powershell
./scripts/test-windows-installer.ps1 -Version v0.5.2
```

The test installs silently into a temporary path containing spaces, compares
every installed payload file with the staged package by SHA-256, verifies the
version and Start menu shortcut, and checks that the desktop shortcut is opt-in.
It reinstalls with the desktop shortcut enabled, then uninstalls and checks
removal of the app, registration, and shortcuts. A marker in the preference
directory checks that reinstall and uninstall preserve user data. Installer
logs are printed on failure. The test does not launch the app or access audio.

References: [Inno Setup application IDs](https://jrsoftware.org/ishelp/topic_setup_appid.htm),
[per-user installation](https://jrsoftware.org/ishelp/topic_setup_privilegesrequired.htm),
[setup command-line options](https://jrsoftware.org/ishelp/topic_setupcmdline.htm),
and [Rust CRT linkage](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes).
