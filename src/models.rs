//! API response and request types mirroring the SeggWat REST API.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ============================================================================
// Domain Types (inlined from seggwat-core for standalone builds)
// ============================================================================

/// Enum of feedback types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FeedbackType {
    Bug,
    Feature,
    Praise,
    Question,
    Improvement,
    Other,
}

impl std::fmt::Display for FeedbackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedbackType::Bug => write!(f, "Bug"),
            FeedbackType::Feature => write!(f, "Feature"),
            FeedbackType::Praise => write!(f, "Praise"),
            FeedbackType::Question => write!(f, "Question"),
            FeedbackType::Improvement => write!(f, "Improvement"),
            FeedbackType::Other => write!(f, "Other"),
        }
    }
}

/// Enum of feedback statuses
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FeedbackStatus {
    New,
    Active,
    Assigned,
    Hold,
    Closed,
    Resolved,
}

impl std::fmt::Display for FeedbackStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedbackStatus::New => write!(f, "New"),
            FeedbackStatus::Active => write!(f, "Active"),
            FeedbackStatus::Assigned => write!(f, "Assigned"),
            FeedbackStatus::Hold => write!(f, "Hold"),
            FeedbackStatus::Closed => write!(f, "Closed"),
            FeedbackStatus::Resolved => write!(f, "Resolved"),
        }
    }
}

/// Enum of feedback sources
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub enum FeedbackSource {
    #[default]
    Widget,
    Manual,
    Mintlify,
    Stripe,
    Polar,
    SocialShare,
}

impl std::fmt::Display for FeedbackSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedbackSource::Widget => write!(f, "Feedback Button"),
            FeedbackSource::Manual => write!(f, "Manual"),
            FeedbackSource::Mintlify => write!(f, "Mintlify"),
            FeedbackSource::Stripe => write!(f, "Stripe"),
            FeedbackSource::Polar => write!(f, "Polar"),
            FeedbackSource::SocialShare => write!(f, "Social Share"),
        }
    }
}

/// The type of rating
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RatingType {
    Helpful,
    Star,
    Nps,
}

impl std::fmt::Display for RatingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RatingType::Helpful => write!(f, "helpful"),
            RatingType::Star => write!(f, "star"),
            RatingType::Nps => write!(f, "nps"),
        }
    }
}

/// Polymorphic rating value based on type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RatingValue {
    Helpful {
        value: bool,
    },
    Star {
        value: u8,
        #[serde(default = "default_max_stars")]
        max_stars: u8,
    },
    Nps {
        value: u8,
    },
}

fn default_max_stars() -> u8 {
    5
}

/// Format a RatingValue for display.
pub fn format_rating_value(rv: &RatingValue) -> String {
    match rv {
        RatingValue::Helpful { value } => {
            if *value { "Helpful" } else { "Not Helpful" }.to_string()
        }
        RatingValue::Star { value, max_stars } => format!("{value}/{max_stars}"),
        RatingValue::Nps { value } => format!("{value}/10"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackCounts {
    pub total: u64,
    pub current_month: u64,
    pub last_month: u64,
}

/// Statistics for helpful (binary) ratings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HelpfulStats {
    pub total: u64,
    pub helpful: u64,
    pub not_helpful: u64,
    pub percentage: f64,
}

/// Statistics for star ratings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StarStats {
    pub total: u64,
    pub average: f64,
    pub distribution: HashMap<u8, u64>,
}

/// Statistics for NPS (Net Promoter Score) ratings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NpsStats {
    pub total: u64,
    pub score: i32,
    pub promoters: u64,
    pub passives: u64,
    pub detractors: u64,
    pub distribution: HashMap<u8, u64>,
}

// ============================================================================
// Core Entities
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feedback {
    pub id: String,
    pub project_id: String,
    pub message: String,
    pub status: FeedbackStatus,
    #[serde(rename = "type")]
    pub feedback_type: FeedbackType,
    pub source: FeedbackSource,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub archived: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub resolution_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub key: String,
    pub name: String,
    pub description: String,
    pub allowed_origins: Vec<String>,
    pub org_id: String,
    pub feedback_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rating {
    pub id: String,
    pub project_id: String,
    pub rating_type: RatingType,
    pub value: RatingValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_by: Option<String>,
    pub archived: bool,
    pub created_at: String,
}

// ============================================================================
// Project Summary
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project: Project,
    pub feedback: serde_json::Value,
    #[serde(default)]
    pub ratings: serde_json::Value,
}

// ============================================================================
// Pagination
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
}

// ============================================================================
// Request Bodies
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct FeedbackCreateRequest {
    pub message: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub feedback_type: Option<FeedbackType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedbackUpdateRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub feedback_type: Option<FeedbackType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<FeedbackStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_note: Option<String>,
}

// ============================================================================
// Response Bodies
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectListResponse {
    pub projects: Vec<Project>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackListResponse {
    pub feedback: Vec<Feedback>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatingListResponse {
    pub ratings: Vec<Rating>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiError {
    #[allow(dead_code)]
    pub error: String,
    pub message: String,
}

// ============================================================================
// User Info
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoamiResponse {
    pub user_id: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub org_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_name: Option<String>,
    pub role: String,
}
