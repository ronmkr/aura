use super::*;

#[tokio::test]
async fn test_prober_sparse() {
    let dir = tempfile::tempdir().unwrap();
    let (method, dur) = AllocationProber::probe(dir.path()).await.unwrap();
    assert!(matches!(
        method,
        AllocationMethod::Sparse | AllocationMethod::ZeroFill | AllocationMethod::Fallocate
    ));
    assert!(dur.as_nanos() > 0);
}
