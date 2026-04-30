//! Clap CLI definitions for the SeggWat CLI.

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "seggwat",
    about = "CLI for the SeggWat feedback platform",
    long_about = "Manage feedback, projects, and ratings from the terminal.\n\n\
                  Authenticate with `seggwat login` (opens browser) or an API key.",
    version
)]
pub struct Cli {
    /// SeggWat API base URL
    #[arg(
        long,
        env = "SEGGWAT_API_URL",
        default_value = "https://seggwat.com",
        global = true
    )]
    pub api_url: String,

    /// API key for authentication
    #[arg(long, env = "SEGGWAT_API_KEY", global = true)]
    pub api_key: Option<String>,

    /// Output as JSON instead of formatted tables
    #[arg(long, global = true)]
    pub json: bool,

    /// Enable verbose debug logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Manage projects
    #[command(alias = "p")]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Manage feedback
    #[command(alias = "fb")]
    Feedback {
        #[command(subcommand)]
        command: FeedbackCommand,
    },
    /// Manage ratings
    #[command(alias = "r")]
    Rating {
        #[command(subcommand)]
        command: RatingCommand,
    },
    /// Log in with your SeggWat account (opens browser)
    Login {
        /// Zitadel domain (auto-detected for seggwat.com, required for self-hosted)
        #[arg(long, env = "SEGGWAT_ZITADEL_DOMAIN")]
        zitadel_domain: Option<String>,

        /// OAuth client ID (auto-detected for seggwat.com, required for self-hosted)
        #[arg(long, env = "SEGGWAT_CLIENT_ID")]
        client_id: Option<String>,
    },
    /// Log out and clear cached tokens
    Logout,
    /// Show the currently authenticated user
    Whoami,
    /// Launch the interactive terminal UI (TUI)
    Tui {
        /// Jump straight to this project's feedback (skips project picker)
        #[arg(long)]
        project_id: Option<String>,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
}

// ============================================================================
// Project Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// List all projects
    #[command(alias = "ls")]
    List,
    /// Get project details
    Get {
        /// Project ID
        project_id: String,
    },
    /// Get project summary with stats
    Summary {
        /// Project ID
        project_id: String,
    },
}

// ============================================================================
// Feedback Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum FeedbackCommand {
    /// List feedback for a project
    #[command(alias = "ls")]
    List {
        /// Project ID
        project_id: String,

        /// Page number (1-indexed)
        #[arg(long, default_value = "1")]
        page: u64,

        /// Items per page (max 100)
        #[arg(long, default_value = "20")]
        limit: u64,

        /// Filter by status
        #[arg(long)]
        status: Option<FeedbackStatusArg>,

        /// Filter by type
        #[arg(long)]
        r#type: Option<FeedbackTypeArg>,

        /// Search message content
        #[arg(short, long)]
        search: Option<String>,
    },
    /// Get a feedback item
    Get {
        /// Project ID
        project_id: String,
        /// Feedback ID
        feedback_id: String,
    },
    /// Create a new feedback item
    Create {
        /// Project ID
        project_id: String,

        /// Feedback message
        #[arg(short, long)]
        message: String,

        /// Feedback type
        #[arg(long)]
        r#type: Option<FeedbackTypeArg>,

        /// URL path context
        #[arg(long)]
        path: Option<String>,

        /// Application version
        #[arg(long)]
        version: Option<String>,
    },
    /// Update a feedback item
    Update {
        /// Project ID
        project_id: String,
        /// Feedback ID
        feedback_id: String,

        /// Updated message
        #[arg(short, long)]
        message: Option<String>,

        /// Updated type
        #[arg(long)]
        r#type: Option<FeedbackTypeArg>,

        /// Updated status
        #[arg(long)]
        status: Option<FeedbackStatusArg>,

        /// Resolution note
        #[arg(long)]
        resolution_note: Option<String>,
    },
    /// Delete a feedback item
    #[command(alias = "rm")]
    Delete {
        /// Project ID
        project_id: String,
        /// Feedback ID
        feedback_id: String,
    },
    /// Show feedback statistics
    Stats {
        /// Project ID
        project_id: String,
    },
}

// ============================================================================
// Rating Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum RatingCommand {
    /// List ratings for a project
    #[command(alias = "ls")]
    List {
        /// Project ID
        project_id: String,

        /// Page number (1-indexed)
        #[arg(long, default_value = "1")]
        page: u64,

        /// Items per page (max 100)
        #[arg(long, default_value = "20")]
        limit: u64,

        /// Filter by rating type
        #[arg(long)]
        r#type: Option<RatingTypeArg>,

        /// Filter by path
        #[arg(long)]
        path: Option<String>,
    },
    /// Get a rating item
    Get {
        /// Project ID
        project_id: String,
        /// Rating ID
        rating_id: String,
    },
    /// Delete a rating
    #[command(alias = "rm")]
    Delete {
        /// Project ID
        project_id: String,
        /// Rating ID
        rating_id: String,
    },
    /// Show rating statistics
    Stats {
        /// Project ID
        project_id: String,

        /// Rating type for stats
        #[arg(long, default_value = "helpful")]
        r#type: RatingTypeArg,
    },
}

// ============================================================================
// Value Enums for CLI arguments
// ============================================================================

#[derive(Debug, Clone, ValueEnum)]
pub enum FeedbackStatusArg {
    New,
    Active,
    Assigned,
    Hold,
    Closed,
    Resolved,
}

impl std::fmt::Display for FeedbackStatusArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New => write!(f, "New"),
            Self::Active => write!(f, "Active"),
            Self::Assigned => write!(f, "Assigned"),
            Self::Hold => write!(f, "Hold"),
            Self::Closed => write!(f, "Closed"),
            Self::Resolved => write!(f, "Resolved"),
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum FeedbackTypeArg {
    Bug,
    Feature,
    Praise,
    Question,
    Improvement,
    Other,
}

impl FeedbackTypeArg {
    pub fn to_model(&self) -> crate::models::FeedbackType {
        match self {
            Self::Bug => crate::models::FeedbackType::Bug,
            Self::Feature => crate::models::FeedbackType::Feature,
            Self::Praise => crate::models::FeedbackType::Praise,
            Self::Question => crate::models::FeedbackType::Question,
            Self::Improvement => crate::models::FeedbackType::Improvement,
            Self::Other => crate::models::FeedbackType::Other,
        }
    }
}

impl std::fmt::Display for FeedbackTypeArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bug => write!(f, "Bug"),
            Self::Feature => write!(f, "Feature"),
            Self::Praise => write!(f, "Praise"),
            Self::Question => write!(f, "Question"),
            Self::Improvement => write!(f, "Improvement"),
            Self::Other => write!(f, "Other"),
        }
    }
}

impl FeedbackStatusArg {
    pub fn to_model(&self) -> crate::models::FeedbackStatus {
        match self {
            Self::New => crate::models::FeedbackStatus::New,
            Self::Active => crate::models::FeedbackStatus::Active,
            Self::Assigned => crate::models::FeedbackStatus::Assigned,
            Self::Hold => crate::models::FeedbackStatus::Hold,
            Self::Closed => crate::models::FeedbackStatus::Closed,
            Self::Resolved => crate::models::FeedbackStatus::Resolved,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum RatingTypeArg {
    Helpful,
    Star,
    Nps,
}

impl std::fmt::Display for RatingTypeArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Helpful => write!(f, "helpful"),
            Self::Star => write!(f, "star"),
            Self::Nps => write!(f, "nps"),
        }
    }
}
