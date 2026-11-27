# Slint UI, Design System & System Tray Guide

## Architecture
ModemDeck v2.0 uses **Slint 1.19.1** (`winit` backend with `femtovg` OpenGL hardware rendering and automatic `software` renderer fallback) with zero GTK4/GLib/GIO runtime dependencies.

## Centralized Design System (`ui/`)
- `ui/tokens.slint` — Spacing, corner radii, font sizes, window dimensions, `AppThemeMode`, `UiComplexityMode`
- `ui/theme.slint` — Global `Theme` reactive properties (`dark`, `light`, `high-contrast`, `custom-accent`, `reduced-motion`, `is-rtl`)
- `ui/components.slint` — Reusable `SectionCard`, `MetricTile`, `StatusBadge`, `ActionButton`, `NavItem`, `SignalBarGauge`, `LiveSvgChart`
- `ui/main.slint` — Collapsible sidebar + 13 pages bound to `src/ui/slint_app.rs`

## System Tray (Ubuntu / GNOME / KDE / XFCE)
- Linux uses `ksni` (StatusNotifierItem over D-Bus) in `src/tray/mod.rs`.
- On Ubuntu GNOME, ensure `gnome-shell-extension-appindicator` (built into Ubuntu Desktop) is enabled for tray icons in the top bar.
- For richer inline telemetry in GNOME Shell, ModemDeck's GNOME extension (`gnome-extension/modemdeck@milmit.net`) reads `$XDG_RUNTIME_DIR/modemdeck-status.json` via IPC without polling ModemManager directly.
