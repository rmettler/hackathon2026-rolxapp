use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
pub enum LoginFlow {
    /// Native app OAuth 2.0 flow with PKCE (default unless specified)
    NativeApp,
    /// Device authorization flow (for headless/SSH environments)
    Device,
}

#[derive(Debug, Parser)]
#[command(name = "rolxapp", about = "RolX time tracking CLI")]
pub struct Cli {
    /// Override the server URL from config
    #[arg(long, global = true, env = "ROLX_SERVER")]
    pub server: Option<String>,

    /// Increase logging verbosity (-v for debug, -vv for trace; can also be controlled via RUST_LOG)
    #[arg(long, short = 'v', global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Sign in with Google and store session
    Login {
        /// OAuth flow to use (default: NativeApp)
        #[command(subcommand)]
        flow: Option<LoginFlow>,
    },
    /// Delete stored session
    Logout,
    /// Print current user from stored session
    Whoami,
    /// Show work entries for a day (default: today)
    Records {
        /// Date in ISO format (yyyy-MM-dd)
        date: Option<String>,
    },
    /// Inspect the config file
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Record work events locally and instantly, even offline (upload with `sync`)
    Stamp {
        #[command(subcommand)]
        command: StampCommand,
    },
    /// Upload stamped entries to the server and refresh the activity list
    Sync,
    /// Search the cached activities, favourites and recently used first (refreshed by `sync`)
    Activities {
        /// Words to search for; every word must match number, name, customer or project
        words: Vec<String>,
        /// Only show favourites
        #[arg(short, long)]
        favourites: bool,
        /// Only show recently used activities
        #[arg(short, long)]
        recent: bool,
        /// First download the full list from the server (does not upload anything)
        #[arg(long)]
        refresh: bool,
    },
    /// Add or remove a favourite activity (on the server)
    Favourite {
        /// Activity id, number or words that match exactly one activity
        #[arg(required = true)]
        activity: Vec<String>,
        /// Remove instead of add
        #[arg(long)]
        remove: bool,
    },
    /// Change or delete entries of a day on the server
    Entry {
        #[command(subcommand)]
        command: EntryCommand,
    },
    /// Show the week: booked time per day, the target and what is left
    Week {
        /// Any day of the week (yyyy-MM-dd), default today
        date: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum EntryCommand {
    /// Change the activity, comment or times of an entry (numbers as shown by `records`)
    Edit {
        /// Entry number
        number: usize,
        /// Day of the entry (yyyy-MM-dd), default today
        #[arg(short, long)]
        date: Option<String>,
        /// New activity: id, number or words that match exactly one activity
        #[arg(short, long)]
        activity: Option<String>,
        /// New work report; an empty text removes it
        #[arg(short, long)]
        comment: Option<String>,
        /// New start time (HH:MM)
        #[arg(long)]
        begin: Option<String>,
        /// New end time (HH:MM)
        #[arg(long)]
        end: Option<String>,
        /// New break in minutes (0 removes it); start and end stay as they are
        #[arg(long = "break")]
        pause: Option<u32>,
        /// Resolve the conflicts this change causes
        #[arg(long, value_enum)]
        fix: Option<FixKind>,
        /// Save even though the entry now overlaps another
        #[arg(long)]
        force: bool,
    },
    /// Book an entry for a time that was not stamped (e.g. a forgotten stamp-in)
    Add {
        /// Activity: id, number or words that match exactly one activity
        #[arg(short, long, required = true)]
        activity: String,
        /// Start time (HH:MM); default: when the latest entry of the day ends
        #[arg(long)]
        begin: Option<String>,
        /// End time (HH:MM)
        #[arg(long, required = true)]
        end: String,
        /// Break in minutes within the entry
        #[arg(long = "break")]
        pause: Option<u32>,
        /// Work report
        #[arg(short, long)]
        comment: Option<String>,
        /// Day to book on (yyyy-MM-dd), default today
        #[arg(short, long)]
        date: Option<String>,
        /// Resolve the conflicts this entry causes
        #[arg(long, value_enum)]
        fix: Option<FixKind>,
        /// Save even though the entry overlaps another
        #[arg(long)]
        force: bool,
    },
    /// Repeat the entries of the last day that had some (only onto an empty day)
    Copy {
        /// Day to copy from (yyyy-MM-dd); default: the latest day with entries in the last 7 days
        #[arg(long)]
        from: Option<String>,
        /// Day to copy onto (yyyy-MM-dd), default today
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Take back the last change made with this app
    Undo,
    /// Delete an entry
    Delete {
        /// Entry number
        number: usize,
        /// Day of the entry (yyyy-MM-dd), default today
        #[arg(short, long)]
        date: Option<String>,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum FixKind {
    /// Shorten the earlier entry so it ends where the later one starts
    Truncate,
    /// Move the later entry so it starts where the earlier one ends
    Shift,
    /// Count unbooked time between entries as a break
    Break,
}

#[derive(Debug, Subcommand)]
pub enum StampCommand {
    /// Start working (closes a session that is still running)
    In {
        /// Activity: id, number or words (omit to assign later)
        activity: Vec<String>,
        /// Use the activity you booked most recently
        #[arg(short, long, conflicts_with = "activity")]
        last: bool,
        /// Work report for this session
        #[arg(short, long)]
        comment: Option<String>,
    },
    /// Stop working
    Out {
        /// Work report for the session that ends
        #[arg(short, long)]
        comment: Option<String>,
        /// Stop at an earlier time today or yesterday (HH:MM), e.g. after forgetting to stamp out
        #[arg(long)]
        at: Option<String>,
    },
    /// Throw the running session away without booking it
    Cancel,
    /// Start a break
    Break,
    /// End a break
    Resume,
    /// Continue with another activity
    Switch {
        /// Activity: id, number or words
        #[arg(required_unless_present = "last")]
        activity: Vec<String>,
        /// Use the activity you booked most recently
        #[arg(short, long, conflicts_with = "activity")]
        last: bool,
        /// Work report for the new session
        #[arg(short, long)]
        comment: Option<String>,
    },
    /// Show whether you are stamped in and what is waiting for upload
    Status,
    /// Assign an activity to all entries that have none yet
    Assign {
        /// Activity: id, number or words
        #[arg(required = true)]
        activity: Vec<String>,
    },
    /// List the entries waiting for upload
    Queue,
    /// Drop an entry that is waiting for upload (numbers as shown by `queue`)
    Discard {
        number: usize,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// List the settings set in the config file
    Ls,
    /// Print the config file path only, e.g. for `$EDITOR "$(rolxapp config path)"`
    Path,
}
