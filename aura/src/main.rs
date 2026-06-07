use clap::{Parser, Subcommand};

mod cli_client;

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

    /// Custom configuration file path
    #[arg(long, global = true)]
    config: Option<String>,

    /// Override download directory path
    #[arg(long, global = true)]
    download_dir: Option<String>,

    /// Override global download bandwidth limit
    #[arg(long, global = true)]
    limit: Option<u64>,

    /// Override global proxy URL
    #[arg(long, global = true)]
    proxy: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Aura background daemon (RPC/WebSocket)
    Daemon {
        /// Port to bind the RPC server
        #[arg(long)]
        rpc_port: Option<u16>,

        /// Token for authentication. If not provided, a random token is generated and saved to ~/.aura/rpc_secret.
        #[arg(long)]
        rpc_secret: Option<String>,

        /// Path to the TLS certificate file
        #[arg(long)]
        tls_cert: Option<String>,

        /// Path to the TLS private key file
        #[arg(long)]
        tls_key: Option<String>,

        /// Automatically generate self-signed TLS certificate and key files
        #[arg(long)]
        generate_tls_cert: bool,
    },
    /// Start the Terminal UI dashboard
    Tui,
    /// Probe the optimal allocation strategy for a given directory
    Probe {
        /// The directory to probe
        #[arg(default_value = ".")]
        dir: String,
    },
    /// View completed download history (ADR-0062)
    History {
        /// Limit the number of records displayed
        #[arg(long, short, default_value_t = 10)]
        limit: usize,

        /// Format to display the history (json, table)
        #[arg(long, short, default_value = "table")]
        format: String,

        /// Filter by task status (completed, failed, removed)
        #[arg(long, short)]
        filter: Option<String>,
    },
    /// Refresh the download metadata for a task, checking ETag and Last-Modified (conditional GET)
    Refresh {
        /// The GID of the task to refresh
        gid: u64,
    },
    /// Show files for a BitTorrent task
    ShowFiles {
        /// The GID of the task
        gid: u64,
    },
    /// Select specific files for a BitTorrent task
    SelectFiles {
        /// The GID of the task
        gid: u64,
        /// Indices of files to download (comma-separated, e.g., 0,2,5)
        #[arg(short, long, value_delimiter = ',')]
        indices: Vec<usize>,
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
        .with_writer(aura_daemon::scrubber::ScrubbingMakeWriter::new(
            std::io::stdout,
        ))
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    // Load configuration based on hierarchy rules
    let mut config = aura_core::Config::load_resolved(cli.config.as_deref())?;

    // Extract daemon-specific overrides
    let mut daemon_rpc_port = None;
    let mut daemon_rpc_secret = None;
    let mut daemon_tls_cert = None;
    let mut daemon_tls_key = None;
    let mut daemon_generate_tls_cert = false;
    if let Some(Commands::Daemon {
        rpc_port,
        rpc_secret,
        tls_cert,
        tls_key,
        generate_tls_cert,
    }) = &cli.command
    {
        daemon_rpc_port = *rpc_port;
        daemon_rpc_secret = rpc_secret.clone();
        daemon_tls_cert = tls_cert.clone();
        daemon_tls_key = tls_key.clone();
        daemon_generate_tls_cert = *generate_tls_cert;
    }

    // Apply CLI overrides to configuration
    config.apply_cli_overrides(aura_core::config::CliOverrides {
        download_dir: cli.download_dir.clone(),
        limit: cli.limit,
        proxy: cli.proxy.clone(),
        rpc_port: daemon_rpc_port,
        rpc_secret: daemon_rpc_secret,
        tls_cert: daemon_tls_cert.clone(),
        tls_key: daemon_tls_key.clone(),
    });

    match cli.command {
        Some(Commands::Daemon { .. }) => {
            // Run daemon
            let args = aura_daemon::Args {
                daemonize: false,
                config,
                tls_cert: daemon_tls_cert,
                tls_key: daemon_tls_key,
                generate_tls_cert: daemon_generate_tls_cert,
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
        Some(Commands::History {
            limit,
            format,
            filter,
        }) => {
            aura_cli::run_history(limit, &format, filter).await?;
        }
        Some(Commands::Refresh { gid }) => {
            cli_client::run_refresh(config.network.rpc_port, config.network.rpc_secret, gid)
                .await?;
        }
        Some(Commands::ShowFiles { gid }) => {
            cli_client::run_show_files(config.network.rpc_port, config.network.rpc_secret, gid)
                .await?;
        }
        Some(Commands::SelectFiles { gid, indices }) => {
            cli_client::run_select_files(
                config.network.rpc_port,
                config.network.rpc_secret,
                gid,
                indices,
            )
            .await?;
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
                    config,
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
