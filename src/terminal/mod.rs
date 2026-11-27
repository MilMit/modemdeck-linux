//! Safe AT Command Terminal.
//! Defaults to Read-only mode, blocks dangerous NV/firmware writes, and
//! coordinates with ModemManager operation lock to prevent concurrent port access.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtSafetyClass {
    ReadOnly,
    StateChanging,
    DangerousBlocked,
}

impl AtSafetyClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "Read-Only",
            Self::StateChanging => "State-Changing (Confirmation Required)",
            Self::DangerousBlocked => "Blocked (Unsafe NV/Firmware/RF Command)",
        }
    }
}

pub fn classify_at_command(cmd: &str) -> AtSafetyClass {
    let upper = cmd.trim().to_ascii_uppercase();
    // Block undocumented NV, EFS, firmware flash, IMEI, or calibration writes
    const BLOCKED_PREFIXES: &[&str] = &[
        "AT+NV",
        "AT^NV",
        "AT!NV",
        "AT+QNV",
        "AT+EGMR",
        "AT+QFOTADL",
        "AT+CFUN=6",
        "AT+CFUN=7",
        "AT^SFM",
        "AT!OPENLOCK",
        "AT!ENTERCND",
    ];
    if BLOCKED_PREFIXES.iter().any(|p| upper.starts_with(p)) {
        return AtSafetyClass::DangerousBlocked;
    }

    const SAFE_READ_ONLY: &[&str] = &[
        "AT",
        "ATI",
        "AT+CGMI",
        "AT+CGMM",
        "AT+CGMR",
        "AT+CSQ",
        "AT+CESQ",
        "AT+COPS?",
        "AT+CREG?",
        "AT+CEREG?",
        "AT+C5GREG?",
        "AT^CELLINFO",
        "AT^CA_INFO",
        "AT+QENG=",
        "AT+QNWINFO",
        "AT!GSTATUS?",
        "AT+CFUN?",
        "AT+CPIN?",
    ];
    if SAFE_READ_ONLY
        .iter()
        .any(|p| upper == *p || upper.starts_with(p) && !upper.contains('='))
        || upper.ends_with('?')
    {
        return AtSafetyClass::ReadOnly;
    }

    AtSafetyClass::StateChanging
}

pub fn execute_at_command(
    port_path: &str,
    command: &str,
    allow_state_changing: bool,
    timeout_ms: u64,
) -> Result<String, String> {
    let class = classify_at_command(command);
    match class {
        AtSafetyClass::DangerousBlocked => {
            return Err("Command blocked: potentially unsafe NV/firmware/RF modification.".into());
        }
        AtSafetyClass::StateChanging if !allow_state_changing => {
            return Err("Terminal is in Read-Only mode. Enable state-changing commands with confirmation to run this command.".into());
        }
        _ => {}
    }

    crate::core::safety::try_acquire_operation_lock(
        port_path,
        crate::core::safety::ModemOperationOwner::AtTerminal,
    )
    .map_err(|e| format!("Busy: {e}"))?;

    struct LockGuard<'a>(&'a str);
    impl Drop for LockGuard<'_> {
        fn drop(&mut self) {
            crate::core::safety::release_operation_lock(
                self.0,
                crate::core::safety::ModemOperationOwner::AtTerminal,
            );
        }
    }
    let _guard = LockGuard(port_path);

    let mut port = serialport::new(port_path, 115_200)
        .timeout(Duration::from_millis(timeout_ms.clamp(300, 10_000)))
        .open()
        .map_err(|e| format!("Could not open AT port {port_path}: {e}"))?;

    let payload = format!("{}\r\n", command.trim());
    port.write_all(payload.as_bytes())
        .map_err(|e| format!("AT write failed: {e}"))?;

    let mut buf = [0u8; 4096];
    let mut response = String::new();
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms.clamp(300, 10_000));
    while std::time::Instant::now() < deadline {
        match port.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                response.push_str(&String::from_utf8_lossy(&buf[..n]));
                if response.contains("\nOK") || response.contains("\nERROR") {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(format!("AT read failed: {e}")),
        }
    }
    Ok(response.trim().to_string())
}

pub fn sanitize_at_output(raw: &str) -> String {
    raw.lines()
        .filter(|line| {
            let l = line.to_ascii_lowercase();
            !l.contains("imei") && !l.contains("imsi") && !l.contains("iccid")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
