//! v1.1.0 bus loss signalling (ICX-AUD-054): readers learn how many events they missed.

use icx_bus::{ICX_RING_SLOTS, SeqlockTelemetryBus};
use icx_core::{AgentKind, ArtifactId};

fn publish_n(bus: &SeqlockTelemetryBus, n: usize) {
    for i in 0..n {
        bus.try_publish(
            AgentKind::ClaudeCode,
            ArtifactId(i as u64 + 1),
            7,
            0,
            [0; 4],
            [0; 16],
            "⟦Θ:x⟧",
            i as u64,
        )
        .expect("uncontended publish succeeds");
    }
}

#[test]
fn reader_that_falls_behind_by_more_than_the_ring_is_told_how_many_it_lost() {
    let bus = SeqlockTelemetryBus::new_in_memory();
    publish_n(&bus, ICX_RING_SLOTS + 10);
    let (events, lost) = bus.poll_since_with_loss(0);
    assert_eq!(events.len(), ICX_RING_SLOTS);
    assert_eq!(lost, 10);
    assert_eq!(bus.dropped_publishes(), 0);
}

#[test]
fn caught_up_reader_reports_no_loss() {
    let bus = SeqlockTelemetryBus::new_in_memory();
    publish_n(&bus, 5);
    let (events, lost) = bus.poll_since_with_loss(2);
    assert_eq!(events.len(), 3);
    assert_eq!(lost, 0);
}

#[test]
fn coherent_future_sequence_is_rejected_and_reported_lost() {
    use std::sync::atomic::Ordering;
    let bus = SeqlockTelemetryBus::new_in_memory();
    publish_n(&bus, 1);
    let layout = bus.in_process_layout().unwrap();
    layout.slots[0].seqlock.store(257 << 1, Ordering::Release);
    let (events, lost) = bus.poll_since_with_loss(0);
    assert!(events.is_empty(), "unexpected future slot: {events:?}");
    assert_eq!(lost, 1);
}

#[cfg(unix)]
#[test]
fn file_backed_bus_preserves_nonzero_metadata_and_each_coordinate_lane() {
    let dir = std::path::PathBuf::from("/tmp/Infinity-Context/v120")
        .join(format!("bus-metadata-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bus = SeqlockTelemetryBus::open_shared_file(dir.join("fabric.shm")).unwrap();
    let coords = std::array::from_fn(|i| (i16::try_from(i).unwrap() - 8) * 100);
    let packet = format!("⟦Θ:codex@metadata⊢{}⟧", "public ".repeat(30));
    bus.try_publish(
        AgentKind::Codex,
        ArtifactId(42),
        7,
        219,
        [0x1234, 0x5678, 0x9abc, 0xdef0],
        coords,
        &packet,
        123_456,
    )
    .unwrap();
    let (events, lost) = bus.poll_since_with_loss(0);
    assert_eq!(lost, 0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].surprise_q8, 219);
    assert_eq!(events[0].fractal_q15, coords);
    assert_eq!(events[0].hdc_sketch, [0x1234, 0x5678, 0x9abc, 0xdef0]);
    assert_eq!(events[0].ahtl_packet, packet);
    assert_eq!(events[0].timestamp_ms, 123_456);
    drop(bus);
    std::fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn separately_opened_file_handles_see_each_others_events() {
    // ICX-AUD-055: the file-backed bus is what crosses process boundaries; two independent
    // opens model two agent processes. Measures publish -> visible latency.
    let dir = std::env::temp_dir().join(format!("icx_bus_xproc_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fabric.shm");
    let writer = SeqlockTelemetryBus::open_shared_file(&path).unwrap();
    let reader = SeqlockTelemetryBus::open_shared_file(&path).unwrap();
    let mut worst = std::time::Duration::ZERO;
    for i in 0..200u64 {
        let since = reader.write_sequence();
        let t = std::time::Instant::now();
        writer
            .try_publish(
                AgentKind::Codex,
                ArtifactId(i + 1),
                9,
                0,
                [0; 4],
                [0; 16],
                "⟦Θ:y⟧",
                i,
            )
            .unwrap();
        let (events, lost) = reader.poll_since_with_loss(since);
        worst = worst.max(t.elapsed());
        assert_eq!(lost, 0);
        assert_eq!(
            events.last().map(|e| e.artifact_id),
            Some(ArtifactId(i + 1))
        );
    }
    eprintln!("file-backed bus worst publish->visible latency: {worst:?}");
    assert!(worst < std::time::Duration::from_millis(50), "{worst:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
