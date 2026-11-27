//! Local-only Remote Management HTTP Server (disabled by default).
//! Enforces localhost binding (127.0.0.1), token authentication, read-only default,
//! per-IP rate limiting, and Origin/CSRF validation.

use crate::core::runtime_status;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct RemoteServerConfig {
    pub enabled: bool,
    pub port: u16,
    pub auth_token: String,
    pub read_only: bool,
}

impl Default for RemoteServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8787,
            auth_token: String::new(),
            read_only: true,
        }
    }
}

pub fn start_localhost_server(config: RemoteServerConfig) -> Result<(), String> {
    if !config.enabled {
        return Err("Remote management is disabled by default.".into());
    }
    if config.auth_token.trim().len() < 12 {
        return Err(
            "Remote management requires an authentication token of at least 12 characters.".into(),
        );
    }
    let addr = format!("127.0.0.1:{}", config.port);
    let listener = TcpListener::bind(&addr).map_err(|e| format!("Bind {addr} failed: {e}"))?;
    let rate_limiter: Arc<Mutex<HashMap<String, (u32, Instant)>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let token = Arc::new(config.auth_token);

    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let token = token.clone();
            let limiter = rate_limiter.clone();
            std::thread::spawn(move || handle_client(stream, &token, &limiter));
        }
    });
    Ok(())
}

fn handle_client(
    mut stream: TcpStream,
    expected_token: &str,
    limiter: &Mutex<HashMap<String, (u32, Instant)>>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let peer = stream
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "unknown".into());

    // Rate limit: max 60 requests / 60s per IP
    if let Ok(mut map) = limiter.lock() {
        let now = Instant::now();
        let entry = map.entry(peer).or_insert((0, now));
        if now.duration_since(entry.1) > Duration::from_secs(60) {
            *entry = (1, now);
        } else {
            entry.0 += 1;
            if entry.0 > 60 {
                let _ = write_http(&mut stream, 429, "text/plain", "Rate limit exceeded");
                return;
            }
        }
    }

    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let path = parts[1];

    let mut auth_header = String::new();
    let mut origin_header = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" || line == "\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("authorization:") {
            auth_header = line[14..].trim().to_string();
        } else if lower.starts_with("x-modemdeck-token:") {
            auth_header = line[18..].trim().to_string();
        } else if lower.starts_with("origin:") {
            origin_header = line[7..].trim().to_string();
        }
    }

    // CSRF / Origin check
    if !origin_header.is_empty()
        && !origin_header.starts_with("http://127.0.0.1")
        && !origin_header.starts_with("http://localhost")
    {
        let _ = write_http(&mut stream, 403, "text/plain", "Forbidden Origin");
        return;
    }

    // Read-only enforcement
    if method != "GET" {
        let _ = write_http(&mut stream, 405, "text/plain", "Read-only mode enabled");
        return;
    }

    // Auth check
    let bearer = auth_header.strip_prefix("Bearer ").unwrap_or(&auth_header);
    if !constant_time_eq(bearer.as_bytes(), expected_token.as_bytes()) {
        let _ = write_http(&mut stream, 401, "text/plain", "Unauthorized");
        return;
    }

    match path {
        "/api/status" => {
            let json = std::fs::read_to_string(runtime_status::status_path())
                .unwrap_or_else(|_| "{}".into());
            let _ = write_http(&mut stream, 200, "application/json", &json);
        }
        "/" => {
            let status_json = std::fs::read_to_string(runtime_status::status_path())
                .unwrap_or_else(|_| "{}".into());
            let html = format!(
                "<!doctype html><html><head><meta charset='utf-8'><title>ModemDeck Local Status</title></head>\
                 <body><h2>ModemDeck Local Read-Only View</h2><pre>{status_json}</pre></body></html>"
            );
            let _ = write_http(&mut stream, 200, "text/html; charset=utf-8", &html);
        }
        _ => {
            let _ = write_http(&mut stream, 404, "text/plain", "Not Found");
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn write_http(
    stream: &mut TcpStream,
    code: u16,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let resp = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes())
}
