use super::*;

#[tokio::test]
async fn test_prober_sparse() {
    let dir = tempfile::tempdir().unwrap();
    let (method, dur) = AllocationProber::probe(dir.path()).await.unwrap();
    assert_eq!(method, AllocationMethod::Sparse);
    assert!(dur.as_nanos() > 0);
}
