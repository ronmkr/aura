use aura_core::storage::StorageDispatch;
use aura_core::test_helpers::MockStorage;
use aura_core::worker::Segment;
use aura_core::TaskId;
use bytes::BytesMut;
use std::sync::Arc;

#[tokio::test]
async fn test_storage_backpressure_throttles_writes() {
    let mock_storage = Arc::new(MockStorage::new());

    // Set a very low pressure threshold
    *mock_storage.pressure_threshold.lock().await = Some(1);

    let task_id = TaskId::random();
    let segment = Segment {
        offset: 0,
        length: 100,
    };

    // First write should succeed
    let res1 = mock_storage
        .submit_write(
            task_id,
            segment.clone(),
            BytesMut::from(&[0u8; 100][..]),
            None,
            None,
        )
        .await;
    assert!(res1.is_ok());

    // Second write should fail due to backpressure
    let res2 = mock_storage
        .submit_write(
            task_id,
            segment,
            BytesMut::from(&[0u8; 100][..]),
            None,
            None,
        )
        .await;
    assert!(res2.is_err());
    assert_eq!(
        res2.unwrap_err().to_string(),
        "Storage error: Mock: backpressure triggered"
    );
}
