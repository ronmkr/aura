use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about = "Aura - The Next-Gen Download Manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// URLs to download (as multiple sources for one file) - Standard CLI mode
    #[arg(num_args = 1..)]
    uris: Option<Vec<String>>,

    /// Output filename (for CLI mode)
    #[arg(short, long)]
    output: Option<String>,

    /// URI to automatically download after current tasks complete (Task Chaining)
    #[arg(long)]
    follow_on: Option<String>,

    /// Enable verbose logging (repeat for more detail: -v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Priority level (0-5, where 0 is highest)
    #[arg(short, long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(0..=5))]
    priority: u32,

    /// Task GIDs this task depends on (comma-separated list)
    #[arg(short, long, value_delimiter = ',')]
    depends_on: Vec<u64>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Aura background daemon (RPC/WebSocket)
    Daemon {
        /// Port to bind the RPC server
        #[arg(long, default_value = "6800")]
        rpc_port: u16,

        /// Token for authentication
        #[arg(long, default_value = "aura_secret_token")]
        rpc_secret: String,
    },
    /// Start the Terminal UI dashboard
    Tui,
    /// Probe the optimal allocation strategy for a given directory
    Probe {
        /// The directory to probe
        #[arg(default_value = ".")]
        dir: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize tracing
    let log_level = match cli.verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    match cli.command {
        Some(Commands::Daemon {
            rpc_port,
            rpc_secret,
        }) => {
            // Run daemon
            let args = aura_daemon::Args {
                rpc_port,
                rpc_secret,
                daemonize: false,
                config: None,
            };
            aura_daemon::run(args).await?;
        }
        Some(Commands::Tui) => {
            // Run TUI
            aura_tui::run().await?;
        }
        Some(Commands::Probe { dir }) => {
            aura_cli::commands::probe::run_probe(Some(dir)).await?;
        }
        None => {
            // Default CLI behavior
            if let Some(uris) = cli.uris {
                let args = aura_cli::Args {
                    uris,
                    output: cli.output,
                    follow_on: cli.follow_on,
                    priority: cli.priority,
                    depends_on: cli.depends_on.into_iter().map(aura_core::TaskId).collect(),
                };
                aura_cli::run(args).await?;
            } else {
                // If no subcommands or URIs provided, print help
                use clap::CommandFactory;
                let mut cmd = Cli::command();
                cmd.print_help()?;
            }
        }
    }

    Ok(())
}
