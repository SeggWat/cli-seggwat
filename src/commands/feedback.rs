use anyhow::{Result, bail};

use crate::cli::FeedbackCommand;
use crate::client::SeggwatClient;
use crate::models::{FeedbackCreateRequest, FeedbackUpdateRequest};
use crate::output;

pub async fn execute(client: &SeggwatClient, command: FeedbackCommand, json: bool) -> Result<()> {
    match command {
        FeedbackCommand::List {
            project_id,
            page,
            limit,
            status,
            r#type,
            search,
        } => {
            let status_str = status.as_ref().map(|s| s.to_string());
            let type_str = r#type.as_ref().map(|t| t.to_string());
            let resp = client
                .list_feedback(
                    &project_id,
                    page,
                    limit,
                    status_str.as_deref(),
                    type_str.as_deref(),
                    search.as_deref(),
                )
                .await?;
            if json {
                output::print_json(&resp)?;
            } else {
                output::print_feedback_table(&resp.feedback, &resp.pagination);
            }
        }
        FeedbackCommand::Get {
            project_id,
            feedback_id,
        } => {
            let feedback = client.get_feedback(&project_id, &feedback_id).await?;
            if json {
                output::print_json(&feedback)?;
            } else {
                output::print_feedback_detail(&feedback);
            }
        }
        FeedbackCommand::Create {
            project_id,
            message,
            r#type,
            path,
            version,
        } => {
            let body = FeedbackCreateRequest {
                message,
                feedback_type: r#type.map(|t| t.to_model()),
                path,
                version,
            };
            let feedback = client.create_feedback(&project_id, &body).await?;
            if json {
                output::print_json(&feedback)?;
            } else {
                println!("Feedback created: {}", feedback.id);
                output::print_feedback_detail(&feedback);
            }
        }
        FeedbackCommand::Update {
            project_id,
            feedback_id,
            message,
            r#type,
            status,
            resolution_note,
        } => {
            let body = FeedbackUpdateRequest {
                message,
                feedback_type: r#type.map(|t| t.to_model()),
                status: status.map(|s| s.to_model()),
                resolution_note,
            };
            if body.message.is_none()
                && body.feedback_type.is_none()
                && body.status.is_none()
                && body.resolution_note.is_none()
            {
                bail!(
                    "Nothing to update. Provide at least one of --message, --type, --status, or --resolution-note."
                );
            }
            let feedback = client
                .update_feedback(&project_id, &feedback_id, &body)
                .await?;
            if json {
                output::print_json(&feedback)?;
            } else {
                println!("Feedback updated: {}", feedback.id);
                output::print_feedback_detail(&feedback);
            }
        }
        FeedbackCommand::Delete {
            project_id,
            feedback_id,
        } => {
            client.delete_feedback(&project_id, &feedback_id).await?;
            if json {
                output::print_json(&serde_json::json!({"deleted": feedback_id}))?;
            } else {
                println!("Feedback deleted: {feedback_id}");
            }
        }
        FeedbackCommand::Stats { project_id } => {
            let stats = client.get_feedback_stats(&project_id).await?;
            if json {
                output::print_json(&stats)?;
            } else {
                output::print_feedback_stats(&stats);
            }
        }
    }
    Ok(())
}
