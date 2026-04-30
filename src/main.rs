mod auth;
mod cli;
mod client;
mod commands;
mod error;
mod models;
mod output;
mod tui;

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser};
use tracing_subscriber::EnvFilter;

use cli::{Cli, Commands};
use client::SeggwatClient;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing. TUI mode uses the alternate screen, so suppress
    // log output (unless -v is set, in which case we assume the user wants it).
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else if matches!(cli.command, Commands::Tui { .. }) {
        EnvFilter::new("off")
    } else {
        EnvFilter::new("info")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let json = cli.json;

    // Handle commands that don't need authentication
    match &cli.command {
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(*shell, &mut cmd, "seggwat", &mut std::io::stdout());
            return Ok(());
        }
        Commands::Login {
            zitadel_domain,
            client_id,
        } => {
            return commands::auth::execute_login(
                &cli.api_url,
                zitadel_domain.as_deref(),
                client_id.as_deref(),
            )
            .await;
        }
        Commands::Logout => {
            return commands::auth::execute_logout();
        }
        _ => {}
    }

    // Resolve authentication: API key > cached OAuth token
    let client = if let Some(api_key) = cli.api_key {
        if !api_key.starts_with("oat_") {
            tracing::warn!("API key does not start with 'oat_' prefix");
        }
        SeggwatClient::with_api_key(&cli.api_url, api_key)?
    } else if let Some(token_store) = auth::load_tokens()? {
        let store = if auth::is_expired(&token_store) {
            tracing::debug!("Access token expired, attempting refresh...");
            match auth::refresh_access_token(&token_store).await {
                Ok(refreshed) => {
                    tracing::debug!("Token refreshed successfully");
                    refreshed
                }
                Err(e) => {
                    tracing::debug!("Token refresh failed: {e}");
                    bail!("Session expired. Run `seggwat login` to re-authenticate.");
                }
            }
        } else {
            token_store
        };
        SeggwatClient::with_bearer_token(&cli.api_url, store.access_token)?
    } else {
        bail!(
            "Not authenticated. Run `seggwat login` or set --api-key / SEGGWAT_API_KEY environment variable."
        );
    };

    // Dispatch authenticated commands
    match cli.command {
        Commands::Project { command } => {
            commands::project::execute(&client, command, json).await?;
        }
        Commands::Feedback { command } => {
            commands::feedback::execute(&client, command, json).await?;
        }
        Commands::Rating { command } => {
            commands::rating::execute(&client, command, json).await?;
        }
        Commands::Whoami => {
            commands::auth::execute_whoami(&client, json).await?;
        }
        Commands::Tui { project_id } => {
            tui::run(client, cli.api_url.clone(), project_id).await?;
        }
        Commands::Completions { .. } | Commands::Login { .. } | Commands::Logout => {
            unreachable!("handled above")
        }
    }

    Ok(())
}
