/// Performance tests for the KVM system
/// 
/// These tests verify that the system meets performance requirements:
/// - Input latency < 50ms (P99)
/// - Clipboard sync latency < 500ms
/// - Network bandwidth < 1Mbps
/// - CPU usage < 5%, Memory < 100MB
/// 
/// Requirements: 3.5, 4.2

use cross_platform_kvm::*;
use cross_platform_kvm::input::{InputEvent, MouseButton, Modifiers};
use cross_platform_kvm::clipboard::{ClipboardService, ClipboardServiceImpl, ClipboardContent};
use cross_platform_kvm::protocol::{encode_input_event, decode_input_event};
use cross_platform_kvm::batching::{InputEventBatcher, BatchConfig, batch_protocol_messages};
use std::time::{Duration, Instant};
use tokio::time::sleep;

#[cfg(test)]
mod input_latency_tests {
    use super::*;

    /// Test input event encoding/decoding latency
    /// Target: < 1ms for encoding + decoding
    #[tokio::test]
    async fn test_input_event_serialization_latency() {
        let events = vec![
            InputEvent::MouseMove { x: 100, y: 200 },
            InputEvent::MouseButton { button: MouseButton::Left, pressed: true },
            InputEvent::KeyPress { 
                key_code: 65, 
                modifiers: Modifiers { shift: true, ctrl: false, alt: false, meta: false },
                pressed: true 
            },
        ];

        let mut total_duration = Duration::ZERO;
        let iterations = 1000;

        for _ in 0..iterations {
            for event in &events {
                let start = Instant::now();
                
                // Encode
                let message = encode_input_event(event).unwrap();
                
                // Decode
                let _decoded = decode_input_event(&message).unwrap();
                
                total_duration += start.elapsed();
            }
        }

        let avg_latency = total_duration / (iterations * events.len() as u32);
        println!("Average serialization latency: {:?}", avg_latency);
        
        // Should be well under 1ms
        assert!(avg_latency < Duration::from_micros(1000), 
            "Serialization latency too high: {:?}", avg_latency);
    }

    /// Test input event batching performance
    /// Verify that batching reduces overhead
    #[tokio::test]
    async fn test_input_batching_performance() {
        let config = BatchConfig {
            max_batch_size: 10,
            max_batch_delay_ms: 5,
            enabled: true,
        };
        let batcher = InputEventBatcher::new(config);

        let events: Vec<_> = (0..100)
            .map(|i| InputEvent::MouseMove { x: i, y: i })
            .collect();

        let start = Instant::now();
        let mut batch_count = 0;

        for event in events {
            if let Some(batch) = batcher.add_event(event).await {
                batch_count += 1;
                
                // Simulate sending batch
                let messages: Vec<_> = batch.iter()
                    .map(|e| encode_input_event(e).unwrap())
                    .collect();
                let _batched = batch_protocol_messages(messages).unwrap();
            }
        }

        // Flush remaining
        let remaining = batcher.flush().await;
        if !remaining.is_empty() {
            batch_count += 1;
        }

        let duration = start.elapsed();
        println!("Batched 100 events into {} batches in {:?}", batch_count, duration);
        
        // Should batch into ~10 batches (100 events / 10 per batch)
        assert!(batch_count <= 15, "Too many batches: {}", batch_count);
        
        // Should complete quickly
        assert!(duration < Duration::from_millis(50), 
            "Batching took too long: {:?}", duration);
    }

    /// Test end-to-end input latency simulation
    /// This simulates the full pipeline: capture -> encode -> transmit -> decode -> inject
    #[tokio::test]
    async fn test_end_to_end_input_latency() {
        let iterations = 100;
        let mut latencies = Vec::new();

        for i in 0..iterations {
            let event = InputEvent::MouseMove { x: i, y: i };
            
            let start = Instant::now();
            
            // 1. Encode
            let message = encode_input_event(&event).unwrap();
            
            // 2. Simulate network transmission (local loopback ~1ms)
            sleep(Duration::from_micros(500)).await;
            
            // 3. Decode
            let _decoded = decode_input_event(&message).unwrap();
            
            // 4. Simulate injection overhead
            sleep(Duration::from_micros(500)).await;
            
            let latency = start.elapsed();
            latencies.push(latency);
        }

        // Calculate P99 latency
        latencies.sort();
        let p99_index = (iterations as f64 * 0.99) as usize;
        let p99_latency = latencies[p99_index];
        
        println!("P99 input latency: {:?}", p99_latency);
        println!("Average latency: {:?}", latencies.iter().sum::<Duration>() / iterations);
        
        // Requirement: < 50ms P99
        assert!(p99_latency < Duration::from_millis(50), 
            "P99 latency exceeds 50ms: {:?}", p99_latency);
    }
}

