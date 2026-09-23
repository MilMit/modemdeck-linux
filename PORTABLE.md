# ModemDeck portable/runtime model

ModemDeck v1.0 RC1.3 removes the two optional command-line runtime helpers used by earlier builds:

- Update checks no longer shell out to `curl`; HTTPS is handled inside the Rust binary.
- Desktop notifications no longer shell out to `notify-send`; they use the freedesktop notification D-Bus service directly.

## End-user packages

### Debian/Ubuntu `.deb` (recommended)

The end user does **not** install build tools or libraries manually. Installing the `.deb` with Ubuntu App Center or `apt install ./modemdeck_*.deb` resolves the runtime packages automatically.

Runtime OS services/packages:

- GTK 4 runtime (`libgtk-4-1`)
- ModemManager
- NetworkManager
- Polkit authorization via `pkexec` (the `pkexec` package pulls in the required polkit daemon)

These are host services. They are deliberately not duplicated inside ModemDeck because doing so would create competing modem/network managers.

The end user does **not** need Cargo, rustc, pkg-config, GTK development headers, curl or notify-send.

### AppImage

The AppImage build bundles ModemDeck and its ordinary shared-library chain so the user can download one file, mark it executable and run it. The host still needs the operating-system modem/network services (ModemManager, NetworkManager, D-Bus and Polkit). On supported Ubuntu desktops these are normally system components; ModemDeck reports a clear error if a required service is unavailable.

## Developer/build machine

Build dependencies are intentionally separate from end-user dependencies. `scripts/install-deps-ubuntu.sh` is only for people compiling ModemDeck from source.
