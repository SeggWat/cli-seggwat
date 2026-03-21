//! Login, logout, and whoami command handlers.

use anyhow::{Result, bail};

use crate::auth;
use crate::client::SeggwatClient;
use crate::output;

/// Default Zitadel domain for the SaaS instance.
const SAAS_ZITADEL_DOMAIN: &str = "id.oxidt.com";

/// Default OAuth client ID for the SaaS instance.
const SAAS_CLIENT_ID: &str = "364773505570443267";

/// Resolve the Zitadel domain from the API URL and optional override.
fn resolve_zitadel_domain(api_url: &str, override_domain: Option<&str>) -> Result<String> {
    if let Some(domain) = override_domain {
        return Ok(domain.to_string());
    }
    if api_url.contains("seggwat.com") {
        return Ok(SAAS_ZITADEL_DOMAIN.to_string());
    }
    if api_url.contains("localhost") || api_url.contains("127.0.0.1") {
        return Ok("localhost:8085".to_string());
    }
    bail!(
        "Cannot auto-detect Zitadel domain for '{api_url}'.\n\
         Use --zitadel-domain or set SEGGWAT_ZITADEL_DOMAIN."
    )
}

/// Resolve the OAuth client ID from the API URL and optional override.
fn resolve_client_id(api_url: &str, override_id: Option<&str>) -> Result<String> {
    if let Some(id) = override_id {
        return Ok(id.to_string());
    }
    if api_url.contains("seggwat.com") {
        return Ok(SAAS_CLIENT_ID.to_string());
    }
    bail!(
        "OAuth client ID required for non-SaaS instances.\n\
         Use --client-id or set SEGGWAT_CLIENT_ID."
    )
}

/// Execute the `login` command.
pub async fn execute_login(
    api_url: &str,
    zitadel_domain: Option<&str>,
    client_id: Option<&str>,
) -> Result<()> {
    let domain = resolve_zitadel_domain(api_url, zitadel_domain)?;
    let cid = resolve_client_id(api_url, client_id)?;

    let store = auth::login(api_url, &domain, &cid).await?;

    println!("Logged in successfully.");
    tracing::debug!(
        "Token expires at: {:?}, has refresh token: {}",
        store.expires_at,
        store.refresh_token.is_some()
    );

    Ok(())
}

/// Execute the `logout` command.
pub fn execute_logout() -> Result<()> {
    auth::clear_tokens()?;
    println!("Logged out. Cached tokens have been removed.");
    Ok(())
}

/// Execute the `whoami` command.
pub async fn execute_whoami(client: &SeggwatClient, json: bool) -> Result<()> {
    let info = client.whoami().await?;
    if json {
        output::print_json(&info)?;
    } else {
        output::print_whoami(&info);
    }
    Ok(())
}
