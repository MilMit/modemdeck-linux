# ModemDeck v2.0.0-beta.1

This is a beta candidate for the Rust + Slint migration. It is not a stable release. Modem hardware, desktop-session, and packaging coverage is not yet complete.

## Included in this candidate

- Rust backend and Slint desktop interface, with GTK4 removed from the build and runtime dependency set.
- Updated desktop packaging metadata and a versioned AppStream entry.
- Rust Semantic Versioning for update comparisons, so stable installs are not offered prereleases.
- Fallback to the public GitHub Releases API when the hosted update manifest cannot be read; the app opens release information and does not replace package-manager installs.
- Removed unused direct Rust dependencies.
- Existing modem, band-control, diagnostics, localization, tray, and GNOME integration code carried into the candidate. Availability remains dependent on modem firmware, system services, and desktop environment.

## Validation

The source quality gates and package checks are recorded with the release build. Hardware-sensitive band changes, crash recovery on physical devices, Wayland/X11 behavior across desktops, and all language layouts require additional independent testing.

## Installation

Use only package artifacts attached to a reviewed release. For managed installs, use the matching distribution package manager. Do not use this prerelease as a stable update channel.
