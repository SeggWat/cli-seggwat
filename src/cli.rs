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
    /// Manage review tasks (extension / on-page review mode)
    #[command(alias = "rev")]
    Review {
        #[command(subcommand)]
        command: ReviewCommand,
    },
    /// Log in with your SeggWat account (opens browser)
    Login,
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
// Review Task Commands
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum ReviewCommand {
    /// List review tasks for a project
    #[command(alias = "ls")]
    List {
        /// Project ID
        project_id: String,

        /// Filter by status
        #[arg(long)]
        status: Option<ReviewTaskStatusArg>,
    },
    /// Get a review task
    Get {
        /// Project ID
        project_id: String,
        /// Review task ID
        task_id: String,
    },
    /// List the annotations (on-page pins) left on a review task
    Comments {
        /// Project ID
        project_id: String,
        /// Review task ID
        task_id: String,
    },
    /// Create a review task
    Create {
        /// Project ID
        project_id: String,

        /// Task title (what to review)
        #[arg(short, long)]
        title: String,

        /// Instructions for the reviewer
        #[arg(short, long)]
        description: Option<String>,

        /// Page URL the task refers to
        #[arg(long)]
        url: Option<String>,
    },
    /// Update a review task (partial; pass "" to clear description/url)
    Update {
        /// Project ID
        project_id: String,
        /// Review task ID
        task_id: String,

        /// Updated title
        #[arg(short, long)]
        title: Option<String>,

        /// Updated description ("" clears it)
        #[arg(short, long)]
        description: Option<String>,

        /// Updated URL ("" clears it)
        #[arg(long)]
        url: Option<String>,

        /// Updated status
        #[arg(long)]
        status: Option<ReviewTaskStatusArg>,
    },
    /// Delete a review task (its comments survive in feedback)
    #[command(alias = "rm")]
    Delete {
        /// Project ID
        project_id: String,
        /// Review task ID
        task_id: String,
    },
    /// Resolve an annotation — mark a pin fixed
    Resolve {
        /// Project ID
        project_id: String,
        /// Annotation (feedback) ID from `review comments`
        feedback_id: String,
    },
    /// Reopen a resolved annotation
    Reopen {
        /// Project ID
        project_id: String,
        /// Annotation (feedback) ID from `review comments`
        feedback_id: String,
    },
    /// Serve a local proxy so your dev app can drop review pins on this project
    ///
    /// Runs a tiny localhost server that forwards the review widget's API calls
    /// to SeggWat with a trusted Origin, so you can annotate a local dev app
    /// without adding its origin to the project's allowed-origins list.
    Serve {
        /// Project ID
        project_id: String,

        /// Port to listen on
        #[arg(long, default_value = "7777")]
        port: u16,

        /// Origin to forward (defaults to the project's first allowed origin)
        #[arg(long)]
        origin: Option<String>,
    },
    /// Account-free local review loop backed by ./.review/ JSON (no SeggWat needed)
    Local {
        #[command(subcommand)]
        command: LocalReviewCommand,
    },
}

// ============================================================================
// Local Review Commands (no account / no project — storage in ./.review/)
// ============================================================================

#[derive(Subcommand, Debug)]
pub enum LocalReviewCommand {
    /// Serve the review widget locally, storing pins in ./.review/
    Serve {
        /// Port to listen on
        #[arg(long, default_value = "7777")]
        port: u16,
    },
    /// List local review tasks
    #[command(alias = "ls")]
    List,
    /// Get a local review task
    Get {
        /// Review task ID
        task_id: String,
    },
    /// List the annotations (on-page pins) on a local review task
    Comments {
        /// Review task ID
        task_id: String,
    },
    /// Create a local review task
    Create {
        /// Task title (what to review)
        #[arg(short, long)]
        title: String,

        /// Instructions for the reviewer
        #[arg(short, long)]
        description: Option<String>,

        /// Page URL the task refers to
        #[arg(long)]
        url: Option<String>,
    },
    /// Update a local review task (partial; pass "" to clear description/url)
    Update {
        /// Review task ID
        task_id: String,

        /// Updated title
        #[arg(short, long)]
        title: Option<String>,

        /// Updated description ("" clears it)
        #[arg(short, long)]
        description: Option<String>,

        /// Updated URL ("" clears it)
        #[arg(long)]
        url: Option<String>,

        /// Updated status
        #[arg(long)]
        status: Option<ReviewTaskStatusArg>,
    },
    /// Delete a local review task
    #[command(alias = "rm")]
    Delete {
        /// Review task ID
        task_id: String,
    },
    /// Resolve a local annotation — mark a pin fixed
    Resolve {
        /// Annotation ID from `review local comments`
        feedback_id: String,
    },
    /// Reopen a resolved local annotation
    Reopen {
        /// Annotation ID from `review local comments`
        feedback_id: String,
    },
}

// ============================================================================
// Value Enums for CLI arguments
// ============================================================================

#[derive(Debug, Clone, ValueEnum)]
pub enum ReviewTaskStatusArg {
    Open,
    Done,
}

impl std::fmt::Display for ReviewTaskStatusArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Done => write!(f, "done"),
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum FeedbackStatusArg {
    New,
    Active,
    Hold,
    Closed,
    Resolved,
}

impl std::fmt::Display for FeedbackStatusArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::New => write!(f, "New"),
            Self::Active => write!(f, "Active"),
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
