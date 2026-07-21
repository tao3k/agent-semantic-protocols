mod storage_performance_receipt_tests {
    use agent_semantic_client_db::storage_performance_receipt::StorageLatencyDistributionMicros;

    #[test]
    fn latency_distribution_uses_nearest_rank_for_fixed_percentiles() {
        let samples: Vec<u64> = (1..=100).rev().collect();
        let receipt = StorageLatencyDistributionMicros::from_samples(&samples)
            .expect("non-empty latency receipt");
        assert_eq!(receipt.sample_count, 100);
        assert_eq!(receipt.p50, 51);
        assert_eq!(receipt.p95, 96);
        assert_eq!(receipt.p99, 100);
        assert_eq!(receipt.max, 100);
        assert!(StorageLatencyDistributionMicros::from_samples(&[]).is_none());
    }
}
