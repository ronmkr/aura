use aura_core::orchestrator::{Engine, Event};
use aura_core::task::{FollowOnAction, TaskType};
use aura_core::{Result, TaskId};

use indicatif::{ProgressBar, ProgressStyle};
use rand::RngExt;

#[derive(Debug)]
pub struct Args {
    pub uris: Vec<String>,
    pub output: Option<String>,
    pub follow_on: Option<String>,
}

pub async fn run(args: Args) -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let mut expanded_uris = Vec::new();
    for uri in args.uris {
        if let Ok(expanded) = aura_core::glob::expand_url(&uri) {
            expanded_uris.extend(expanded);
        } else {
            expanded_uris.push(uri);
        }
    }

    if expanded_uris.is_empty() {
        return Ok(());
    }

    // Bootstrap the engine
    let config = aura_core::Config::from_file("Aura.toml").unwrap_or_default();
    let (engine, orchestrator, mut storage) = Engine::new(config).await?;

    let mut events = engine.subscribe();

    // Inferred directory
    let current_dir = std::env::current_dir().unwrap();

    // Register and add all tasks
    let mut tasks_to_add = Vec::new();
    for uri in &expanded_uris {
        let path_obj = std::path::Path::new(uri);
        let is_local_file = path_obj.exists() && path_obj.is_file();

        let (inferred_name, is_metadata) = if is_local_file
            && (uri.ends_with(".torrent") || uri.ends_with(".metalink") || uri.ends_with(".meta4"))
        {
            ("unnamed".to_string(), true)
        } else if is_local_file {
            (
                path_obj
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "download.bin".to_string()),
                false,
            )
        } else {
            (
                url::Url::parse(uri)
                    .ok()
                    .and_then(|u| u.path_segments()?.next_back()?.to_string().into())
                    .filter(|s: &String| !s.is_empty())
                    .unwrap_or_else(|| "download.bin".to_string()),
                false,
            )
        };

        tasks_to_add.push((uri.clone(), inferred_name, is_metadata));
    }

    if tasks_to_add.is_empty() {
        return Ok(());
    }

    // If output is specified, we treat all URIs as sources for that one output
    if let Some(output_name) = &args.output {
        let id = TaskId(rand::rng().random());
        let path = current_dir.join(output_name);
        storage.register_task(id, path, 0, None, Vec::new()).await;

        let mut sources = Vec::new();
        for (uri, _, _) in tasks_to_add {
            let ttype = if uri.ends_with(".torrent") {
                TaskType::BitTorrent
            } else if uri.starts_with("ftp://") || uri.starts_with("ftps://") {
                TaskType::Ftp
            } else {
                TaskType::Http
            };
            sources.push((uri, ttype));
        }

        engine
            .add_task_with_options(
                id,
                None,
                output_name.clone(),
                sources,
                None,
                100,
                false,
                Vec::new(),
                args.follow_on.map(FollowOnAction::Custom),
            )
            .await?;
    } else {
        for (uri, name, is_metadata) in tasks_to_add {
            let path = current_dir.join(&name);
            let id = TaskId(rand::rng().random());
            if !is_metadata {
                storage.register_task(id, path, 0, None, Vec::new()).await;
            }

            let ttype = if uri.ends_with(".torrent") {
                TaskType::BitTorrent
            } else if uri.ends_with(".metalink") || uri.ends_with(".meta4") {
                TaskType::Http
            } else if uri.starts_with("ftp://") || uri.starts_with("ftps://") {
                TaskType::Ftp
            } else {
                TaskType::Http
            };
            engine
                .add_task_with_options(
                    id,
                    None,
                    name,
                    vec![(uri.clone(), ttype)],
                    None,
                    100,
                    false,
                    Vec::new(),
                    args.follow_on.clone().map(FollowOnAction::Custom),
                )
                .await?;
        }
    }

    // Spawn the actors
    tokio::spawn(async move {
        if let Err(e) = orchestrator.run().await {
            tracing::error!("Orchestrator error: {}", e);
        }
    });

    tokio::spawn(async move {
        if let Err(e) = storage.run().await {
            tracing::error!("Storage Engine error: {}", e);
        }
    });

    // Multi-progress bar setup
    use indicatif::MultiProgress;
    let mp = MultiProgress::new();
    let mut bars = std::collections::HashMap::new();

    // Listen for events
    while let Ok(event) = events.recv().await {
        match event {
            Event::TaskAdded(id) => {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner:.green} [{elapsed_precise}] {msg}")
                        .expect("Failed to set spinner style"),
                );
                pb.set_message(format!("Initializing task {}", id));
                bars.insert(id, pb);
            }
            Event::MetadataResolved {
                id,
                total_length,
                name: matured_name,
                ..
            } => {
                if let Some(pb) = bars.get(&id) {
                    let display_name = matured_name.unwrap_or_else(|| format!("{}", id));
                    pb.set_length(total_length);
                    pb.set_style(ProgressStyle::default_bar()
                        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta}) {msg}")
                        .expect("Failed to set progress bar style")
                        .progress_chars("#>-"));
                    pb.set_message(format!("Downloading {}", display_name));
                }
            }
            Event::TaskProgress {
                id,
                completed_bytes,
                uploaded_bytes,
                total_bytes,
            } => {
                if let Some(pb) = bars.get(&id) {
                    if pb.length() != Some(total_bytes) {
                        pb.set_length(total_bytes);
                    }
                    pb.set_position(completed_bytes);
                    if uploaded_bytes > 0 {
                        pb.set_message(format!("UP: {}", bytesize::ByteSize::b(uploaded_bytes)));
                    }
                }
            }
            Event::TaskCompleted(id) => {
                if let Some(pb) = bars.get(&id) {
                    pb.finish_with_message(format!("Task {} complete", id));
                }
                if !bars.is_empty() && bars.values().all(|b| b.is_finished()) {
                    break;
                }
            }
            Event::TaskError { id, message } => {
                if let Some(pb) = bars.get(&id) {
                    pb.abandon_with_message(format!("Task {} error: {}", id, message));
                }
                if !bars.is_empty() && bars.values().all(|b| b.is_finished()) {
                    break;
                }
            }
            Event::TaskPaused(id) => {
                if let Some(pb) = bars.get(&id) {
                    pb.set_message(format!("Task {} paused", id.0));
                }
            }
            Event::TaskResumed(id) => {
                if let Some(pb) = bars.get(&id) {
                    pb.set_message(format!("Task {} resumed", id.0));
                }
            }
        }
    }

    engine.shutdown().await?;
    Ok(())
}
