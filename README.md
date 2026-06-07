# Temporal Compress

[![crates.io](https://img.shields.io/crates/v/temporal-compress.svg)](https://crates.io/crates/temporal-compress)
[![docs.rs](https://docs.rs/temporal-compress/badge.svg)](https://docs.rs/temporal-compress)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> **Logarithmic time compression for cognitive event timelines — recent events at full fidelity, older events logarithmically compressed.**

---

## The Problem

Agents generate enormous event streams. Storing every event at full resolution is prohibitively expensive, but naive downsampling loses critical recent detail. Human memory solves this naturally: you remember this morning in vivid detail, last week in broad strokes, and last year as a few landmark events.

## Why This Exists

Temporal Compress implements **logarithmic time compression** with tiered buckets:
- **Last 1 minute**: Up to 100 events, full fidelity
- **Last 1 hour**: Up to 60 events, slight compression
- **Last 1 day**: Up to 48 events
- **Last 1 week**: Up to 28 events
- **Last 1 year**: Up to 52 events

Recent events are kept at full density while older events use logarithmic sampling, preserving proportionally more recent data within each tier.

## Architecture

```
  Time ──────────────────────────────────────────→
  
  │◄── Last1m (100) ──►│                        │
                       │◄── Last1h (60) ──►│    │
                                            │◄─ Last1d (48) ─►│
                                                               │◄─ Last1w ─►│
                                                                            │◄─ Last1y ─►│
  
  Density: ████████████ ██████████  ████████  ██████  ████  ██
           high ←──────────────────────────────────────→ low
  
  FidelityScore: actual_events / max_events per tier
```

## Installation

```toml
[dependencies]
temporal-compress = "0.1"
```

## API Reference

### `CompressionTier`

Five tiers of time-based compression:

```rust
use temporal_compress::CompressionTier;

assert_eq!(CompressionTier::Last1m.duration_secs(), 60.0);
assert_eq!(CompressionTier::Last1h.duration_secs(), 3600.0);
assert_eq!(CompressionTier::Last1d.duration_secs(), 86400.0);

assert_eq!(CompressionTier::from_age(30.0), CompressionTier::Last1m);
assert_eq!(CompressionTier::from_age(120.0), CompressionTier::Last1h);
```

### `CompressedBucket`

A compressed set of events within a tier:

```rust
use temporal_compress::*;

let events: Vec<TimePoint> = (0..200)
    .map(|i| TimePoint::new(i as f64, format!("event_{}", i)))
    .collect();

let bucket = CompressedBucket::compress(events, CompressionTier::Last1m);
assert_eq!(bucket.len(), 100); // compressed to max_events
```

### `CompressedTimeline`

Queryable timeline across all tiers:

```rust
use temporal_compress::*;

let mut timeline = CompressedTimeline::new(1000.0);
// ... add buckets ...
let events = timeline.query_range(500.0, 800.0);
let fidelity = timeline.fidelity();
let total = timeline.total_events();
```

### `TimeCompressor`

The main entry point for compressing event streams:

```rust
use temporal_compress::*;

let compressor = TimeCompressor::new(100000.0);
let events: Vec<TimePoint> = (0..500)
    .map(|i| TimePoint::new(i as f64 * 200.0, format!("e{}", i)))
    .collect();

let timeline = compressor.compress(events);
// Events are partitioned into tiers and compressed
assert!(timeline.total_events() < 500);
```

### `FidelityScore`

```rust
use temporal_compress::FidelityScore;

let high = FidelityScore::new(0.9);
assert!(high.is_high());

let low = FidelityScore::new(0.2);
assert!(low.is_low());
```

## Usage Examples

### Example 1: Compress a Day of Events

```rust
use temporal_compress::*;

let now = 86400.0; // 1 day in seconds
let compressor = TimeCompressor::new(now);

let events: Vec<TimePoint> = (0..1000)
    .map(|i| TimePoint::new(i as f64 * 86.4, format!("tick_{}", i)))
    .collect();

let timeline = compressor.compress(events);
println!("Compressed {} events into {} across {} buckets",
    1000, timeline.total_events(), timeline.buckets().len());
```

### Example 2: Query Historical Events

```rust
use temporal_compress::*;

let mut timeline = CompressedTimeline::new(100.0);
timeline.add_bucket(CompressedBucket {
    tier: CompressionTier::Last1m,
    events: vec![
        TimePoint::new(10.0, "early"),
        TimePoint::new(50.0, "mid"),
        TimePoint::new(90.0, "late"),
    ],
});

let results = timeline.query_range(40.0, 95.0);
assert_eq!(results.len(), 2);
```

### Example 3: Fidelity Analysis

```rust
use temporal_compress::*;

let compressor = TimeCompressor::new(100000.0);
let events: Vec<TimePoint> = (0..500)
    .map(|i| TimePoint::new(i as f64 * 200.0, format!("e{}", i)))
    .collect();

let timeline = compressor.compress(events);
let fidelity = timeline.fidelity();
println!("Timeline fidelity: {:.1}%", fidelity.0 * 100.0);
```

## Mathematical Background

**Logarithmic Sampling** maps target indices back to source indices using:

```
source_idx = (N - 1) × (1 - (1 - ratio)²)
```

Where `ratio = i / max_events`. This quadratic mapping preserves more density at the recent end of each bucket, mimicking the logarithmic nature of human memory retention (Ebbinghaus forgetting curve).

**Fidelity Score**:

```
F = actual_events / max_capacity
```

Range [0, 1]. Above 0.8 is high fidelity; below 0.3 is low fidelity.

## Performance

| Operation | Complexity |
|-----------|-----------|
| Tier classification | O(1) |
| Bucket compression | O(n) |
| Timeline query | O(B × E) |
| Full compression | O(n log n) |

## License

Licensed under the [MIT License](LICENSE).

## Contributing

1. Fork the repository
2. Create a feature branch
3. Write tests
4. Push and open a Pull Request
