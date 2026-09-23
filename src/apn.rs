use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct ApnProfile {
    pub name: String,
    pub uuid: String,
    pub apn: String,
    pub username: String,
    pub autoconnect: bool,
    pub active: bool,
}

fn run_nmcli(args: &[&str]) -> Result<String, String> {
    let out = Command::new("nmcli")
        .args(args)
        .output()
        .map_err(|e| if e.kind() == std::io::ErrorKind::NotFound {
            "nmcli/NetworkManager is not installed.".to_string()
        } else { format!("Could not start nmcli: {e}") })?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() { "NetworkManager rejected the APN operation.".into() } else { err })
    }
}

pub fn list() -> Result<Vec<ApnProfile>, String> {
    let raw = run_nmcli(&["-t", "-f", "NAME,UUID,TYPE,AUTOCONNECT", "connection", "show"])?;
    let active_raw = run_nmcli(&["-t", "-f", "UUID", "connection", "show", "--active"]).unwrap_or_default();
    let active: std::collections::HashSet<String> = active_raw.lines().map(|s| s.trim().to_string()).collect();
    let mut profiles = Vec::new();

    for line in raw.lines() {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() < 4 || parts[2] != "gsm" { continue; }
        let name = parts[0].to_string();
        let uuid = parts[1].to_string();
        let details = run_nmcli(&["-g", "gsm.apn,gsm.username", "connection", "show", "uuid", &uuid]).unwrap_or_default();
        let mut d = details.lines();
        let apn = d.next().unwrap_or("").trim().to_string();
        let username = d.next().unwrap_or("").trim().to_string();
        profiles.push(ApnProfile {
            name,
            uuid: uuid.clone(),
            apn,
            username,
            autoconnect: parts[3].eq_ignore_ascii_case("yes"),
            active: active.contains(&uuid),
        });
    }
    Ok(profiles)
}

fn valid_apn(apn: &str) -> bool {
    !apn.is_empty() && apn.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
}

pub fn create(name: &str, apn: &str, username: &str, password: &str, autoconnect: bool) -> Result<(), String> {
    let name = name.trim();
    let apn = apn.trim().to_ascii_lowercase();
    if name.is_empty() { return Err("Connection name cannot be empty.".into()); }
    if !valid_apn(&apn) { return Err("APN may only contain lowercase letters, digits, dots and hyphens.".into()); }
    let mut args = vec!["connection", "add", "type", "gsm", "ifname", "*", "con-name", name, "apn", apn.as_str()];
    if !username.trim().is_empty() { args.extend(["user", username.trim()]); }
    run_nmcli(&args)?;
    let value = if autoconnect { "yes" } else { "no" };
    let _ = run_nmcli(&["connection", "modify", "id", name, "connection.autoconnect", value]);
    if !password.is_empty() {
        run_nmcli(&["connection", "modify", "id", name, "gsm.password", password])?;
    }
    Ok(())
}

pub fn update(uuid: &str, name: &str, apn: &str, username: &str, password: &str, autoconnect: bool) -> Result<(), String> {
    let apn = apn.trim().to_ascii_lowercase();
    if name.trim().is_empty() { return Err("Connection name cannot be empty.".into()); }
    if !valid_apn(&apn) { return Err("APN may only contain lowercase letters, digits, dots and hyphens.".into()); }
    run_nmcli(&[
        "connection", "modify", "uuid", uuid,
        "connection.id", name.trim(),
        "connection.autoconnect", if autoconnect { "yes" } else { "no" },
        "gsm.apn", &apn,
        "gsm.username", username.trim(),
    ])?;
    if !password.is_empty() {
        run_nmcli(&["connection", "modify", "uuid", uuid, "gsm.password", password])?;
    }
    Ok(())
}

pub fn activate(uuid: &str) -> Result<(), String> {
    run_nmcli(&["connection", "up", "uuid", uuid]).map(|_| ())
}

pub fn delete(uuid: &str) -> Result<(), String> {
    run_nmcli(&["connection", "delete", "uuid", uuid]).map(|_| ())
}
