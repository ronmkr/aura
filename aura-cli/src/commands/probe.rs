use std::path::PathBuf;

pub async fn run_probe(dir: Option<String>) -> aura_core::Result<()> {
    let path = dir.unwrap_or_else(|| ".".to_string());
    let path_buf = PathBuf::from(path);

    let (method, dur) = aura_core::storage::prober::AllocationProber::probe(&path_buf).await?;
    println!("Probe results for {:?}:", path_buf);
    println!("Recommended allocation method: {:?}", method);
    println!("Time taken: {:?}", dur);

    Ok(())
}
