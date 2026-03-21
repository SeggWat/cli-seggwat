//! Output formatting: tables (default) and JSON.

use comfy_table::{Cell, ContentArrangement, Table, presets::UTF8_FULL_CONDENSED};
use serde::Serialize;

use crate::models::{
    Feedback, FeedbackCounts, HelpfulStats, NpsStats, PaginationInfo, Project, ProjectSummary,
    Rating, StarStats, WhoamiResponse, format_rating_value,
};

/// Print any serializable value as pretty JSON.
pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

// ============================================================================
// Projects
// ============================================================================

pub fn print_projects_table(projects: &[Project]) {
    if projects.is_empty() {
        println!("No projects found.");
        return;
    }

    let mut table = new_table();
    table.set_header(vec!["ID", "Name", "Key", "Feedback"]);

    for p in projects {
        table.add_row(vec![
            Cell::new(&p.id),
            Cell::new(&p.name),
            Cell::new(&p.key),
            Cell::new(p.feedback_count),
        ]);
    }

    println!("{table}");
}

pub fn print_project_detail(project: &Project) {
    println!("ID:              {}", project.id);
    println!("Name:            {}", project.name);
    println!("Key:             {}", project.key);
    println!("Description:     {}", project.description);
    println!("Organization:    {}", project.org_id);
    println!("Feedback Count:  {}", project.feedback_count);
    if !project.allowed_origins.is_empty() {
        println!("Allowed Origins: {}", project.allowed_origins.join(", "));
    }
}

pub fn print_project_summary(summary: &ProjectSummary) {
    print_project_detail(&summary.project);
    println!();

    if let Some(feedback) = summary.feedback.as_object() {
        println!("--- Feedback ---");
        if let Some(total) = feedback.get("total") {
            println!("  Total:         {total}");
        }
        if let Some(current) = feedback.get("current_month") {
            println!("  This Month:    {current}");
        }
        if let Some(last) = feedback.get("last_month") {
            println!("  Last Month:    {last}");
        }
        if let Some(by_type) = feedback.get("by_type").and_then(|v| v.as_object()) {
            let parts: Vec<String> = by_type.iter().map(|(k, v)| format!("{k}: {v}")).collect();
            println!("  By Type:       {}", parts.join(", "));
        }
        if let Some(by_status) = feedback.get("by_status").and_then(|v| v.as_object()) {
            let parts: Vec<String> = by_status.iter().map(|(k, v)| format!("{k}: {v}")).collect();
            println!("  By Status:     {}", parts.join(", "));
        }
    }

    if let Some(ratings) = summary.ratings.as_object() {
        println!();
        println!("--- Ratings ---");
        if let Some(helpful) = ratings.get("helpful").and_then(|v| v.as_object()) {
            let total = helpful.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
            let pct = helpful
                .get("percentage")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            println!("  Helpful:       {total} total ({pct:.1}% positive)");
        }
        if let Some(star) = ratings.get("star").and_then(|v| v.as_object()) {
            let total = star.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
            let avg = star.get("average").and_then(|v| v.as_f64()).unwrap_or(0.0);
            println!("  Star:          {total} total (avg {avg:.1})");
        }
        if let Some(nps) = ratings.get("nps").and_then(|v| v.as_object()) {
            let total = nps.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
            let score = nps.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
            println!("  NPS:           {total} total (score {score})");
        }
    }
}

// ============================================================================
// Feedback
// ============================================================================

pub fn print_feedback_table(feedback: &[Feedback], pagination: &PaginationInfo) {
    if feedback.is_empty() {
        println!("No feedback found.");
        return;
    }

    let mut table = new_table();
    table.set_header(vec!["ID", "Type", "Status", "Message", "Created"]);

    for f in feedback {
        let msg = truncate(&f.message, 60);
        let created = format_timestamp(&f.created_at);
        table.add_row(vec![
            Cell::new(&f.id),
            Cell::new(&f.feedback_type),
            Cell::new(&f.status),
            Cell::new(msg),
            Cell::new(created),
        ]);
    }

    println!("{table}");
    print_pagination(pagination);
}

