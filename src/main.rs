#![allow(dead_code)]
mod analyzer;
mod antenna;
mod backup;
mod cells;
mod cli;
mod core;
mod diagnostics;
mod doctor;
mod drivers;
mod esim;
mod failover;
mod fcc;
mod firmware;
mod gps;
mod hardware;
mod hotspot;
mod i18n;
mod locking;
mod model;
mod modem;
mod netdiag;
mod netmode;
mod notifications;
mod operators;
mod optimization;
mod platform;
mod remote;
mod rf_lab;
mod scanner;
mod scheduler;
mod services;
mod storage;
mod terminal;
mod tray;
mod ui;
mod usage;

#[cfg(test)]
mod tests;

fn main() {
    core::i18n_init_helper();
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("ModemDeck {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Some(code) = modem::helper::maybe_run(&args) {
        std::process::exit(code);
    }
    if let Some(code) = cli::maybe_run_cli(&args) {
        std::process::exit(code);
    }
    if args.get(1).map(String::as_str) == Some("--rollback-guard") {
        let seconds = args
            .get(2)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(47);
        let token = args.get(3).cloned().unwrap_or_default();
        std::thread::sleep(std::time::Duration::from_secs(seconds));
        let still_ours = core::safety::read_pending()
            .map(|pending| !token.is_empty() && pending.guard_token == token)
            .unwrap_or(false);
        if !still_ours {
            std::process::exit(0);
        }
        let code = match modem::ModemManagerClient::new() {
            Ok(client) => match client.recover_pending_band_change() {
                Ok(_) => 0,
                Err(_) => 1,
            },
            Err(_) => 1,
        };
        std::process::exit(code);
    }
    if args.iter().any(|arg| arg == "--monitor") {
        std::process::exit(services::monitor::run());
    }

    let start_minimized = args.iter().any(|a| a == "--minimized" || a == "--tray");
    let code = ui::slint_app::run_gui(start_minimized);
    std::process::exit(code);
}
