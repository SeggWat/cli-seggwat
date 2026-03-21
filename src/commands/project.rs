use anyhow::Result;

use crate::cli::ProjectCommand;
use crate::client::SeggwatClient;
use crate::output;

pub async fn execute(client: &SeggwatClient, command: ProjectCommand, json: bool) -> Result<()> {
    match command {
        ProjectCommand::List => {
            let resp = client.list_projects().await?;
            if json {
                output::print_json(&resp)?;
            } else {
                output::print_projects_table(&resp.projects);
            }
        }
        ProjectCommand::Get { project_id } => {
            let project = client.get_project(&project_id).await?;
            if json {
                output::print_json(&project)?;
            } else {
                output::print_project_detail(&project);
            }
        }
        ProjectCommand::Summary { project_id } => {
            let summary = client.get_project_summary(&project_id).await?;
            if json {
                output::print_json(&summary)?;
            } else {
                output::print_project_summary(&summary);
            }
        }
    }
    Ok(())
}
