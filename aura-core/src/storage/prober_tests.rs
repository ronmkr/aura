use crate::storage::prober::{AllocationMethod, AllocationProber};
use tempfile::tempdir;

#[tokio::test]
async fn test_allocation_prober() {
    let dir = tempdir().unwrap();
    let (method, dur): (AllocationMethod, std::time::Duration) =
        AllocationProber::probe(dir.path()).await.unwrap();

    assert!(matches!(
        method,
        AllocationMethod::Sparse | AllocationMethod::ZeroFill | AllocationMethod::Fallocate
    ));
    assert!(dur.as_nanos() > 0);
}
