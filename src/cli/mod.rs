use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Default, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Json,
    Human,
    Markdown,
}

#[derive(Parser, Debug)]
#[command(name = "cantrip", about = "AI-powered business operating system for solo founders", version)]
pub struct Cli {
    /// Output format
    #[arg(long, value_enum, default_value = "json", global = true)]
    pub format: OutputFormat,

    /// Project slug
    #[arg(long, global = true)]
    pub project: Option<String>,

    /// Team slug [default: personal]
    #[arg(long, global = true)]
    pub team: Option<String>,

    /// Verbose output
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Bootstrap a new project
    Init {
        /// Project name (required)
        #[arg(long)]
        name: String,
        /// Project description (required)
        #[arg(long)]
        description: String,
        /// Path to a product brief file (optional)
        #[arg(long)]
        brief: Option<String>,
    },

    /// Where am I? Show project overview or drill into entity types
    Snapshot {
        /// Entity type to inspect [possible values: icps, pain-points, value-props, experiments, channels, competitors, contacts]
        entity_type: Option<String>,
        /// Entity ID for detail view
        id: Option<String>,
    },

    /// What needs my judgment? Review inferred items and escalations
    Review {
        /// Action to take
        action: Option<ReviewAction>,
        /// Entity or escalation ID
        id: Option<String>,
        /// Resolution message (used with resolve action)
        #[arg(long)]
        resolution: Option<String>,
    },

    /// What should I do next? Gap analysis and opportunities
    Next {
        /// Mode: 'prompt' generates a pasteable LLM prompt, 'run' spawns a background agent
        mode: Option<NextMode>,
        /// Opportunity ID (required for prompt/run)
        id: Option<String>,
    },

    /// View audit trail of all actions
    History {
        /// Filter by event type (e.g. init, entity_created, review)
        #[arg(long, name = "type")]
        event_type: Option<String>,
        /// Filter by entity type (e.g. icp, pain_point)
        #[arg(long)]
        entity: Option<String>,
        /// Only events after this ISO date
        #[arg(long)]
        since: Option<String>,
        /// Maximum number of events to return
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Ideal customer profile operations
    Icp {
        #[command(subcommand)]
        action: EntityAction,
    },

    /// Pain point operations
    PainPoint {
        #[command(subcommand)]
        action: EntityAction,
    },

    /// Value proposition operations
    ValueProp {
        #[command(subcommand)]
        action: EntityAction,
    },

    /// Experiment operations
    Experiment {
        #[command(subcommand)]
        action: EntityAction,
    },

    /// Channel operations
    Channel {
        #[command(subcommand)]
        action: EntityAction,
    },

    /// Competitor operations
    Competitor {
        #[command(subcommand)]
        action: EntityAction,
    },

    /// Contact operations
    Contact {
        #[command(subcommand)]
        action: EntityAction,
    },

    /// Manage projects
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },

    /// Manage users
    User {
        #[command(subcommand)]
        action: UserAction,
    },

    /// Manage API keys
    Apikey {
        #[command(subcommand)]
        action: ApikeyAction,
    },

    /// Manage credits and usage metering
    Meter {
        #[command(subcommand)]
        action: Option<MeterAction>,
    },

    /// Authenticate with the Cantrip daemon
    Login {
        /// API key (interactive prompt if omitted)
        #[arg(long)]
        key: Option<String>,
        /// API URL (default: https://api.cantrip.ai)
        #[arg(long)]
        url: Option<String>,
    },

    /// Clear stored credentials
    Logout,

    /// Show current identity
    Whoami,

    /// [internal] Tick the outer loop
    #[command(name = "_tick", hide = true)]
    Tick,

    /// [internal] Run an inner loop by ID
    #[command(name = "_loop", hide = true)]
    Loop { id: String },
}

#[derive(Subcommand, Debug, Clone)]
pub enum EntityAction {
    /// List all entities of this type
    List,
    /// Show a single entity by ID
    Show { id: String },
    /// Add a new entity (pass fields as --field key=value)
    Add {
        /// Entity name (required for most types)
        #[arg(long)]
        name: Option<String>,
        /// Entity description
        #[arg(long)]
        description: Option<String>,
        /// Additional fields as key=value pairs
        #[arg(long = "field", value_name = "KEY=VALUE")]
        fields: Vec<String>,
    },
    /// Edit an existing entity (pass fields as --field key=value)
    Edit {
        id: String,
        /// Update name
        #[arg(long)]
        name: Option<String>,
        /// Update description
        #[arg(long)]
        description: Option<String>,
        /// Additional fields as key=value pairs
        #[arg(long = "field", value_name = "KEY=VALUE")]
        fields: Vec<String>,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ReviewAction {
    Accept,
    Reject,
    Resolve,
    Dismiss,
}

impl std::fmt::Display for ReviewAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReviewAction::Accept => write!(f, "accept"),
            ReviewAction::Reject => write!(f, "reject"),
            ReviewAction::Resolve => write!(f, "resolve"),
            ReviewAction::Dismiss => write!(f, "dismiss"),
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum NextMode {
    /// Generate a pasteable LLM prompt for an opportunity
    Prompt,
    /// Spawn a background agent to work on an opportunity
    Run,
}

#[derive(Subcommand, Debug, Clone)]
pub enum UserAction {
    /// Create a new user with their own team
    Create {
        /// User email
        #[arg(long)]
        email: String,
        /// User display name
        #[arg(long)]
        name: String,
        /// Team name (globally unique)
        #[arg(long)]
        team: String,
        /// Team display name (defaults to team name)
        #[arg(long)]
        team_display: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ApikeyAction {
    /// Create an API key for a user
    Create {
        /// User ID (optional when authenticated)
        #[arg(long)]
        user: Option<String>,
        /// Key name (for identification)
        #[arg(long, default_value = "default")]
        name: String,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum MeterAction {
    /// Show credit balance (default)
    Balance,
    /// Show credit transaction history
    History {
        /// Maximum number of entries to show
        #[arg(long, default_value = "20")]
        limit: u32,
    },
    /// Show available credit packs and pricing
    Tiers,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ProjectAction {
    /// List all projects
    List,
    /// Create a new project (alias for init)
    Create {
        /// Project name (required)
        #[arg(long)]
        name: String,
        /// Project description (required)
        #[arg(long)]
        description: String,
        /// Path to a product brief file (optional)
        #[arg(long)]
        brief: Option<String>,
    },
    /// Update a project's name or description
    Update {
        /// Project slug (uses default project if omitted)
        slug: Option<String>,
        /// New display name
        #[arg(long)]
        name: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a project and all its data
    Delete {
        /// Project slug (uses default project if omitted)
        slug: Option<String>,
    },
    /// Set the default project for future commands
    Switch {
        /// Project slug to switch to
        slug: String,
    },
}
