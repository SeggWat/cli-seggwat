use anyhow::Result;

use crate::cli::{RatingCommand, RatingTypeArg};
use crate::client::SeggwatClient;
use crate::output;

pub async fn execute(client: &SeggwatClient, command: RatingCommand, json: bool) -> Result<()> {
    match command {
        RatingCommand::List {
            project_id,
            page,
            limit,
            r#type,
            path,
        } => {
            let type_str = r#type.as_ref().map(|t| t.to_string());
            let resp = client
                .list_ratings(
                    &project_id,
                    page,
                    limit,
                    type_str.as_deref(),
                    path.as_deref(),
                )
                .await?;
            if json {
                output::print_json(&resp)?;
            } else {
                output::print_ratings_table(&resp.ratings, &resp.pagination);
            }
        }
        RatingCommand::Get {
            project_id,
            rating_id,
        } => {
            let rating = client.get_rating(&project_id, &rating_id).await?;
            if json {
                output::print_json(&rating)?;
            } else {
                output::print_rating_detail(&rating);
            }
        }
        RatingCommand::Delete {
            project_id,
            rating_id,
        } => {
            client.delete_rating(&project_id, &rating_id).await?;
            if json {
                output::print_json(&serde_json::json!({"deleted": rating_id}))?;
            } else {
                println!("Rating deleted: {rating_id}");
            }
        }
        RatingCommand::Stats { project_id, r#type } => match r#type {
            RatingTypeArg::Helpful => {
                let stats = client.get_helpful_stats(&project_id).await?;
                if json {
                    output::print_json(&stats)?;
                } else {
                    output::print_helpful_stats(&stats);
                }
            }
            RatingTypeArg::Star => {
                let stats = client.get_star_stats(&project_id).await?;
                if json {
                    output::print_json(&stats)?;
                } else {
                    output::print_star_stats(&stats);
                }
            }
            RatingTypeArg::Nps => {
                let stats = client.get_nps_stats(&project_id).await?;
                if json {
                    output::print_json(&stats)?;
                } else {
                    output::print_nps_stats(&stats);
                }
            }
        },
    }
    Ok(())
}
