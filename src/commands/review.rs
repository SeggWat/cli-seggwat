use anyhow::{Result, bail};

use crate::cli::ReviewCommand;
use crate::client::SeggwatClient;
use crate::models::{
    FeedbackStatus, FeedbackUpdateRequest, ReviewTaskCreateRequest, ReviewTaskUpdateRequest,
};
use crate::output;

pub async fn execute(client: &SeggwatClient, command: ReviewCommand, json: bool) -> Result<()> {
    match command {
        ReviewCommand::List { project_id, status } => {
            let status_str = status.as_ref().map(|s| s.to_string());
            let tasks = client
                .list_review_tasks(&project_id, status_str.as_deref())
                .await?;
            if json {
                output::print_json(&tasks)?;
            } else {
                output::print_review_task_table(&tasks);
            }
        }
        ReviewCommand::Get {
            project_id,
            task_id,
        } => {
            let task = client.get_review_task(&project_id, &task_id).await?;
            if json {
                output::print_json(&task)?;
            } else {
                output::print_review_task_detail(&task);
            }
        }
        ReviewCommand::Comments {
            project_id,
            task_id,
        } => {
            let comments = client
                .list_review_task_comments(&project_id, &task_id)
                .await?;
            if json {
                output::print_json(&comments)?;
            } else {
                output::print_review_comments_table(&comments);
            }
        }
        ReviewCommand::Create {
            project_id,
            title,
            description,
            url,
        } => {
            let body = ReviewTaskCreateRequest {
                title,
                description,
                url,
            };
            let task = client.create_review_task(&project_id, &body).await?;
            if json {
                output::print_json(&task)?;
            } else {
                println!("Review task created: {}", task.id);
                output::print_review_task_detail(&task);
            }
        }
        ReviewCommand::Update {
            project_id,
            task_id,
            title,
            description,
            url,
            status,
        } => {
            let body = ReviewTaskUpdateRequest {
                title,
                description,
                url,
                status: status.as_ref().map(|s| s.to_string()),
            };
            if body.title.is_none()
                && body.description.is_none()
                && body.url.is_none()
                && body.status.is_none()
            {
                bail!(
                    "Nothing to update. Provide at least one of --title, --description, --url, or --status."
                );
            }
            let task = client
                .update_review_task(&project_id, &task_id, &body)
                .await?;
            if json {
                output::print_json(&task)?;
            } else {
                println!("Review task updated: {}", task.id);
                output::print_review_task_detail(&task);
            }
        }
        ReviewCommand::Delete {
            project_id,
            task_id,
        } => {
            client.delete_review_task(&project_id, &task_id).await?;
            if json {
                output::print_json(&serde_json::json!({"deleted": task_id}))?;
            } else {
                println!("Review task deleted: {task_id}");
            }
        }
        ReviewCommand::Resolve {
            project_id,
            feedback_id,
        } => {
            set_annotation_resolved(client, &project_id, &feedback_id, true, json).await?;
        }
        ReviewCommand::Reopen {
            project_id,
            feedback_id,
        } => {
            set_annotation_resolved(client, &project_id, &feedback_id, false, json).await?;
        }
        ReviewCommand::Serve {
            project_id,
            port,
            origin,
        } => {
            crate::serve::run(client, &project_id, port, origin).await?;
        }
        ReviewCommand::Local { .. } => {
            unreachable!("`review local` runs before auth (see main.rs)")
        }
    }
    Ok(())
}

/// Resolve/reopen a review annotation. Annotations are feedback rows, so this
/// flips their status (Resolved / New) via the feedback update endpoint —
/// mirroring the widget's resolve.
async fn set_annotation_resolved(
    client: &SeggwatClient,
    project_id: &str,
    feedback_id: &str,
    resolved: bool,
    json: bool,
) -> Result<()> {
    let body = FeedbackUpdateRequest {
        message: None,
        feedback_type: None,
        status: Some(if resolved {
            FeedbackStatus::Resolved
        } else {
            FeedbackStatus::New
        }),
        resolution_note: None,
    };
    let feedback = client
        .update_feedback(project_id, feedback_id, &body)
        .await?;
    if json {
        output::print_json(&feedback)?;
    } else {
        println!(
            "Annotation {}: {}",
            if resolved { "resolved" } else { "reopened" },
            feedback.id
        );
    }
    Ok(())
}
