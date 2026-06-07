//! # temporal-compress
//!
//! Logarithmic time compression for cognitive event timelines.
//!
//! Recent events are stored at full resolution while older events are
//! compressed logarithmically, enabling efficient long-term memory storage.

/// A point in time with an associated event.
#[derive(Debug, Clone, PartialEq)]
pub struct TimePoint {
    pub timestamp: f64,
    pub event: String,
}

impl TimePoint {
    pub fn new(timestamp: f64, event: impl Into<String>) -> Self {
        TimePoint {
            timestamp,
            event: event.into(),
        }
    }
}

/// Compression tiers for time-based storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompressionTier {
    Last1m,
    Last1h,
    Last1d,
    Last1w,
    Last1y,
}

impl CompressionTier {
    /// Return the tier's duration in seconds.
    pub fn duration_secs(&self) -> f64 {
        match self {
            CompressionTier::Last1m => 60.0,
            CompressionTier::Last1h => 3600.0,
            CompressionTier::Last1d => 86400.0,
            CompressionTier::Last1w => 604800.0,
            CompressionTier::Last1y => 31536000.0,
        }
    }

    /// Determine the appropriate tier for an age (seconds ago).
    pub fn from_age(age_secs: f64) -> CompressionTier {
        if age_secs <= 60.0 {
            CompressionTier::Last1m
        } else if age_secs <= 3600.0 {
            CompressionTier::Last1h
        } else if age_secs <= 86400.0 {
            CompressionTier::Last1d
        } else if age_secs <= 604800.0 {
            CompressionTier::Last1w
        } else {
            CompressionTier::Last1y
        }
    }

    /// Maximum number of events to retain per tier.
    pub fn max_events(&self) -> usize {
        match self {
            CompressionTier::Last1m => 100,
            CompressionTier::Last1h => 60,
            CompressionTier::Last1d => 48,
            CompressionTier::Last1w => 28,
            CompressionTier::Last1y => 52,
        }
    }
}

/// A fidelity score indicating how much detail is preserved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FidelityScore(pub f64);

impl FidelityScore {
    pub fn new(score: f64) -> Self {
        FidelityScore(score.clamp(0.0, 1.0))
    }

    pub fn is_high(&self) -> bool {
        self.0 >= 0.8
    }

    pub fn is_low(&self) -> bool {
        self.0 < 0.3
    }
}

/// A compressed bucket of events within a tier.
#[derive(Debug, Clone)]
pub struct CompressedBucket {
    pub tier: CompressionTier,
    pub events: Vec<TimePoint>,
}

impl CompressedBucket {
    /// Compress a list of events down to the bucket max for this tier.
    /// Uses logarithmic sampling: more recent events are kept at higher density.
    pub fn compress(events: Vec<TimePoint>, tier: CompressionTier) -> Self {
        let max = tier.max_events();
        let compressed = if events.len() <= max {
            events
        } else {
            // Logarithmic sampling: keep proportionally more recent events
            let mut result = Vec::with_capacity(max);
            for i in 0..max {
                // Logarithmic mapping: earlier indices get denser sampling
                let ratio = i as f64 / max as f64;
                let source_idx = ((events.len() as f64 - 1.0) * (1.0 - (1.0 - ratio).powf(2.0))) as usize;
                result.push(events[source_idx.min(events.len() - 1)].clone());
            }
            result
        };
        CompressedBucket {
            tier,
            events: compressed,
        }
    }

    /// Number of events in this bucket.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// A queryable compressed timeline.
#[derive(Debug, Clone)]
pub struct CompressedTimeline {
    buckets: Vec<CompressedBucket>,
    now: f64,
}

impl CompressedTimeline {
    /// Create a new timeline at the given "now" timestamp.
    pub fn new(now: f64) -> Self {
        CompressedTimeline {
            buckets: Vec::new(),
            now,
        }
    }

    /// Add a bucket to the timeline.
    pub fn add_bucket(&mut self, bucket: CompressedBucket) {
        self.buckets.push(bucket);
    }

    /// Query all events within a time range.
    pub fn query_range(&self, start: f64, end: f64) -> Vec<&TimePoint> {
        self.buckets
            .iter()
            .flat_map(|b| b.events.iter())
            .filter(|tp| tp.timestamp >= start && tp.timestamp <= end)
            .collect()
    }

    /// Compute the overall fidelity score for this timeline.
    pub fn fidelity(&self) -> FidelityScore {
        if self.buckets.is_empty() {
            return FidelityScore::new(1.0);
        }
        let total_original: usize = self.buckets.iter().map(|b| b.events.len()).sum();
        let total_capacity: usize = self.buckets.iter().map(|b| b.tier.max_events()).sum();
        if total_capacity == 0 {
            FidelityScore::new(1.0)
        } else {
            FidelityScore::new(total_original as f64 / total_capacity as f64)
        }
    }

    /// Total number of events across all buckets.
    pub fn total_events(&self) -> usize {
        self.buckets.iter().map(|b| b.events.len()).sum()
    }

    /// Get reference to buckets.
    pub fn buckets(&self) -> &[CompressedBucket] {
        &self.buckets
    }
}

/// The time compressor that manages compression across tiers.
pub struct TimeCompressor {
    pub now: f64,
}

impl TimeCompressor {
    pub fn new(now: f64) -> Self {
        TimeCompressor { now }
    }

