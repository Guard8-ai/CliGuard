use serde::{Deserialize, Serialize};

/// The value type of a flag or argument.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValueType {
    String,
    Bool,
    Int,
    Float,
    Path,
    Enum(Vec<std::string::String>),
    Custom(std::string::String),
}

/// Which CLI framework generated the help output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Framework {
    Clap,
    Cobra,
    Click,
    Argparse,
    GnuStyle,
    Unknown,
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Clap => write!(f, "clap"),
            Self::Cobra => write!(f, "cobra"),
            Self::Click => write!(f, "click"),
            Self::Argparse => write!(f, "argparse"),
            Self::GnuStyle => write!(f, "gnu"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// A CLI flag/option (e.g. `--verbose`, `-o FILE`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flag {
    pub short: Option<std::string::String>,
    pub long: Option<std::string::String>,
    pub description: std::string::String,
    pub value_type: ValueType,
    pub required: bool,
    pub default: Option<std::string::String>,
    pub env_var: Option<std::string::String>,
}

/// A positional argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Arg {
    pub name: std::string::String,
    pub description: std::string::String,
    pub value_type: ValueType,
    pub required: bool,
    pub variadic: bool,
}

/// A group of related commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandGroup {
    pub name: std::string::String,
    pub commands: Vec<std::string::String>,
}

/// A CLI command or subcommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub name: std::string::String,
    pub description: std::string::String,
    pub aliases: Vec<std::string::String>,
    pub subcommands: Vec<Command>,
    pub flags: Vec<Flag>,
    pub args: Vec<Arg>,
    pub examples: Vec<std::string::String>,
}

/// The complete specification of a CLI tool, as parsed from its help output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: std::string::String,
    pub version: Option<std::string::String>,
    pub description: std::string::String,
    pub framework: Framework,
    pub commands: Vec<Command>,
    pub global_flags: Vec<Flag>,
    pub groups: Vec<CommandGroup>,
}
