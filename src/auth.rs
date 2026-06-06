//! OAuth Authorization Code + PKCE flow and token storage.

use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;

use crate::client::build_http_client;
use crate::error::CliError;

/// Stored credential obtained via `seggwat login`.
///
/// The dashboard's token endpoint mints a long-lived `oat_` organization access
/// token (no refresh token, no expiry), so this is effectively a
/// browser-acquired API key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStore {
    pub access_token: String,
    /// The API URL this token was issued for.
    pub api_url: String,
}

/// Token endpoint success response.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

/// Token endpoint error response.
#[derive(Debug, Deserialize)]
struct TokenErrorResponse {
    #[allow(dead_code)]
    error: String,
    error_description: Option<String>,
}

// ============================================================================
// Token Storage
// ============================================================================

/// Path to the token store file.
pub fn token_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let config_dir = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    PathBuf::from(config_dir)
        .join("seggwat")
        .join("tokens.json")
}

/// Load cached tokens from disk.
pub fn load_tokens() -> Result<Option<TokenStore>, CliError> {
    let path = token_path();
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path).map_err(|e| CliError::TokenStorage {
        message: format!("Failed to read {}: {e}", path.display()),
    })?;
    let store: TokenStore =
        serde_json::from_str(&contents).map_err(|e| CliError::TokenStorage {
            message: format!("Failed to parse {}: {e}", path.display()),
        })?;
    Ok(Some(store))
}

/// Save tokens to disk with restricted permissions.
pub fn save_tokens(store: &TokenStore) -> Result<(), CliError> {
    let path = token_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| CliError::TokenStorage {
            message: format!("Failed to create {}: {e}", parent.display()),
        })?;
    }
    let json = serde_json::to_string_pretty(store).map_err(|e| CliError::TokenStorage {
        message: format!("Failed to serialize tokens: {e}"),
    })?;
    std::fs::write(&path, &json).map_err(|e| CliError::TokenStorage {
        message: format!("Failed to write {}: {e}", path.display()),
    })?;

    // Set file permissions to 0600 (owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms).map_err(|e| CliError::TokenStorage {
            message: format!("Failed to set permissions on {}: {e}", path.display()),
        })?;
    }

    Ok(())
}

/// Remove cached tokens.
pub fn clear_tokens() -> Result<(), CliError> {
    let path = token_path();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| CliError::TokenStorage {
            message: format!("Failed to remove {}: {e}", path.display()),
        })?;
    }
    Ok(())
}

// ============================================================================
// PKCE
// ============================================================================

/// Generate a PKCE code verifier and S256 challenge.
pub fn generate_pkce() -> (String, String) {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
    let verifier = URL_SAFE_NO_PAD.encode(&bytes);

    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(digest);

    (verifier, challenge)
}

/// Generate a random state parameter for CSRF protection.
pub fn generate_state() -> String {
    let mut rng = rand::rng();
    let bytes: Vec<u8> = (0..16).map(|_| rng.random::<u8>()).collect();
    URL_SAFE_NO_PAD.encode(&bytes)
}

// ============================================================================
// Login Flow
// ============================================================================

/// OAuth client identifier sent to the dashboard's authorization server. The
/// dashboard does not treat the client_id value as a security control (the real
/// boundaries are the redirect_uri allowlist, the user's session, consent, and
/// PKCE), so a fixed identifier works for every instance.
const CLIENT_ID: &str = "seggwat-cli";

/// Run the OAuth Authorization Code + PKCE login flow against the dashboard's
/// own authorization server (`/oauth/authorize` + `/oauth/token`).
///
/// 1. Generate PKCE verifier/challenge and random state
/// 2. Start a local callback server on a loopback `/callback`
/// 3. Open the browser to the authorize URL
/// 4. Wait for the callback with the auth code
/// 5. Exchange the code for a long-lived `oat_` access token
/// 6. Save and return it
pub async fn login(api_url: &str) -> Result<TokenStore, CliError> {
    let api_url = api_url.trim_end_matches('/');
    let (verifier, challenge) = generate_pkce();
    let state = generate_state();

    // Bind a loopback callback server. The dashboard's redirect_uri allowlist
    // requires a loopback host with the path exactly `/callback` and no query.
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| CliError::LoginFailed {
            message: format!("Failed to bind callback server: {e}"),
        })?;
    let port = listener.local_addr().unwrap().port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let authorize_url = format!(
        "{api_url}/oauth/authorize?\
         response_type=code\
         &client_id={CLIENT_ID}\
         &redirect_uri={}\
         &code_challenge={challenge}\
         &code_challenge_method=S256\
         &state={state}",
        urlencoding::encode(&redirect_uri),
    );

    // Try to open browser, print URL as fallback
    println!("Opening browser for authentication...");
    if open::that(&authorize_url).is_err() {
        println!("Could not open browser. Please visit this URL:");
        println!();
        println!("  {authorize_url}");
        println!();
    }

    // Wait for the callback (120s timeout)
    let code = wait_for_callback(listener, &state).await?;

    // Exchange the authorization code for an access token
    let token_url = format!("{api_url}/oauth/token");
    let http_client = build_http_client()?;
    let resp = http_client
        .post(&token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", &code),
            ("redirect_uri", &redirect_uri),
            ("code_verifier", &verifier),
        ])
        .send()
        .await
        .map_err(|e| CliError::LoginFailed {
            message: format!("Token exchange request failed: {e}"),
        })?;

    if !resp.status().is_success() {
        let err: TokenErrorResponse = resp.json().await.unwrap_or(TokenErrorResponse {
            error: "unknown".to_string(),
            error_description: Some("Token exchange failed".to_string()),
        });
        return Err(CliError::LoginFailed {
            message: err
                .error_description
                .unwrap_or_else(|| "Token exchange failed".to_string()),
        });
    }

    let token_resp: TokenResponse = resp.json().await.map_err(|e| CliError::LoginFailed {
        message: format!("Failed to parse token response: {e}"),
    })?;

    let store = TokenStore {
        access_token: token_resp.access_token,
        api_url: api_url.to_string(),
    };

    save_tokens(&store)?;
    Ok(store)
}