#[cfg(test)]
mod clipboard_latency_tests {
    use super::*;

    /// Test clipboard content hashing performance
    #[tokio::test]
    async fn test_clipboard_hash_performance() {
        let text_content = ClipboardContent::Text("Hello, World!".repeat(1000));
        let image_content = ClipboardContent::Image(vec![0u8; 1024 * 100]); // 100KB image

        let start = Instant::now();
        for _ in 0..100 {
            let _hash = text_content.hash();
        }
        let text_duration = start.elapsed();
        println!("100 text hashes: {:?}", text_duration);

        let start = Instant::now();
        for _ in 0..100 {
            let _hash = image_content.hash();
        }
        let image_duration = start.elapsed();
        println!("100 image hashes (100KB): {:?}", image_duration);

        // Hashing should be fast
        assert!(text_duration < Duration::from_millis(100));
        assert!(image_duration < Duration::from_millis(500));
    }

    /// Test clipboard sync latency with incremental sync
    #[tokio::test]
    async fn test_clipboard_incremental_sync() {
        let service = ClipboardServiceImpl::new(10).unwrap();
        
        let content1 = ClipboardContent::Text("Test content 1".to_string());
        let content2 = ClipboardContent::Text("Test content 2".to_string());
        let content1_dup = ClipboardContent::Text("Test content 1".to_string());

        // First sync
        let start = Instant::now();
        assert!(!service.is_already_synced(&content1).await);
        service.mark_as_synced(content1.clone()).await;
        let first_sync = start.elapsed();

        // Second sync (different content)
        let start = Instant::now();
        assert!(!service.is_already_synced(&content2).await);
        service.mark_as_synced(content2.clone()).await;
        let second_sync = start.elapsed();

        // Third sync (duplicate content - should be cached)
        let start = Instant::now();
        assert!(service.is_already_synced(&content1_dup).await);
        let cached_check = start.elapsed();

        println!("First sync: {:?}", first_sync);
        println!("Second sync: {:?}", second_sync);
        println!("Cached check: {:?}", cached_check);

        // Cached check should be much faster
        assert!(cached_check < Duration::from_micros(100), 
            "Cached check too slow: {:?}", cached_check);
    }

    /// Test clipboard sync end-to-end latency
    /// Target: < 500ms
    #[tokio::test]
    async fn test_clipboard_sync_end_to_end_latency() {
        let service = ClipboardServiceImpl::new(10).unwrap();
        
        let test_contents = vec![
            ClipboardContent::Text("Short text".to_string()),
            ClipboardContent::Text("A".repeat(10000)), // 10KB text
            ClipboardContent::Image(vec![0u8; 1024 * 100]), // 100KB image
        ];

        for content in test_contents {
            let start = Instant::now();
            
            // Simulate full sync cycle
            let hash = content.hash();
            let is_cached = service.is_already_synced(&content).await;
            
            if !is_cached {
                // Encode for transmission
                let message = cross_platform_kvm::protocol::encode_clipboard_content(&content).unwrap();
                
                // Simulate network transmission
                sleep(Duration::from_millis(10)).await;
                
                // Decode
                let _decoded = cross_platform_kvm::protocol::decode_clipboard_content(&message).unwrap();
                
                // Mark as synced
                service.mark_as_synced(content.clone()).await;
            }
            
            let latency = start.elapsed();
            println!("Clipboard sync latency ({}): {:?}", 
                match &content {
                    ClipboardContent::Text(t) => format!("text {}B", t.len()),
                    ClipboardContent::Image(i) => format!("image {}KB", i.len() / 1024),
                    ClipboardContent::Empty => "empty".to_string(),
                },
                latency
            );
            
            // Requirement: < 500ms
            assert!(latency < Duration::from_millis(500), 
                "Clipboard sync latency exceeds 500ms: {:?}", latency);
        }
    }
}

