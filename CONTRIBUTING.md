# Contributing to ModemDeck Linux

Thank you for helping improve ModemDeck Linux by MilMit.

Before opening a pull request, build with `cargo build --release` and avoid adding vendor AT commands unless the exact modem/firmware behavior is documented and capability-gated. Safety-critical modem writes must include a recovery path.

For bug reports, include Ubuntu version, ModemManager version, modem model/firmware and privacy-safe diagnostics. Do not post IMEI, SIM identifiers or phone numbers publicly.
