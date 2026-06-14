use service_manager::*;
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(clap::Subcommand, Debug, Clone)]
pub enum ServiceAction {
    /// Install the daemon as a system service
    Install {
        /// Custom configuration file path to use for the service
        #[arg(long)]
        config: Option<String>,
        /// IP address to bind the RPC server
        #[arg(long)]
        bind_address: Option<String>,
        /// Port to bind the RPC server
        #[arg(long)]
        rpc_port: Option<u16>,
        /// The username under which the service should run (e.g. root or Administrator)
        #[arg(long)]
        user: Option<String>,
    },
    /// Uninstall the system service
    Uninstall,
    /// Start the system service
    Start,
    /// Stop the system service
    Stop,
    /// Check the status of the system service
    Status,
}

pub fn handle_service_command(action: ServiceAction) -> Result<(), Box<dyn std::error::Error>> {
    let label: ServiceLabel = "com.aura.daemon".parse()?;

    let manager = match <dyn ServiceManager>::native() {
        Ok(m) => m,
        Err(e) => {
            return Err(format!(
                "Failed to detect native service manager on this platform: {}. \
                Please ensure you run this command on a supported platform (systemd, launchd, or Windows Service).",
                e
            ).into());
        }
    };

    match action {
        ServiceAction::Install {
            config,
            bind_address,
            rpc_port,
            user,
        } => {
            println!("Installing Aura daemon as a system service...");

            let exe_path = std::env::current_exe()?;
            let resolved_config = config.map(|cfg| {
                std::fs::canonicalize(&cfg)
                    .unwrap_or_else(|_| PathBuf::from(cfg))
                    .into_os_string()
                    .into_string()
                    .unwrap_or_default()
            });
            let args = build_install_args(resolved_config, bind_address, rpc_port);

            let ctx = ServiceInstallCtx {
                label: label.clone(),
                program: exe_path,
                args,
                contents: None,
                username: user,
                working_directory: None,
                environment: None,
                autostart: true,
                restart_policy: RestartPolicy::default(),
            };

            match manager.install(ctx) {
                Ok(_) => {
                    println!("Aura daemon service installed successfully.");
                    println!("To start the service, run: aura service start");
                }
                Err(e) => {
                    eprintln!("Error installing service: {}", e);
                    suggest_privileges();
                    return Err(e.into());
                }
            }
        }
        ServiceAction::Uninstall => {
            println!("Uninstalling Aura daemon service...");
            match manager.uninstall(ServiceUninstallCtx { label }) {
                Ok(_) => println!("Aura daemon service uninstalled successfully."),
                Err(e) => {
                    eprintln!("Error uninstalling service: {}", e);
                    suggest_privileges();
                    return Err(e.into());
                }
            }
        }
        ServiceAction::Start => {
            println!("Starting Aura daemon service...");
            match manager.start(ServiceStartCtx { label }) {
                Ok(_) => println!("Aura daemon service started successfully."),
                Err(e) => {
                    eprintln!("Error starting service: {}", e);
                    suggest_privileges();
                    return Err(e.into());
                }
            }
        }
        ServiceAction::Stop => {
            println!("Stopping Aura daemon service...");
            match manager.stop(ServiceStopCtx { label }) {
                Ok(_) => println!("Aura daemon service stopped successfully."),
                Err(e) => {
                    eprintln!("Error stopping service: {}", e);
                    suggest_privileges();
                    return Err(e.into());
                }
            }
        }
        ServiceAction::Status => {
            let label_str = "com.aura.daemon";
            println!("Querying status for service '{}'...", label_str);

            #[cfg(target_os = "linux")]
            {
                let output = std::process::Command::new("systemctl")
                    .arg("status")
                    .arg(label_str)
                    .output()?;

                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                print!("{}", stdout);
                eprint!("{}", stderr);
            }

            #[cfg(target_os = "macos")]
            {
                let output = std::process::Command::new("launchctl")
                    .arg("list")
                    .output()?;

                let stdout = String::from_utf8_lossy(&output.stdout);
                let found = stdout.lines().find(|line| line.contains(label_str));
                if let Some(line) = found {
                    println!("Service is registered in launchd:");
                    println!("  {}", line);
                } else {
                    println!("Service '{}' not found in launchd active list.", label_str);
                }
            }

            #[cfg(target_os = "windows")]
            {
                let output = std::process::Command::new("sc.exe")
                    .arg("query")
                    .arg(label_str)
                    .output()?;

                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                print!("{}", stdout);
                eprint!("{}", stderr);
            }

            #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
            {
                println!("Service status query not supported on this platform.");
            }
        }
    }

    Ok(())
}

pub fn build_install_args(
    config: Option<String>,
    bind_address: Option<String>,
    rpc_port: Option<u16>,
) -> Vec<OsString> {
    let mut args = vec![OsString::from("daemon")];

    if let Some(cfg) = config {
        args.push(OsString::from("--config"));
        args.push(OsString::from(cfg));
    }
    if let Some(addr) = bind_address {
        args.push(OsString::from("--bind-address"));
        args.push(OsString::from(addr));
    }
    if let Some(port) = rpc_port {
        args.push(OsString::from("--rpc-port"));
        args.push(OsString::from(port.to_string()));
    }

    #[cfg(target_os = "windows")]
    {
        args.push(OsString::from("--windows-service"));
    }

    args
}

fn suggest_privileges() {
    #[cfg(unix)]
    {
        eprintln!("Note: Service management commands typically require administrator privileges. Try running this command with 'sudo'.");
    }
    #[cfg(windows)]
    {
        eprintln!("Note: Service management commands typically require administrator privileges. Please run your terminal/prompt as Administrator.");
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