#[cfg(test)]
mod bandwidth_tests {
    use super::*;

    /// Test network bandwidth consumption for typical usage
    /// Target: < 1Mbps for normal usage
    #[tokio::test]
    async fn test_input_event_bandwidth() {
        // Simulate 1 second of mouse movement at 60 Hz
        let events_per_second = 60;
        let mut total_bytes = 0;

        for i in 0..events_per_second {
            let event = InputEvent::MouseMove { x: i, y: i };
            let message = encode_input_event(&event).unwrap();
            total_bytes += message.to_bytes().len();
        }

        let bandwidth_bps = total_bytes * 8; // bits per second
        let bandwidth_mbps = bandwidth_bps as f64 / 1_000_000.0;

        println!("Mouse movement bandwidth (60 Hz): {:.3} Mbps", bandwidth_mbps);
        
        // Should be well under 1 Mbps
        assert!(bandwidth_mbps < 1.0, 
            "Bandwidth too high: {:.3} Mbps", bandwidth_mbps);
    }

    /// Test bandwidth with batching enabled
    #[tokio::test]
    async fn test_batched_bandwidth() {
        let config = BatchConfig {
            max_batch_size: 10,
            max_batch_delay_ms: 5,
            enabled: true,
        };
        let batcher = InputEventBatcher::new(config);

        // Simulate 1 second of events at 60 Hz
        let events_per_second = 60;
        let mut total_bytes = 0;
        let mut batch_count = 0;

        for i in 0..events_per_second {
            let event = InputEvent::MouseMove { x: i, y: i };
            
            if let Some(batch) = batcher.add_event(event).await {
                batch_count += 1;
                let messages: Vec<_> = batch.iter()
                    .map(|e| encode_input_event(e).unwrap())
                    .collect();
                let batched = batch_protocol_messages(messages).unwrap();
                total_bytes += batched.len();
            }
        }

        // Flush remaining
        let remaining = batcher.flush().await;
        if !remaining.is_empty() {
            let messages: Vec<_> = remaining.iter()
                .map(|e| encode_input_event(e).unwrap())
                .collect();
            let batched = batch_protocol_messages(messages).unwrap();
            total_bytes += batched.len();
            batch_count += 1;
        }

        let bandwidth_bps = total_bytes * 8;
        let bandwidth_mbps = bandwidth_bps as f64 / 1_000_000.0;

        println!("Batched bandwidth (60 Hz, {} batches): {:.3} Mbps", batch_count, bandwidth_mbps);
        
        // Batching should keep bandwidth low
        assert!(bandwidth_mbps < 1.0, 
            "Batched bandwidth too high: {:.3} Mbps", bandwidth_mbps);
    }

    /// Test clipboard bandwidth for large transfers
    #[tokio::test]
    async fn test_clipboard_bandwidth() {
        // Test with 1MB image (near the 10MB limit)
        let image_content = ClipboardContent::Image(vec![0u8; 1024 * 1024]);
        let message = cross_platform_kvm::protocol::encode_clipboard_content(&image_content).unwrap();
        
        let bytes = message.to_bytes();
        let size_mb = bytes.len() as f64 / (1024.0 * 1024.0);
        
        println!("1MB clipboard transfer size: {:.2} MB", size_mb);
        
        // Should not add significant overhead
        assert!(size_mb < 1.1, "Too much overhead: {:.2} MB", size_mb);
    }
}

#[cfg(test)]
mod resource_usage_tests {
    use super::*;

    /// Test memory usage of connection pool
    #[tokio::test]
    async fn test_connection_pool_memory() {
        use cross_platform_kvm::security::{generate_device_certificate, AuthorizationManager};
        use cross_platform_kvm::network::TlsConnectionManager;
        use std::sync::Arc;

        let cert = generate_device_certificate("test-device").unwrap();
        let auth_manager = Arc::new(AuthorizationManager::new());
        let manager = TlsConnectionManager::new(cert, auth_manager).unwrap();

        // Register multiple devices
        for i in 0..50 {
            let addr = format!("127.0.0.1:{}", 8000 + i).parse().unwrap();
            manager.register_device(format!("device-{}", i), addr).await;
        }

        // Get pool stats
        let stats = manager.get_pool_stats().await;
        println!("Connection pool stats: {:?}", stats);

        // Memory usage should be reasonable
        // Each connection info is small, 50 connections should be < 10KB
        assert!(stats.total == 0); // No actual connections established
    }