/// Wait for the OAuth callback on the local server.
///
/// Browsers frequently pre-open speculative TCP connections that carry no
/// request, so we loop over `accept()` and skip any connection that doesn't
/// carry our callback query, until the real redirect arrives or the overall
/// 120s deadline elapses.
async fn wait_for_callback(
    listener: TcpListener,
    expected_state: &str,
) -> Result<String, CliError> {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(120);

    loop {
        let (mut stream, _) = tokio::time::timeout_at(deadline, listener.accept())
            .await
            .map_err(|_| CliError::LoginTimeout)?
            .map_err(|e| CliError::LoginFailed {
                message: format!("Failed to accept callback connection: {e}"),
            })?;

        // Read just the request line; the rest of the request is irrelevant.
        let mut request_line = String::new();
        {
            let mut reader = BufReader::new(&mut stream);
            if reader.read_line(&mut request_line).await.is_err() || request_line.trim().is_empty() {
                continue; // stray pre-connect with no data — keep waiting
            }
        }

        // Parse: GET /callback?code=xxx&state=yyy HTTP/1.1
        let Some(path) = request_line.split_whitespace().nth(1) else {
            continue;
        };
        let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
        let params: std::collections::HashMap<String, String> = query
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .map(|(k, v)| {
                (
                    urlencoding::decode(k)
                        .unwrap_or_else(|_| k.into())
                        .into_owned(),
                    urlencoding::decode(v)
                        .unwrap_or_else(|_| v.into())
                        .into_owned(),
                )
            })
            .collect();

        // Connections without our callback params (favicon, preconnect, etc.)
        // are not the redirect we're waiting for — answer and keep listening.
        if !params.contains_key("code") && !params.contains_key("error") {
            send_http_response(&mut stream, "404 Not Found", "<html><body>Not found.</body></html>")
                .await;
            continue;
        }

        // Error response from the authorization server.
        if let Some(error) = params.get("error") {
            let unknown = "Unknown error".to_string();
            let desc = params.get("error_description").unwrap_or(&unknown);
            let html = format!(
                "<html><body><h1>Login Failed</h1><p>{desc}</p><p>You can close this tab.</p></body></html>"
            );
            send_http_response(&mut stream, "400 Bad Request", &html).await;
            return Err(CliError::LoginFailed {
                message: format!("{error}: {desc}"),
            });
        }

        // Verify state (CSRF).
        let received_state = params.get("state").ok_or_else(|| CliError::LoginFailed {
            message: "Missing state parameter in callback".to_string(),
        })?;
        if received_state.as_str() != expected_state {
            let html = "<html><body><h1>Login Failed</h1><p>State mismatch (possible CSRF). Please try again.</p></body></html>";
            send_http_response(&mut stream, "400 Bad Request", html).await;
            return Err(CliError::LoginFailed {
                message: "State parameter mismatch — possible CSRF attack".to_string(),
            });
        }

        // Extract authorization code.
        let code = params
            .get("code")
            .ok_or_else(|| CliError::LoginFailed {
                message: "Missing authorization code in callback".to_string(),
            })?
            .to_string();

        let html = "<html><body>\
            <h1>Login Successful</h1>\
            <p>You can close this tab and return to the terminal.</p>\
            </body></html>";
        send_http_response(&mut stream, "200 OK", html).await;

        return Ok(code);
    }
}

/// Send a minimal HTTP response.
async fn send_http_response(stream: &mut tokio::net::TcpStream, status: &str, body: &str) {
    use tokio::io::AsyncWriteExt;
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;
}
