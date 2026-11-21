# Validation and remaining checks

Only disposable GitHub-hosted Windows runners are authorized. Do not execute these
tools, builds, UI input automation or installers on the user's personal PC.

Automated CI gates:
- Original engine timing function compared with the supplied ZIP, ignoring formatting.
- Rust formatting, Clippy with warnings denied, model and UI geometry unit tests.
- Default development feature and development-disabled activation tests.
- Release compilation with the committed dependency lockfile.
- Sixteen offscreen GDI renders (four screens at 96/120/144/192 DPI).
- Silent installer and uninstaller, payload and both shortcut locations.
- SHA-256 of the setup executable and downloadable evidence.

Offscreen renders exercise the shared renderer and substitute text for native EDIT
children. They are **not interactive screenshots** and do not prove native edit
focus, dragging, display compositor cadence or keyboard event ordering.

Interactive acceptance still requires an isolated Windows VM:
- Compare all screens to the reference, including the native rate and key editors.
- Move across mixed-DPI monitors and test native dragging without jitter.
- Check Tab/Shift+Tab, Enter, Escape, Delete, paste, focus loss and capture cancellation.
- Exercise CPS/KPS 1/500/950/1400 against an isolated counting target; measure actual
  timing and CPU use. Unit tests and a build do not verify input throughput.
- Check modifier combinations, duplicate bindings, release while focus changes,
  hidden UI recovery, sleep/resume and clean exit during active output.
- Check transitions, close/cancel reversal and presentation on a 165+ Hz display.
  Animations use elapsed time and a 6 ms requested timer; Windows scheduling and
  compositor limits mean a 165 Hz cadence is not guaranteed.
- Check interactive install, optional desktop shortcut and launch-after-install.

Known design differences: Inno Setup provides the installer wizard's native controls
for installation/uninstallation reliability; its black/purple styling and R branding
do not exactly reproduce the tiny three-stage mockup. The R is a vector reconstruction,
not an original supplied vector asset. No pixel-perfect or jitter-free certification
has been performed. Development activation is not suitable for a production release.