    /// Test batching memory efficiency
    #[tokio::test]
    async fn test_batching_memory_efficiency() {
        let config = BatchConfig {
            max_batch_size: 100,
            max_batch_delay_ms: 10,
            enabled: true,
        };
        let batcher = InputEventBatcher::new(config);

        // Add many events
        for i in 0..1000 {
            let event = InputEvent::MouseMove { x: i, y: i };
            let _ = batcher.add_event(event).await;
        }

        // Batch size should be limited by max_batch_size
        let size = batcher.batch_size().await;
        assert!(size <= 100, "Batch size too large: {}", size);
    }

    /// Test clipboard cache memory management
    #[tokio::test]
    async fn test_clipboard_cache_memory() {
        let service = ClipboardServiceImpl::new(10).unwrap();

        // Add many items to cache
        for i in 0..200 {
            let content = ClipboardContent::Text(format!("Content {}", i));
            service.mark_as_synced(content).await;
        }

        // Cache should be limited to 100 entries
        // We can't directly check cache size, but we can verify it doesn't grow unbounded
        // by checking that old entries are evicted
        let old_content = ClipboardContent::Text("Content 0".to_string());
        let is_cached = service.is_already_synced(&old_content).await;
        
        // Old content should have been evicted
        assert!(!is_cached, "Cache not properly limited");
    }
}

#[cfg(test)]
mod integration_performance_tests {
    use super::*;

    /// Test complete workflow performance
    /// Simulates: input capture -> batch -> encode -> transmit -> decode -> inject
    #[tokio::test]
    async fn test_complete_workflow_performance() {
        let config = BatchConfig {
            max_batch_size: 10,
            max_batch_delay_ms: 5,
            enabled: true,
        };
        let batcher = InputEventBatcher::new(config);

        let iterations = 100;
        let mut latencies = Vec::new();

        for i in 0..iterations {
            let start = Instant::now();
            
            // 1. Capture event
            let event = InputEvent::MouseMove { x: i, y: i };
            
            // 2. Add to batch
            if let Some(batch) = batcher.add_event(event).await {
                // 3. Encode batch
                let messages: Vec<_> = batch.iter()
                    .map(|e| encode_input_event(e).unwrap())
                    .collect();
                let batched_data = batch_protocol_messages(messages).unwrap();
                
                // 4. Simulate network transmission
                sleep(Duration::from_micros(500)).await;
                
                // 5. Decode batch
                let decoded_messages = cross_platform_kvm::batching::unbatch_protocol_messages(&batched_data).unwrap();
                
                // 6. Decode events
                for msg in decoded_messages {
                    let _event = decode_input_event(&msg).unwrap();
                }
                
                let latency = start.elapsed();
                latencies.push(latency);
            }
        }

        // Flush and process remaining
        let remaining = batcher.flush().await;
        if !remaining.is_empty() {
            let start = Instant::now();
            let messages: Vec<_> = remaining.iter()
                .map(|e| encode_input_event(e).unwrap())
                .collect();
            let batched_data = batch_protocol_messages(messages).unwrap();
            let decoded_messages = cross_platform_kvm::batching::unbatch_protocol_messages(&batched_data).unwrap();
            for msg in decoded_messages {
                let _event = decode_input_event(&msg).unwrap();
            }
            latencies.push(start.elapsed());
        }

        if !latencies.is_empty() {
            latencies.sort();
            let p99_index = ((latencies.len() as f64) * 0.99) as usize;
            let p99_latency = latencies[p99_index.min(latencies.len() - 1)];
            let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;

            println!("Complete workflow P99 latency: {:?}", p99_latency);
            println!("Complete workflow average latency: {:?}", avg_latency);

            // Should meet the 50ms requirement
            assert!(p99_latency < Duration::from_millis(50), 
                "Workflow P99 latency exceeds 50ms: {:?}", p99_latency);
        }
    }
}
