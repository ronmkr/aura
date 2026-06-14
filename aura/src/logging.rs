pub fn init_logging(verbose: u8, is_service: bool, use_json: bool) {
    let log_level = match verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };

    match (is_service, use_json) {
        (true, true) => {
            let subscriber = tracing_subscriber::FmtSubscriber::builder()
                .json()
                .with_max_level(log_level)
                .with_target(false)
                .with_writer(aura_daemon::scrubber::ScrubbingMakeWriter::new(
                    std::io::stderr,
                ))
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .expect("Failed to set tracing subscriber");
        }
        (true, false) => {
            let subscriber = tracing_subscriber::FmtSubscriber::builder()
                .with_max_level(log_level)
                .with_target(false)
                .with_writer(aura_daemon::scrubber::ScrubbingMakeWriter::new(
                    std::io::stderr,
                ))
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .expect("Failed to set tracing subscriber");
        }
        (false, true) => {
            let subscriber = tracing_subscriber::FmtSubscriber::builder()
                .json()
                .with_max_level(log_level)
                .with_target(false)
                .with_writer(aura_daemon::scrubber::ScrubbingMakeWriter::new(
                    std::io::stdout,
                ))
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .expect("Failed to set tracing subscriber");
        }
        (false, false) => {
            let subscriber = tracing_subscriber::FmtSubscriber::builder()
                .with_max_level(log_level)
                .with_target(false)
                .with_writer(aura_daemon::scrubber::ScrubbingMakeWriter::new(
                    std::io::stdout,
                ))
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .expect("Failed to set tracing subscriber");
        }
    }
}
