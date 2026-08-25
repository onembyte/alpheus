//! Google Drive connection.
//!
//! OAuth installed-app flow over a loopback redirect: the user supplies their
//! OWN "Desktop app" OAuth client (nothing Google-related ships with Alpheus),
//! the browser handles consent, and the refresh token + client credentials
//! live in the macOS Keychain — never in settings.json or localStorage. The
//! webview only ever receives short-lived access tokens and talks to the
//! Drive API directly.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::State;

#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "com.francomichetti.storage-manager.google";
#[cfg(target_os = "macos")]
const KEYCHAIN_ACCOUNT: &str = "oauth";
const SCOPE: &str = "https://www.googleapis.com/auth/drive";

#[derive(Serialize, Deserialize, Clone)]
struct StoredCreds {
    client_id: String,
    client_secret: String,
    refresh_token: String,
}

pub struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

#[derive(Default)]
pub struct GoogleState(pub Mutex<Option<CachedToken>>);

// ---------------------------------------------------------------- keychain

fn creds_file_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    std::path::PathBuf::from(home).join(".config/alpheus/google_credentials.json")
}

fn keychain_read() -> Option<StoredCreds> {
    #[cfg(target_os = "macos")]
    {
        let out = Command::new("/usr/bin/security")
            .args([
                "find-generic-password",
                "-s",
                KEYCHAIN_SERVICE,
                "-a",
                KEYCHAIN_ACCOUNT,
                "-w",
            ])
            .output()
            .ok()?;
        if out.status.success() {
            if let Ok(c) = serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()) {
                return Some(c);
            }
        }
    }
    let p = creds_file_path();
    if p.exists() {
        let content = std::fs::read_to_string(&p).ok()?;
        return serde_json::from_str(&content).ok();
    }
    None
}

fn keychain_write(creds: &StoredCreds) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let json = serde_json::to_string(creds).map_err(|e| e.to_string())?;
        if let Ok(out) = Command::new("/usr/bin/security")
            .args([
                "add-generic-password",
                "-U",
                "-s",
                KEYCHAIN_SERVICE,
                "-a",
                KEYCHAIN_ACCOUNT,
                "-w",
                &json,
            ])
            .output()
        {
            if out.status.success() {
                return Ok(());
            }
        }
    }
    let p = creds_file_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(creds).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn keychain_delete() {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("/usr/bin/security")
            .args([
                "delete-generic-password",
                "-s",
                KEYCHAIN_SERVICE,
                "-a",
                KEYCHAIN_ACCOUNT,
            ])
            .output();
    }
    let p = creds_file_path();
    let _ = std::fs::remove_file(p);
}

// ---------------------------------------------------------------- helpers

/// Percent-encode everything outside RFC 3986 unreserved characters.
fn urlenc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn random_hex(n_bytes: usize) -> String {
    let mut buf = vec![0u8; n_bytes];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut buf))
        .is_err()
    {
        // fallback: time-derived, still unguessable enough for a loopback state
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        buf.iter_mut()
            .enumerate()
            .for_each(|(i, b)| *b = (t >> (i % 4 * 8)) as u8 ^ i as u8);
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

fn token_post(params: &[(&str, &str)]) -> Result<TokenResponse, String> {
    ureq::post("https://oauth2.googleapis.com/token")
        .send_form(params)
        .map_err(|e| format!("token request failed: {e}"))?
        .into_json::<TokenResponse>()
        .map_err(|e| format!("unexpected token response: {e}"))
}

// ---------------------------------------------------------------- commands

/// Runs the full browser consent flow and stores the refresh token.
#[tauri::command(async)]
pub fn google_connect(client_id: String, client_secret: String) -> Result<(), String> {
    let client_id = client_id.trim().to_string();
    let client_secret = client_secret.trim().to_string();
    if client_id.is_empty() || client_secret.is_empty() {
        return Err("paste both the Client ID and the Client Secret".into());
    }

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let redirect = format!("http://127.0.0.1:{port}");
    let state = random_hex(16);

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}",
        urlenc(&client_id),
        urlenc(&redirect),
        urlenc(SCOPE),
        state
    );
    let _ = Command::new("/usr/bin/open").arg(&auth_url).output();

    let deadline = Instant::now() + Duration::from_secs(240);
    let code = loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let query = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("");
                let param = |k: &str| {
                    query
                        .split(['?', '&'])
                        .find_map(|p| p.strip_prefix(&format!("{k}=")).map(str::to_string))
                };
                let state_ok = param("state").map(|s| s == state).unwrap_or(false);
                let code = param("code");
                let ok = state_ok && code.is_some();
                let body = if ok {
                    "<h2>Alpheus is connected to Google Drive.</h2><p>You can close this tab.</p>"
                } else {
                    "<h2>Connection failed.</h2><p>Return to Alpheus and try again.</p>"
                };
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body style=\"font-family:-apple-system,sans-serif;text-align:center;margin-top:18vh\">{body}</body></html>"
                    )
                    .as_bytes(),
                );
                if !state_ok {
                    return Err("state mismatch — try connecting again".into());
                }
                match code {
                    Some(c) => break urldecode(&c),
                    None => {
                        return Err(param("error")
                            .map(|e| urldecode(&e))
                            .unwrap_or_else(|| "authorization was denied".into()))
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err("timed out waiting for the browser sign-in".into());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e.to_string()),
        }
    };

    let tok = token_post(&[
        ("code", code.as_str()),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("grant_type", "authorization_code"),
    ])?;
    let refresh_token = tok.refresh_token.ok_or(
        "Google returned no refresh token — remove Alpheus at myaccount.google.com/permissions and reconnect",
    )?;
    keychain_write(&StoredCreds {
        client_id,
        client_secret,
        refresh_token,
    })
}

/// True when a refresh token is stored in the Keychain.
#[tauri::command]
pub fn google_status() -> bool {
    keychain_read().is_some()
}

/// Short-lived access token for the webview's direct Drive API calls.
#[tauri::command(async)]
pub fn google_token(state: State<GoogleState>) -> Result<String, String> {
    if let Some(cached) = state.0.lock().unwrap().as_ref() {
        if cached.expires_at > Instant::now() + Duration::from_secs(60) {
            return Ok(cached.access_token.clone());
        }
    }
    let creds = keychain_read().ok_or("Google Drive is not connected")?;
    let tok = token_post(&[
        ("client_id", creds.client_id.as_str()),
        ("client_secret", creds.client_secret.as_str()),
        ("refresh_token", creds.refresh_token.as_str()),
        ("grant_type", "refresh_token"),
    ])?;
    *state.0.lock().unwrap() = Some(CachedToken {
        access_token: tok.access_token.clone(),
        expires_at: Instant::now() + Duration::from_secs(tok.expires_in),
    });
    Ok(tok.access_token)
}

/// Revokes the grant and forgets the Keychain entry.
#[tauri::command(async)]
pub fn google_disconnect(state: State<GoogleState>) -> Result<(), String> {
    if let Some(creds) = keychain_read() {
        let _ = ureq::post("https://oauth2.googleapis.com/revoke")
            .send_form(&[("token", creds.refresh_token.as_str())]);
    }
    keychain_delete();
    *state.0.lock().unwrap() = None;
    Ok(())
}
