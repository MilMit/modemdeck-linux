mod apn;
mod communications;
mod diagnostics;
mod drivers;
mod helper;
mod history;
mod i18n;
mod model;
mod modem_manager;
mod monitor;
mod power;
mod profiles;
mod runtime_status;
mod safety;
mod settings;
mod traffic;
mod ui;
mod updates;

use gtk::prelude::*;

fn main() -> glib::ExitCode {
    i18n::init(&settings::Settings::load().language);
    let args: Vec<String> = std::env::args().collect();
    if let Some(code) = helper::maybe_run(&args) { std::process::exit(code); }
    if args.get(1).map(String::as_str) == Some("--rollback-guard") {
        let seconds = args.get(2).and_then(|value| value.parse::<u64>().ok()).unwrap_or(47);
        let token = args.get(3).cloned().unwrap_or_default();
        std::thread::sleep(std::time::Duration::from_secs(seconds));
        let still_ours = safety::read_pending().map(|pending| !token.is_empty() && pending.guard_token == token).unwrap_or(false);
        if !still_ours { std::process::exit(0); }
        let code = match modem_manager::ModemManagerClient::new() { Ok(client) => match client.recover_pending_band_change() { Ok(_) => 0, Err(_) => 1 }, Err(_) => 1 };
        std::process::exit(code);
    }
    if args.iter().any(|arg| arg == "--monitor") { std::process::exit(monitor::run()); }
    let app = gtk::Application::builder().application_id("net.milmit.ModemDeck").build();
    app.connect_activate(ui::build);
    app.run()
}
