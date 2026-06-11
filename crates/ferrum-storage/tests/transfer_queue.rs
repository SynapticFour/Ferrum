//! Transfer queue drain behaviour.

use ferrum_storage::{BandwidthMonitor, TransferDirection, TransferQueue};

#[test]
fn test_very_low_defers_drain_until_schedule() {
    let tq = TransferQueue::new(3600);
    let bw = BandwidthMonitor::new(Default::default());
    bw.inject_mock_bps(50_000);
    tq.enqueue(
        "obj-large".into(),
        12 * 1024 * 1024,
        TransferDirection::Download,
    );
    assert_eq!(tq.len(), 1);
    assert!(tq.drain_if_ready(&bw).is_empty());
    assert_eq!(tq.len(), 1);
}

#[test]
fn test_high_bandwidth_drains_when_due() {
    let tq = TransferQueue::new(0);
    let bw = BandwidthMonitor::new(Default::default());
    bw.inject_mock_bps(20_000_000);
    tq.enqueue(
        "obj-large".into(),
        12 * 1024 * 1024,
        TransferDirection::Download,
    );
    let drained = tq.drain_if_ready(&bw);
    assert_eq!(drained.len(), 1);
    assert_eq!(drained[0].object_id, "obj-large");
    assert_eq!(tq.len(), 0);
}
