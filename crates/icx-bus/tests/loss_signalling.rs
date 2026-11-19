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