    /// Compress a list of events into a timeline.
    pub fn compress(&self, events: Vec<TimePoint>) -> CompressedTimeline {
        let mut timeline = CompressedTimeline::new(self.now);
        // Partition events by tier
        let mut tier_events: std::collections::HashMap<CompressionTier, Vec<TimePoint>> =
            std::collections::HashMap::new();
        for event in events {
            let age = self.now - event.timestamp;
            let tier = CompressionTier::from_age(age.max(0.0));
            tier_events.entry(tier).or_default().push(event);
        }
        // Compress each tier
        for (tier, events) in tier_events {
            let bucket = CompressedBucket::compress(events, tier);
            timeline.add_bucket(bucket);
        }
        timeline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_from_age() {
        assert_eq!(CompressionTier::from_age(30.0), CompressionTier::Last1m);
        assert_eq!(CompressionTier::from_age(120.0), CompressionTier::Last1h);
        assert_eq!(CompressionTier::from_age(5000.0), CompressionTier::Last1d);
        assert_eq!(CompressionTier::from_age(200000.0), CompressionTier::Last1w);
        assert_eq!(CompressionTier::from_age(40000000.0), CompressionTier::Last1y);
    }

    #[test]
    fn test_tier_ordering() {
        assert!(CompressionTier::Last1m < CompressionTier::Last1h);
        assert!(CompressionTier::Last1h < CompressionTier::Last1d);
        assert!(CompressionTier::Last1d < CompressionTier::Last1w);
        assert!(CompressionTier::Last1w < CompressionTier::Last1y);
    }

    #[test]
    fn test_tier_durations() {
        assert_eq!(CompressionTier::Last1h.duration_secs(), 3600.0);
        assert_eq!(CompressionTier::Last1d.duration_secs(), 86400.0);
    }

    #[test]
    fn test_bucket_compress_noop() {
        let events: Vec<TimePoint> = (0..5).map(|i| TimePoint::new(i as f64, format!("e{}", i))).collect();
        let bucket = CompressedBucket::compress(events, CompressionTier::Last1m);
        assert_eq!(bucket.len(), 5);
        assert_eq!(bucket.tier, CompressionTier::Last1m);
    }

    #[test]
    fn test_bucket_compress_large() {
        let events: Vec<TimePoint> = (0..200).map(|i| TimePoint::new(i as f64, format!("e{}", i))).collect();
        let bucket = CompressedBucket::compress(events, CompressionTier::Last1m);
        assert_eq!(bucket.len(), 100); // max for Last1m
    }

    #[test]
    fn test_timeline_query_range() {
        let mut timeline = CompressedTimeline::new(100.0);
        let events = vec![
            TimePoint::new(10.0, "early"),
            TimePoint::new(50.0, "mid"),
            TimePoint::new(90.0, "late"),
        ];
        timeline.add_bucket(CompressedBucket {
            tier: CompressionTier::Last1m,
            events,
        });
        let result = timeline.query_range(40.0, 95.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_timeline_total_events() {
        let mut timeline = CompressedTimeline::new(100.0);
        timeline.add_bucket(CompressedBucket {
            tier: CompressionTier::Last1m,
            events: vec![TimePoint::new(1.0, "a")],
        });
        timeline.add_bucket(CompressedBucket {
            tier: CompressionTier::Last1h,
            events: vec![TimePoint::new(0.0, "b"), TimePoint::new(0.5, "c")],
        });
        assert_eq!(timeline.total_events(), 3);
    }

    #[test]
    fn test_fidelity_score() {
        let high = FidelityScore::new(0.9);
        let low = FidelityScore::new(0.2);
        assert!(high.is_high());
        assert!(!high.is_low());
        assert!(!low.is_high());
        assert!(low.is_low());
    }

    #[test]
    fn test_fidelity_clamped() {
        let over = FidelityScore::new(1.5);
        assert_eq!(over.0, 1.0);
        let under = FidelityScore::new(-0.5);
        assert_eq!(under.0, 0.0);
    }

    #[test]
    fn test_compressor_basic() {
        let compressor = TimeCompressor::new(1000.0);
        let events = vec![
            TimePoint::new(999.0, "recent"),
            TimePoint::new(500.0, "hours ago"),
            TimePoint::new(100.0, "days ago"),
        ];
        let timeline = compressor.compress(events);
        assert!(timeline.total_events() >= 3);
    }

    #[test]
    fn test_compressor_many_events() {
        let compressor = TimeCompressor::new(100000.0);
        let events: Vec<TimePoint> = (0..500)
            .map(|i| TimePoint::new(i as f64 * 200.0, format!("e{}", i)))
            .collect();
        let timeline = compressor.compress(events);
        // Should compress into buckets
        assert!(timeline.total_events() < 500);
        assert!(!timeline.buckets().is_empty());
    }

    #[test]
    fn test_empty_timeline_fidelity() {
        let timeline = CompressedTimeline::new(0.0);
        assert!(timeline.fidelity().is_high());
    }

    #[test]
    fn test_timepoint_equality() {
        let a = TimePoint::new(1.0, "hello");
        let b = TimePoint::new(1.0, "hello");
        assert_eq!(a, b);
    }
}
