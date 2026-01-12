//! daedalos-notify - Desktop notifications for Daedalos
//!
//! A unified notification system that works across platforms.
//! Sends desktop notifications for long-running tasks and events.

use anyhow::Result;
use clap::{Parser, Subcommand};

use daedalos_notify::{
    Backend, Notification, NotificationHistory, Urgency,
};

#[derive(Parser)]
#[command(name = "notify")]
#[command(about = "Desktop notifications for Daedalos - alert when long tasks complete")]
#[command(version)]
#[command(after_help = r#"WHEN TO USE:
    - Long-running builds or tests complete
    - Background tasks finish while you're away
    - Important events need your attention

EXAMPLES:
    notify "Build complete"                # Simple notification
    notify success "Tests passed"          # Success notification with icon
    notify error "Deploy failed" -u        # Urgent error notification
    notify watch cargo build               # Notify when command finishes
    notify progress "Still running..."     # Silent progress update

PLATFORM SUPPORT:
    Linux: notify-send (libnotify)
    macOS: osascript / terminal-notifier
    Windows: PowerShell toast notifications
"#)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Message to send (when no subcommand is used)
    #[arg(trailing_var_arg = true)]
    message: Vec<String>,

    /// Notification title
    #[arg(short, long, global = true, default_value = "Daedalos")]
    title: String,

    /// Icon name or path
    #[arg(short, long, global = true)]
    icon: Option<String>,

    /// Mark as urgent
    #[arg(short, long, global = true)]
    urgent: bool,

    /// Auto-dismiss timeout in seconds
    #[arg(long, global = true)]
    timeout: Option<u32>,

    /// Disable sound
    #[arg(long, global = true)]
    no_sound: bool,

    /// Command to run when clicked
    #[arg(long, global = true)]
    action: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a notification with custom title and options
    Send {
        /// Message to display
        message: String,
    },

    /// Send a success notification (green checkmark icon)
    Success {
        /// Success message
        message: String,
    },

    /// Send an error notification (red X icon)
    Error {
        /// Error message
        message: String,
    },

    /// Send a warning notification (yellow warning icon)
    Warn {
        /// Warning message
        message: String,
    },

    /// Send a silent progress notification (no sound, low urgency)
    Progress {
        /// Progress message
        message: String,
    },

    /// Run a command and notify when it completes (success/failure)
    Watch {
        /// Command to run and watch
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Show recent notification history
    History {
        /// Number of notifications to show
        #[arg(default_value = "20")]
        limit: usize,
    },

    /// Clear all notification history
    Clear,

    /// Test notification system and show detected backend
    Test,
}

fn build_notification(cli: &Cli, message: &str) -> Notification {
    let mut notif = Notification::new(message).with_title(&cli.title);

    if let Some(icon) = &cli.icon {
        notif = notif.with_icon(icon);
    }

    if cli.urgent {
        notif = notif.with_urgency(Urgency::Critical);
    }

    if let Some(timeout) = cli.timeout {
        notif = notif.with_timeout(timeout);
    }

    if cli.no_sound {
        notif = notif.silent();
    }

    if let Some(action) = &cli.action {
        notif = notif.with_action(action);
    }

    notif
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Send { message }) => {
            let notif = build_notification(&cli, message);
            daedalos_notify::send(&notif)?;
            println!("Notification sent");
        }

        Some(Commands::Success { message }) => {
            let notif = Notification::new(message)
                .with_title("Success")
                .with_urgency(Urgency::Normal);
            daedalos_notify::send(&notif)?;
            println!("Success notification sent");
        }

        Some(Commands::Error { message }) => {
            let urgency = if cli.urgent {
                Urgency::Critical
            } else {
                Urgency::Normal
            };
            let notif = Notification::new(message)
                .with_title("Error")
                .with_urgency(urgency);
            daedalos_notify::send(&notif)?;
            println!("Error notification sent");
        }

        Some(Commands::Warn { message }) => {
            let notif = Notification::new(message)
                .with_title("Warning")
                .with_urgency(Urgency::Normal);
            daedalos_notify::send(&notif)?;
            println!("Warning notification sent");
        }

        Some(Commands::Progress { message }) => {
            let notif = Notification::new(message)
                .with_title("In Progress")
                .with_urgency(Urgency::Low)
                .silent();
            daedalos_notify::send(&notif)?;
            println!("Progress notification sent");
        }

        Some(Commands::Watch { command }) => {
            let cmd = command.join(" ");
            println!("Running: {}", cmd);

            let exit_code = daedalos_notify::watch(&cmd).await?;

            if exit_code == 0 {
                println!("Command completed successfully");
            } else {
                println!("Command failed with exit code {}", exit_code);
                std::process::exit(exit_code);
            }
        }

        Some(Commands::History { limit }) => {
            let history = NotificationHistory::new()?;

            if history.is_empty() {
                println!("No notification history");
                return Ok(());
            }

            println!("Notification History\n");

            let records = history.recent(*limit)?;
            for record in records {
                let urgency_marker = match record.urgency.as_str() {
                    "critical" => " [!]",
                    _ => "",
                };
                println!(
                    "{}{}\n  {}: {}\n",
                    record.datetime().format("%Y-%m-%d %H:%M:%S"),
                    urgency_marker,
                    record.title,
                    record.message
                );
            }
        }

        Some(Commands::Clear) => {
            let history = NotificationHistory::new()?;
            history.clear()?;
            println!("Notification history cleared");
        }

        Some(Commands::Test) => {
            let backend = Backend::detect();
            println!("Testing notification system");
            println!("  Backend: {}", backend.name());
            println!();

            let notif = Notification::new("If you see this, notifications are working!")
                .with_title("Daedalos Test");
            daedalos_notify::send(&notif)?;

            println!();
            println!("Test notification sent");
            println!("Check if you received the notification");
        }

        None => {
            // Default behavior: treat positional args as message
            if cli.message.is_empty() {
                // No message, show help
                use clap::CommandFactory;
                Cli::command().print_help()?;
                return Ok(());
            }

            let message = cli.message.join(" ");
            let notif = build_notification(&cli, &message);
            daedalos_notify::send(&notif)?;
            println!("Notification sent");
        }
    }

    Ok(())
}
