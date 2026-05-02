use aura_core::orchestrator::{Engine, Event};
use aura_core::task::TaskType;
use aura_core::{Result, TaskId};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// URLs to download (as multiple sources for one file)
    #[arg(required = true, num_args = 1..)]
    uris: Vec<String>,

    /// Output filename
    #[arg(short, long)]
    output: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

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
    let config = aura_core::Config::default();
    let (engine, orchestrator, mut storage) = Engine::new(config).await?;
    
    // Inferred directory
    let current_dir = std::env::current_dir().unwrap();

    // Register and add all tasks
    for uri in &expanded_uris {
        let name = url::Url::parse(uri).ok()
            .and_then(|u| u.path_segments()?.last()?.to_string().into())
            .filter(|s: &String| !s.is_empty())
            .unwrap_or_else(|| "download.bin".to_string());
            
        let path = current_dir.join(&name);
        let id = TaskId(rand::random());
        storage.register_task(id, path);
        
        let ttype = if uri.ends_with(".torrent") { 
            TaskType::BitTorrent 
        } else if uri.starts_with("ftp://") || uri.starts_with("ftps://") {
            TaskType::Ftp
        } else { 
            TaskType::Http 
        };
        engine.add_task_with_id(id, name, uri.clone(), 0, ttype).await?;
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

    let mut events = engine.subscribe();

    // Multi-progress bar setup
    use indicatif::MultiProgress;
    let mp = MultiProgress::new();
    let mut bars = std::collections::HashMap::new();

    // Listen for events
    while let Ok(event) = events.recv().await {
        match event {
            Event::TaskAdded(id) => {
                let pb = mp.add(ProgressBar::new_spinner());
                pb.set_style(ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {msg}")
                    .expect("Failed to set spinner style"));
                pb.set_message(format!("Initializing task {}", id));
                bars.insert(id, pb);
            }
            Event::MetadataResolved { id, total_length, name: matured_name, .. } => {
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
            Event::TaskProgress { id, completed_bytes, total_bytes } => {
                if let Some(pb) = bars.get(&id) {
                    if pb.length() != Some(total_bytes) {
                        pb.set_length(total_bytes);
                    }
                    pb.set_position(completed_bytes);
                }
            }
            Event::TaskCompleted(id) => {
                if let Some(pb) = bars.get(&id) {
                    pb.finish_with_message(format!("Task {} complete", id));
                }
                if bars.values().all(|b| b.is_finished()) {
                    break;
                }
            }
            Event::TaskError { id, message } => {
                if let Some(pb) = bars.get(&id) {
                    pb.abandon_with_message(format!("Task {} error: {}", id, message));
                }
                if bars.values().all(|b| b.is_finished()) {
                    break;
                }
            }
            _ => {}
        }
    }

    engine.shutdown().await?;
    Ok(())
}
