# ModemDeck v2.0.0

This is the first stable ModemDeck 2 release, following validation of the Rust + Slint migration.

## Included in this release

- Rust backend and Slint desktop interface, with GTK4 removed from the build and runtime dependency set.
- Updated desktop packaging metadata and a versioned AppStream entry.
- Rust Semantic Versioning for update comparisons and stable-channel updates.
- Fallback to the public GitHub Releases API when the hosted update manifest cannot be read; the app opens release information and does not replace package-manager installs.
- Removed unused direct Rust dependencies.
- Existing modem, band-control, diagnostics, localization, tray, and GNOME integration code carried into the candidate. Availability remains dependent on modem firmware, system services, and desktop environment.

## Validation

The source quality gates and package checks are recorded with the release build. Hardware-sensitive behavior still depends on modem firmware, permissions, and the host desktop session.

## Installation

Use only package artifacts attached to this reviewed release. For managed installs, use the matching distribution package manager.
