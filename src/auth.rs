//! OAuth Authorization Code + PKCE flow and token storage.

use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;

use crate::client::build_http_client;
use crate::error::CliError;

/// Stored OAuth tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenStore {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix timestamp when the access token expires.
    pub expires_at: Option<i64>,
    /// The API URL these tokens are for.
    pub api_url: String,
    /// Zitadel domain (needed for token refresh).
    pub zitadel_domain: String,
    /// OAuth client ID (needed for token refresh).
    pub client_id: String,
}

/// Token endpoint response from Zitadel.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
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

/// Check whether the stored access token has expired.
pub fn is_expired(store: &TokenStore) -> bool {
    match store.expires_at {
        Some(exp) => {
            let now = chrono::Utc::now().timestamp();
            now >= (exp - 30) // 30-second buffer
        }
        None => false, // No expiry info — assume valid
    }
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

/// Run the full OAuth Authorization Code + PKCE login flow.
///
/// 1. Generate PKCE verifier/challenge and random state
/// 2. Start a local callback server
/// 3. Open browser to authorize URL
/// 4. Wait for callback with auth code
/// 5. Exchange code for tokens
/// 6. Save and return tokens
pub async fn login(
    api_url: &str,
    zitadel_domain: &str,
    client_id: &str,
) -> Result<TokenStore, CliError> {
    let (verifier, challenge) = generate_pkce();
    let state = generate_state();

    // Bind to a random available port on localhost
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| CliError::LoginFailed {
            message: format!("Failed to bind callback server: {e}"),
        })?;
    let port = listener.local_addr().unwrap().port();
    let redirect_uri = format!("http://localhost:{port}");

    // Build the authorize URL
    let scheme = if zitadel_domain.starts_with("localhost") || zitadel_domain.starts_with("127.") {
        "http"
    } else {
        "https"
    };
    let scopes = "openid profile email offline_access urn:zitadel:iam:org:project:id:zitadel:aud";
    let authorize_url = format!(
        "{scheme}://{zitadel_domain}/oauth/v2/authorize?\
         client_id={client_id}\
         &redirect_uri={}\
         &response_type=code\
         &scope={}\
         &code_challenge={challenge}\
         &code_challenge_method=S256\
         &state={state}",
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(scopes),
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

    // Exchange the authorization code for tokens
    let token_url = format!("{scheme}://{zitadel_domain}/oauth/v2/token");
    let http_client = build_http_client()?;
    let resp = http_client
        .post(&token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
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

    let expires_at = token_resp
        .expires_in
        .map(|secs| chrono::Utc::now().timestamp() + secs);

    let store = TokenStore {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token,
        expires_at,
        api_url: api_url.to_string(),
        zitadel_domain: zitadel_domain.to_string(),
        client_id: client_id.to_string(),
    };

    save_tokens(&store)?;
    Ok(store)
}

/// Wait for the OAuth callback on the local server.
async fn wait_for_callback(
    listener: TcpListener,
    expected_state: &str,
) -> Result<String, CliError> {
    let timeout = tokio::time::Duration::from_secs(120);

    let (stream, _) = tokio::time::timeout(timeout, listener.accept())
        .await
        .map_err(|_| CliError::LoginTimeout)?
        .map_err(|e| CliError::LoginFailed {
            message: format!("Failed to accept callback connection: {e}"),
        })?;

    // Convert to std for synchronous reading (single request, simple parsing)
    let std_stream = stream.into_std().map_err(|e| CliError::LoginFailed {
        message: format!("Stream conversion failed: {e}"),
    })?;
    std_stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();

    let mut reader = BufReader::new(&std_stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| CliError::LoginFailed {
            message: format!("Failed to read callback request: {e}"),
        })?;

    // Parse: GET /callback?code=xxx&state=yyy HTTP/1.1
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| CliError::LoginFailed {
            message: "Invalid callback request".to_string(),
        })?;

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

    // Check for error response
    let unknown = "Unknown error".to_string();
    if let Some(error) = params.get("error") {
        let desc = params.get("error_description").unwrap_or(&unknown);
        let html = format!(
            "<html><body><h1>Login Failed</h1><p>{desc}</p><p>You can close this tab.</p></body></html>"
        );
        send_http_response(&std_stream, "400 Bad Request", &html);
        return Err(CliError::LoginFailed {
            message: format!("{error}: {desc}"),
        });
    }

    // Verify state
    let received_state = params.get("state").ok_or_else(|| CliError::LoginFailed {
        message: "Missing state parameter in callback".to_string(),
    })?;

    if *received_state != expected_state {
        let html = "<html><body><h1>Login Failed</h1><p>State mismatch (possible CSRF). Please try again.</p></body></html>";
        send_http_response(&std_stream, "400 Bad Request", html);
        return Err(CliError::LoginFailed {
            message: "State parameter mismatch — possible CSRF attack".to_string(),
        });
    }

    // Extract authorization code
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
    send_http_response(&std_stream, "200 OK", html);

    Ok(code)
}

/// Send a minimal HTTP response.
fn send_http_response(stream: &std::net::TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );
    let mut writer = std::io::BufWriter::new(stream);
    let _ = writer.write_all(response.as_bytes());
    let _ = writer.flush();
}

// ============================================================================
// Token Refresh
// ============================================================================

/// Refresh the access token using the stored refresh token.
pub async fn refresh_access_token(store: &TokenStore) -> Result<TokenStore, CliError> {
    let refresh_token = store
        .refresh_token
        .as_deref()
        .ok_or(CliError::TokenRefreshFailed {
            message: "No refresh token available".to_string(),
        })?;

    let scheme = if store.zitadel_domain.starts_with("localhost")
        || store.zitadel_domain.starts_with("127.")
    {
        "http"
    } else {
        "https"
    };
    let token_url = format!("{scheme}://{}/oauth/v2/token", store.zitadel_domain);

    let http_client = build_http_client()?;
    let resp = http_client
        .post(&token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &store.client_id),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|e| CliError::TokenRefreshFailed {
            message: format!("Refresh request failed: {e}"),
        })?;

    if !resp.status().is_success() {
        let err: TokenErrorResponse = resp.json().await.unwrap_or(TokenErrorResponse {
            error: "unknown".to_string(),
            error_description: Some("Token refresh failed".to_string()),
        });
        return Err(CliError::TokenRefreshFailed {
            message: err
                .error_description
                .unwrap_or_else(|| "Token refresh failed".to_string()),
        });
    }

    let token_resp: TokenResponse =
        resp.json()
            .await
            .map_err(|e| CliError::TokenRefreshFailed {
                message: format!("Failed to parse refresh response: {e}"),
            })?;

    let expires_at = token_resp
        .expires_in
        .map(|secs| chrono::Utc::now().timestamp() + secs);

    let new_store = TokenStore {
        access_token: token_resp.access_token,
        refresh_token: token_resp.refresh_token.or(store.refresh_token.clone()),
        expires_at,
        api_url: store.api_url.clone(),
        zitadel_domain: store.zitadel_domain.clone(),
        client_id: store.client_id.clone(),
    };

    save_tokens(&new_store)?;
    Ok(new_store)
}
