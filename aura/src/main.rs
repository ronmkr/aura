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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

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
        None => {
            // Default CLI behavior
            if let Some(uris) = cli.uris {
                let args = aura_cli::Args {
                    uris,
                    output: cli.output,
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