pub fn print_feedback_detail(feedback: &Feedback) {
    println!("ID:              {}", feedback.id);
    println!("Project:         {}", feedback.project_id);
    println!("Type:            {}", feedback.feedback_type);
    println!("Status:          {}", feedback.status);
    println!("Source:          {}", feedback.source);
    println!(
        "Created:         {}",
        format_timestamp(&feedback.created_at)
    );
    println!("Archived:        {}", feedback.archived);
    if let Some(path) = &feedback.path {
        println!("Path:            {path}");
    }
    if let Some(version) = &feedback.version {
        println!("Version:         {version}");
    }
    if let Some(by) = &feedback.submitted_by {
        println!("Submitted By:    {by}");
    }
    if let Some(note) = &feedback.resolution_note {
        println!("Resolution Note: {note}");
    }
    println!();
    println!("Message:");
    println!("{}", feedback.message);
}

pub fn print_feedback_stats(stats: &FeedbackCounts) {
    println!("Total:        {}", stats.total);
    println!("This Month:   {}", stats.current_month);
    println!("Last Month:   {}", stats.last_month);
}

// ============================================================================
// Ratings
// ============================================================================

pub fn print_ratings_table(ratings: &[Rating], pagination: &PaginationInfo) {
    if ratings.is_empty() {
        println!("No ratings found.");
        return;
    }

    let mut table = new_table();
    table.set_header(vec!["ID", "Type", "Value", "Path", "Created"]);

    for r in ratings {
        table.add_row(vec![
            Cell::new(&r.id),
            Cell::new(r.rating_type),
            Cell::new(format_rating_value(&r.value)),
            Cell::new(r.path.as_deref().unwrap_or("-")),
            Cell::new(format_timestamp(&r.created_at)),
        ]);
    }

    println!("{table}");
    print_pagination(pagination);
}

pub fn print_rating_detail(rating: &Rating) {
    println!("ID:           {}", rating.id);
    println!("Project:      {}", rating.project_id);
    println!("Type:         {}", rating.rating_type);
    println!("Value:        {}", format_rating_value(&rating.value));
    println!("Created:      {}", format_timestamp(&rating.created_at));
    println!("Archived:     {}", rating.archived);
    if let Some(path) = &rating.path {
        println!("Path:         {path}");
    }
    if let Some(version) = &rating.version {
        println!("Version:      {version}");
    }
    if let Some(by) = &rating.submitted_by {
        println!("Submitted By: {by}");
    }
}

pub fn print_helpful_stats(stats: &HelpfulStats) {
    println!("--- Helpful Rating Stats ---");
    println!("Total:       {}", stats.total);
    println!("Helpful:     {}", stats.helpful);
    println!("Not Helpful: {}", stats.not_helpful);
    println!("Percentage:  {:.1}%", stats.percentage);

    if stats.total > 0 {
        let bar_len = 30;
        let filled = ((stats.percentage / 100.0) * bar_len as f64) as usize;
        let empty = bar_len - filled;
        println!("             [{}{}]", "#".repeat(filled), "-".repeat(empty));
    }
}

pub fn print_star_stats(stats: &StarStats) {
    println!("--- Star Rating Stats ---");
    println!("Total:   {}", stats.total);
    println!("Average: {:.1}", stats.average);
    println!();

    let max_count = stats.distribution.values().max().copied().unwrap_or(1);
    for star in (1u8..=5).rev() {
        let count = stats.distribution.get(&star).copied().unwrap_or(0);
        let bar_len = if max_count > 0 {
            ((count as f64 / max_count as f64) * 20.0) as usize
        } else {
            0
        };
        println!("  {} star: {} {}", star, "#".repeat(bar_len), count);
    }
}

pub fn print_nps_stats(stats: &NpsStats) {
    println!("--- NPS Stats ---");
    println!("Total:      {}", stats.total);
    println!("NPS Score:  {}", stats.score);
    println!("Promoters:  {} (9-10)", stats.promoters);
    println!("Passives:   {} (7-8)", stats.passives);
    println!("Detractors: {} (0-6)", stats.detractors);
}

// ============================================================================
// User Info
// ============================================================================

pub fn print_whoami(info: &WhoamiResponse) {
    println!("User ID:      {}", info.user_id);
    println!("Email:        {}", info.email);
    if let Some(name) = &info.display_name {
        println!("Name:         {name}");
    }
    println!("Organization: {}", info.org_id);
    if let Some(name) = &info.org_name {
        println!("Org Name:     {name}");
    }
    println!("Role:         {}", info.role);
}

// ============================================================================
// Helpers
// ============================================================================

fn new_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL_CONDENSED)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

fn print_pagination(p: &PaginationInfo) {
    println!("Page {} of {} ({} total)", p.page, p.total_pages, p.total);
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{truncated}...")
    }
}

fn format_timestamp(ts: &str) -> String {
    // Try to parse ISO 8601 and format as shorter date
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| ts.to_string())
}
