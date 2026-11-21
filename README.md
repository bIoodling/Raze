# Raze Macro

Windows development build of Raze Macro with a permanent black and purple interface.
The original 1–1400 CPS/KPS engine timing loop is preserved.

## Development activation

This branch uses **offline development activation only**: `RAZE-DEV-2026`.
It is a public test key, not production licensing. There are no startup licensing
requests, HTTP dependencies, hardware identifiers or licensing background threads.
Activation and settings live in `%LOCALAPPDATA%\RazeMacro\Development`.
The previous network licensing implementation is retained in an inactive source file
and the unchanged source-import commit. A build without the `dev-activation`
feature rejects the development key; production licensing is not implemented.

## Use

Install `Raze-Macro-Setup.exe` from the Windows Actions artifact. Rust and Cargo
are not required on the customer's machine. Open settings with the compact
dashboard's ellipsis, or press Enter while the dashboard has focus.
Assign Hold Key and Toggle UI; KPS also requires Spam Key.
Select CPS/KPS, enter a rate from 1 to 1400, then choose Toggle or Hold.
The dashboard displays configured rate while active, not a measured throughput.
Bindings start unassigned and never use the left or right mouse button.
Letters, digits, numpad operators, F1–F11 and middle/side mouse buttons are allowed.
Windows/modifier/system keys and combinations are rejected. Escape cancels capture;
Delete or Backspace clears it. Duplicate identities are rejected even when scan
metadata differs. Holding a modifier or pressing a reserved key stops macro output.
Raw Input observes input without installing hooks or suppressing Windows shortcuts.

Use Tab / Shift+Tab to navigate settings, Enter or Space to select, and Escape to
return to the dashboard. The rate field supports native selection, copy/paste,
Delete, Home/End and Ctrl+A. A valid rate is committed on Enter or focus loss.
Closing transforms the same window to a confirmation; Cancel reverses it.

## Build on isolated Windows

Install Rust stable MSVC and the Windows SDK / C++ build tools in an isolated VM.
From the repository root:

```powershell
cargo fmt --manifest-path RazeMacro/Cargo.toml -- --check
cargo clippy --manifest-path RazeMacro/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path RazeMacro/Cargo.toml --locked
cargo build --manifest-path RazeMacro/Cargo.toml --locked --release
python scripts/make_assets.py
& "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" installer/Raze.iss
```

Then embed the core setup in the compact native installer front end:

```powershell
$env:RAZE_INNER_SETUP = (Resolve-Path dist/Raze-Core-Setup.exe).Path
cargo build --manifest-path RazeMacro/Cargo.toml --locked --release --features installer --bin Raze-Macro-Setup
Copy-Item RazeMacro/target/release/Raze-Macro-Setup.exe dist/Raze-Macro-Setup.exe
```

Output: `dist/Raze-Macro-Setup.exe`. Inno Setup 6 is needed to package, not to run.
CI uses GitHub-hosted `windows-2022` runners only. It never targets a personal PC.
The installer is per-user, creates Start Menu shortcuts, offers a desktop shortcut,
includes an uninstaller and offers launch after installation.

See [testing and limitations](docs/TESTING.md) and [changelog](CHANGELOG.md).

## Branding and font rights

The angular R is reconstructed as a native vector from the supplied reference.
Consolas is selected from the Windows installation and is not redistributed.
No external font files are bundled. The reference board's explanatory labels
and product-family panel are documentation, not extra application screens.
Source is publicly visible for development; no additional source license is granted
by this README.
