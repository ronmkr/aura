use aura_core::rss::{FeedSubscription, RssManager};
use serde_json::json;

#[derive(clap::Subcommand, Debug, Clone)]
pub enum FeedAction {
    /// Subscribe to an RSS/Atom feed
    Add {
        /// URL of the feed
        url: String,
        /// Custom name for this subscription
        #[arg(long)]
        name: Option<String>,
        /// Custom polling interval in minutes (default is 30)
        #[arg(long)]
        poll_interval: Option<u64>,
        /// Title matching filters
        #[arg(long, short)]
        filter: Vec<String>,
        /// Category matching filters
        #[arg(long, short)]
        category: Vec<String>,
        /// Maximum item size limit in bytes
        #[arg(long)]
        max_size: Option<u64>,
    },
    /// Unsubscribe from a feed by URL or name
    Remove {
        /// Name or URL of the feed
        target: String,
    },
    /// List all subscribed feeds
    List,
    /// Force a refresh/poll of all feeds immediately
    Refresh,
}

pub async fn handle_feed_command(
    action: FeedAction,
    rpc_port: u16,
    rpc_secret: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let rss_manager = RssManager::new();

    match action {
        FeedAction::Add {
            url,
            name,
            poll_interval,
            filter,
            category,
            max_size,
        } => {
            let name = name.unwrap_or_else(|| {
                if let Ok(u) = url::Url::parse(&url) {
                    u.host_str().unwrap_or(&url).to_string()
                } else {
                    url.clone()
                }
            });

            let filters = if filter.is_empty() {
                None
            } else {
                Some(filter)
            };

            let categories = if category.is_empty() {
                None
            } else {
                Some(category)
            };

            let sub = FeedSubscription {
                url: url.clone(),
                name: name.clone(),
                poll_interval,
                filters,
                categories,
                max_size,
            };

            match rss_manager.add_subscription(sub) {
                Ok(_) => {
                    println!("Successfully subscribed to feed '{}' (URL: {}).", name, url);
                    let _ = trigger_rpc_refresh(rpc_port, rpc_secret).await;
                }
                Err(e) => {
                    eprintln!("Error adding subscription: {}", e);
                    std::process::exit(1);
                }
            }
        }
        FeedAction::Remove { target } => match rss_manager.remove_subscription(&target) {
            Ok(_) => {
                println!("Successfully unsubscribed from feed '{}'.", target);
            }
            Err(e) => {
                eprintln!("Error removing subscription: {}", e);
                std::process::exit(1);
            }
        },
        FeedAction::List => match rss_manager.load_subscriptions() {
            Ok(subs) => {
                if subs.is_empty() {
                    println!("No active feed subscriptions.");
                    return Ok(());
                }
                println!(
                    "{:<25} {:<10} {:<40} {:<30}",
                    "Name", "Interval", "Filters", "URL"
                );
                println!("{}", "-".repeat(110));
                for s in subs {
                    let interval = format!("{}m", s.poll_interval.unwrap_or(30));
                    let filter_str = s
                        .filters
                        .as_ref()
                        .map(|f| f.join(", "))
                        .unwrap_or_else(|| "None".to_string());

                    let url_disp = if s.url.len() > 40 {
                        format!("{}...", &s.url[..37])
                    } else {
                        s.url.clone()
                    };

                    println!(
                        "{:<25} {:<10} {:<40} {:<30}",
                        s.name, interval, filter_str, url_disp
                    );
                }
            }
            Err(e) => {
                eprintln!("Error loading subscriptions: {}", e);
                std::process::exit(1);
            }
        },
        FeedAction::Refresh => {
            println!("Requesting immediate polling of all feeds from daemon...");
            match trigger_rpc_refresh(rpc_port, rpc_secret).await {
                Ok(_) => println!("Feeds refresh requested successfully."),
                Err(e) => {
                    eprintln!(
                        "Failed to trigger daemon refresh: {}. Daemon might be offline.",
                        e
                    );
                }
            }
        }
    }

    Ok(())
}

async fn trigger_rpc_refresh(
    port: u16,
    secret: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let url = format!("http://localhost:{}/jsonrpc", port);
    let secret = aura_core::Config::resolve_rpc_secret(secret);

    let mut req = client.post(&url);
    if let Some(ref sec) = secret {
        req = req.header(aura_core::RPC_AUTH_HEADER, sec);
    }

    let payload = json!({
        "jsonrpc": aura_core::JSONRPC_VERSION,
        "method": "aura.refreshFeeds",
        "params": vec![json!(())],
        "id": "cli-feed-refresh"
    });

    let resp = req.json(&payload).send().await?;
    let body: serde_json::Value = resp.json().await?;
    if let Some(err) = body.get("error") {
        return Err(format!("RPC error: {}", err).into());
    }
    Ok(())
}
