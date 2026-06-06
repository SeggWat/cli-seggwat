//! Login, logout, and whoami command handlers.

use anyhow::Result;

use crate::auth;
use crate::client::SeggwatClient;
use crate::output;

/// Execute the `login` command.
pub async fn execute_login(api_url: &str) -> Result<()> {
    auth::login(api_url).await?;
    println!("Logged in successfully.");
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
