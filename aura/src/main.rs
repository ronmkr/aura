use clap::{Parser, Subcommand};
mod cli_client;
mod feed;
mod logging;
mod service;
#[cfg(target_os = "windows")]
mod service_windows;

#[derive(Parser, Debug)]
#[command(author, version, about = "Aura", long_about = None)]
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
        /// IP address to bind the RPC server
        #[arg(long)]
        bind_address: Option<String>,
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
        /// Run as a Windows Service (internal flag used by SCM)
        #[arg(long)]
        windows_service: bool,
    },
    /// Manage the Aura daemon as a system service (systemd, launchd, Windows Service)
    Service {
        #[command(subcommand)]
        action: service::ServiceAction,
    },
    /// Manage RSS/Atom feed subscriptions
    Feed {
        #[command(subcommand)]
        action: feed::FeedAction,
    },

    /// Start the Terminal UI dashboard
    Tui,
    /// Probe the optimal allocation strategy for a given directory
    Probe {
        /// The directory to probe
        #[arg(default_value = ".")]
        dir: String,
    },
    /// View completed download history (Decision-0062)
    History {
        /// Limit the number of records displayed
        #[arg(long, short, default_value_t = 10)]
        limit: usize,
        /// Format to display the history (json, table)
        #[arg(long, default_value = "table")]
        format: String,
        /// Filter by task status (completed, failed, removed)
        #[arg(long, short)]
        filter: Option<String>,
    },
    /// View real-time engine status and bandwidth schedules (Decision-0063)
    Status,
    /// Refresh the download metadata for a task, checking ETag and Last-Modified (conditional GET)
    Refresh {
        /// The GID of the task to refresh
        gid: u64,
    },
    /// Force a recheck on the target/part file for a task
    Recheck {
        /// The GID of the task to recheck
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
    /// Bulk add tasks from a directory (torrents, metalinks)
    AddFromFolder {
        /// The directory to scan
        dir: String,
        /// Scan recursively
        #[arg(short, long)]
        recursive: bool,
    },
    /// Bulk add tasks from a text file (list of URIs)
    AddFromFile {
        /// The file to read
        path: String,
    },
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    // Initialize tracing
    let is_service = std::env::var("AURA_SERVICE").is_ok()
        || std::env::var("AURA_LOG_JSON").is_ok()
        || std::env::args().any(|arg| arg == "--windows-service");
    let use_json = std::env::var("AURA_LOG_JSON").is_ok();

    logging::init_logging(cli.verbose, is_service, use_json);
    // Load configuration based on hierarchy rules
    let mut config = aura_core::Config::load_resolved(cli.config.as_deref())?;
    // Extract daemon-specific overrides
    let mut daemon_bind_address = None;
    let mut daemon_rpc_port = None;
    let mut daemon_rpc_secret = None;
    let mut daemon_tls_cert = None;
    let mut daemon_tls_key = None;
    let mut daemon_generate_tls_cert = false;
    let mut daemon_windows_service = false;
    if let Some(Commands::Daemon {
        bind_address,
        rpc_port,
        rpc_secret,
        tls_cert,
        tls_key,
        generate_tls_cert,
        windows_service,
    }) = &cli.command
    {
        daemon_bind_address = bind_address.clone();
        daemon_rpc_port = *rpc_port;
        daemon_rpc_secret = rpc_secret.clone();
        daemon_tls_cert = tls_cert.clone();
        daemon_tls_key = tls_key.clone();
        daemon_generate_tls_cert = *generate_tls_cert;
        daemon_windows_service = *windows_service;
    }
    // Apply CLI overrides to configuration
    config.apply_cli_overrides(aura_core::config::CliOverrides {
        download_dir: cli.download_dir.clone(),
        limit: cli.limit,
        proxy: cli.proxy.clone(),
        bind_address: daemon_bind_address,
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
                custom_shutdown: None,
            };
            #[cfg(target_os = "windows")]
            {
                if daemon_windows_service {
                    service_windows::run_as_windows_service(args)?;
                } else {
                    aura_daemon::run(args).await?;
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                if daemon_windows_service {
                    return Err("Windows service mode is only supported on Windows".into());
                }
                aura_daemon::run(args).await?;
            }
        }
        Some(Commands::Service { action }) => {
            service::handle_service_command(action)?;
        }
        Some(Commands::Feed { action }) => {
            feed::handle_feed_command(
                action,
                config.network.rpc_port,
                config.network.rpc_secret.clone(),
            )
            .await?;
        }

        Some(Commands::Tui) => {
            // Run TUI
            let rpc_url = format!("http://localhost:{}/jsonrpc", config.network.rpc_port);
            aura_tui::run(rpc_url, config.network.rpc_secret.clone()).await?;
        }
        Some(Commands::Probe { dir }) => {
            aura_cli::commands::probe::run_probe(Some(dir)).await?;
        }
        Some(Commands::History {
            limit,
            format,
            filter,
        }) => {
            aura_cli::run_history(&config, limit, &format, filter).await?;
        }
        Some(Commands::Status) => {
            cli_client::run_status(config.network.rpc_port, config.network.rpc_secret).await?;
        }
        Some(Commands::Refresh { gid }) => {
            cli_client::run_refresh(config.network.rpc_port, config.network.rpc_secret, gid)
                .await?;
        }
        Some(Commands::Recheck { gid }) => {
            cli_client::run_recheck(config.network.rpc_port, config.network.rpc_secret, gid)
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
        Some(Commands::AddFromFolder { dir, recursive }) => {
            cli_client::run_add_from_folder(
                config.network.rpc_port,
                config.network.rpc_secret,
                &dir,
                recursive,
            )
            .await?;
        }
        Some(Commands::AddFromFile { path }) => {
            cli_client::run_add_from_file(
                config.network.rpc_port,
                config.network.rpc_secret,
                &path,
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
