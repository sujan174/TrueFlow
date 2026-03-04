use clap::{Parser, Subcommand};

/// TrueFlow — Secure API Gateway for AI Agents
#[derive(Parser)]
#[command(name = "trueflow", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the gateway server
    Serve {
        /// Port to bind
        #[arg(short, long, default_value = "8443")]
        port: u16,
    },

    /// Manage virtual tokens
    Token {
        #[command(subcommand)]
        command: TokenCommands,
    },

    /// Manage stored credentials
    Credential {
        #[command(subcommand)]
        command: CredentialCommands,
    },

    /// Manage HITL approvals
    Approval {
        #[command(subcommand)]
        command: ApprovalCommands,
    },

    /// Manage policies
    Policy {
        #[command(subcommand)]
        command: PolicyCommands,
    },
}

#[derive(Subcommand)]
pub enum TokenCommands {
    /// Create a new virtual token
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        credential: String,
        #[arg(long)]
        upstream: String,
        #[arg(long)]
        project_id: Option<String>,
        #[arg(long, value_delimiter = ',')]
        policy_ids: Option<Vec<String>>,
    },
    /// List tokens for a project
    List {
        #[arg(long)]
        project_id: String,
    },
    /// Revoke a token
    Revoke {
        #[arg(long)]
        token_id: String,
    },
}

#[derive(Subcommand)]
pub enum CredentialCommands {
    /// Store a new API credential in the vault
    Add {
        #[arg(long)]
        name: String,
        #[arg(long)]
        provider: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        project_id: Option<String>,
        /// Auth injection mode: bearer, basic, header, query
        #[arg(long, default_value = "bearer")]
        mode: String,
        /// Header name (or query param name) for injection
        #[arg(long, default_value = "Authorization")]
        header: String,
    },
    /// List stored credentials (metadata only)
    List {
        #[arg(long)]
        project_id: String,
    },
}

#[derive(Subcommand)]
pub enum ApprovalCommands {
    /// List pending approval requests
    List {
        #[arg(short, long)]
        project_id: Option<String>,
    },
    /// Approve a pending request
    Approve { request_id: String },
    /// Reject a pending request
    Reject { request_id: String },
}

#[derive(Subcommand)]
pub enum PolicyCommands {
    /// Create a new policy
    Create {
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "enforce")]
        mode: String,
        #[arg(long, default_value = "pre")]
        phase: String,
        #[arg(long)]
        project_id: Option<String>,

        // Rules configuration
        #[arg(long, help = "Rate limit (e.g., '10/min')")]
        rate_limit: Option<String>,

        #[arg(long, help = "HITL timeout (e.g., '10m')")]
        hitl_timeout: Option<String>,
        #[arg(long, help = "HITL fallback action (approve/reject)")]
        hitl_fallback: Option<String>,
    },
    /// List policies
    List {
        #[arg(long)]
        project_id: String,
    },
    /// Delete a policy
    Delete {
        #[arg(long)]
        id: String,
    },
}
