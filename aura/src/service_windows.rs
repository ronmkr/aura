#![cfg(target_os = "windows")]

use std::ffi::OsString;
use std::sync::Mutex;
use std::time::Duration;
use windows_service::{
    define_windows_service,
    service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus},
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

static DAEMON_ARGS: Mutex<Option<aura_daemon::Args>> = Mutex::new(None);

define_windows_service!(ffi_service_main, my_service_main);

pub fn run_as_windows_service(args: aura_daemon::Args) -> Result<(), Box<dyn std::error::Error>> {
    {
        let mut guard = DAEMON_ARGS.lock().unwrap();
        *guard = Some(args);
    }

    // service_dispatcher::start blocks until the service stops.
    service_dispatcher::start("com.aura.daemon", ffi_service_main)?;
    Ok(())
}

fn my_service_main(_arguments: Vec<OsString>) {
    // Create a channel to communicate SCM stop signal to the running daemon
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let shutdown_tx_clone = shutdown_tx.clone();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                let _ = shutdown_tx_clone.try_send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = match service_control_handler::register("com.aura.daemon", event_handler) {
        Ok(handle) => handle,
        Err(_) => return,
    };

    // Signal start pending
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: windows_service::service::ServiceType::OWN_PROCESS,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(5),
    });

    // Retrieve daemon args
    let mut args = match {
        let mut guard = DAEMON_ARGS.lock().unwrap();
        guard.take()
    } {
        Some(a) => a,
        None => {
            let _ = status_handle.set_service_status(ServiceStatus {
                service_type: windows_service::service::ServiceType::OWN_PROCESS,
                current_state: ServiceState::Stopped,
                controls_accepted: ServiceControlAccept::empty(),
                exit_code: ServiceExitCode::Win32(1053), // ERROR_SERVICE_SPECIFIC_ERROR
                checkpoint: 0,
                wait_hint: Duration::from_secs(0),
            });
            return;
        }
    };

    // Inject custom shutdown receiver into the daemon arguments
    args.custom_shutdown = Some(shutdown_rx);

    // Signal running
    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: windows_service::service::ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(0),
    });

    // Start a new multithreaded runtime and block on daemon run
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            let _ = status_handle.set_service_status(ServiceStatus {
                service_type: windows_service::service::ServiceType::OWN_PROCESS,
                current_state: ServiceState::Stopped,
                controls_accepted: ServiceControlAccept::empty(),
                exit_code: ServiceExitCode::Win32(1053),
                checkpoint: 0,
                wait_hint: Duration::from_secs(0),
            });
            return;
        }
    };

    let run_result = rt.block_on(async { aura_daemon::run(args).await });

    // Signal service stopped
    let exit_code = match run_result {
        Ok(_) => 0,
        Err(_) => 1,
    };

    let _ = status_handle.set_service_status(ServiceStatus {
        service_type: windows_service::service::ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(exit_code),
        checkpoint: 0,
        wait_hint: Duration::from_secs(0),
    });
}
