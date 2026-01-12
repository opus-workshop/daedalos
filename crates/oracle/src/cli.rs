//! CLI argument parsing for oracle

use clap::Parser;

/// How oracle was invoked (affects default behavior)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InvocationStyle {
    /// Full oracle experience
    Oracle,
    /// Standard mode
    Ora,
    /// One-shot focused
    Ask,
}

#[derive(Parser, Debug)]
#[command(name = "oracle")]
#[command(about = "Unified LLM interface - the curl for language models")]
#[command(version)]
#[command(after_help = "\
ALIASES:
    oracle    REPL mode, verbose output
    ora       REPL mode, standard output
    ask       One-shot if prompt given, REPL otherwise

EXAMPLES:
    ask \"what does this function do?\"
    git diff | ask \"review this change\"
    ask -c \"what about edge cases?\"
    ask -s myproject \"continue our discussion\"
    ask -b ollama \"explain this locally\"

BACKENDS:
    claude     Claude Code CLI (default)
    opencode   OpenCode CLI
    ollama     Local models via Ollama

CONFIGURATION:
    ~/.config/oracle/config.toml

    [default]
    backend = \"claude\"

    [backends.claude]
    command = \"claude\"
    args = [\"-p\", \"{prompt}\"]")]
pub struct Cli {
    /// The prompt to send (if not provided, enters REPL mode)
    #[arg(trailing_var_arg = true, num_args = 0..)]
    pub prompt_parts: Vec<String>,

    /// The combined prompt (computed from prompt_parts)
    #[arg(skip)]
    pub prompt: Option<String>,

    /// Continue last conversation
    #[arg(short = 'c', long = "continue")]
    pub continue_session: bool,

    /// Use/resume named session
    #[arg(short = 's', long = "session")]
    pub session: Option<String>,

    /// Override backend (claude, opencode, ollama)
    #[arg(short = 'b', long = "backend")]
    pub backend: Option<String>,

    /// Output as JSON (for tool consumption)
    #[arg(short = 'j', long = "json")]
    pub json: bool,

    /// Suppress non-essential output
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,
}

impl Cli {
    /// Parse CLI with invocation style context
    pub fn parse_with_style(_style: InvocationStyle) -> Self {
        let mut cli = Self::parse();

        // Combine prompt parts into single prompt
        if !cli.prompt_parts.is_empty() {
            cli.prompt = Some(cli.prompt_parts.join(" "));
        }

        cli
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parse() {
        Cli::command().debug_assert();
    }
}
