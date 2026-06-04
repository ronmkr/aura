use aura_core::history::{CompletedTaskRecord, HistoryManager};
use bytesize::ByteSize;

pub async fn run_history(
    limit: usize,
    format: &str,
    filter: Option<String>,
) -> aura_core::Result<()> {
    let mut records = HistoryManager::read_records();
    records.reverse(); // Newest first

    if let Some(f) = filter {
        let f_lower = f.to_lowercase();
        records.retain(|r| {
            if f_lower == "completed" {
                r.phase.to_lowercase() == "complete"
            } else {
                r.phase.to_lowercase() == f_lower
            }
        });
    }

    let records: Vec<CompletedTaskRecord> = records.into_iter().take(limit).collect();

    if format == "json" {
        println!(
            "{}",
            serde_json::to_string_pretty(&records).unwrap_or_default()
        );
    } else {
        // Table format
        println!(
            "{:<16} {:<32} {:<10} {:<12} {:<24}",
            "ID", "Name", "Status", "Size", "Completed At"
        );
        println!("{}", "-".repeat(98));
        for rec in records {
            let size_str = ByteSize::b(rec.total_bytes).to_string();
            let name_truncated = if rec.name.len() > 30 {
                format!("{}...", &rec.name[..27])
            } else {
                rec.name.clone()
            };
            println!(
                "{:<16} {:<32} {:<10} {:<12} {:<24}",
                rec.id,
                name_truncated,
                rec.phase,
                size_str,
                rec.completed_at.to_rfc3339()
            );
        }
    }

    Ok(())
}
