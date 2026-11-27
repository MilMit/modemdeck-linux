# ModemDeck runtime and portable packaging

ModemDeck v1.0 RC1.3 removes the two optional command-line runtime helpers used by earlier builds:

- Update checks no longer shell out to `curl`; HTTPS is handled inside the Rust binary.
- Desktop notifications no longer shell out to `notify-send`; they use the freedesktop notification D-Bus service directly.

The v2 interface uses Slint. GTK4 is not a build or runtime dependency. `fontconfig`, `freetype`, `libxkbcommon`, and `libudev` are native runtime dependencies of the Linux build; the package manager resolves these from the distribution.

## End-user packages

### Debian/Ubuntu `.deb` (recommended)

The end user does **not** install build tools or libraries manually. Installing the `.deb` with Ubuntu App Center or `apt install ./modemdeck_*.deb` resolves the runtime packages automatically.

Runtime OS services/packages:

- ModemManager
- NetworkManager
- D-Bus and desktop notification services provided by the host
- Polkit authorization (`pkexec`) for privileged operations when available

These are host services. They are deliberately not duplicated inside ModemDeck because doing so would create competing modem/network managers.

The end user does **not** need Cargo, rustc, pkg-config, GTK development headers, curl or notify-send.

### AppImage

The AppImage builder is architecture-specific and downloads linuxdeploy at build time. An AppImage is distributable only after a successful build and runtime validation on that architecture. The host still needs ModemManager, NetworkManager, D-Bus, a graphical session, and Polkit for privileged operations.

## Developer/build machine

Build dependencies are intentionally separate from end-user dependencies. `scripts/install-deps-ubuntu.sh` is only for people compiling ModemDeck from source.
