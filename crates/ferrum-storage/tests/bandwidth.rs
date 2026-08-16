// SPDX-License-Identifier: BUSL-1.1
use ferrum_core::BandwidthConfig;
use ferrum_storage::{BandwidthClass, BandwidthMonitor};

#[test]
fn test_bandwidth_class_detection() {
    let monitor = BandwidthMonitor::new(BandwidthConfig::default());
    monitor.inject_mock_bps(20_000_000);
    assert_eq!(monitor.classify(), BandwidthClass::High);
    monitor.inject_mock_bps(5_000_000);
    assert_eq!(monitor.classify(), BandwidthClass::Medium);
    monitor.inject_mock_bps(500_000);
    assert_eq!(monitor.classify(), BandwidthClass::Low);
    monitor.inject_mock_bps(50_000);
    assert_eq!(monitor.classify(), BandwidthClass::VeryLow);
}

#[test]
fn test_chunk_sizes_by_class() {
    assert_eq!(BandwidthClass::High.chunk_size_bytes(), 64 * 1024 * 1024);
    assert_eq!(BandwidthClass::VeryLow.chunk_size_bytes(), 512 * 1024);
}
