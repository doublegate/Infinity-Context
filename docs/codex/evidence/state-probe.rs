//! Isolated audit probe. All writes are confined to a freshly created /tmp workspace.
use icx_adapters::{InfinityContextMmu, McpPagerServer};
use icx_bus::SeqlockTelemetryBus;
use icx_core::{AgentKind, ArtifactId};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[path = "/home/parobek/Code/OSS_Public-Projects/Infinity-Context/crates/icx-cli/src/daemon.rs"]
mod daemon;

fn dispatch(_: AgentKind, event: &str, payload: &str, mmu: &mut InfinityContextMmu) -> Result<String, serde_json::Error> {
    let result = mmu.ingest_payload(AgentKind::GenericMcp, "audit-crash-fixture", payload, false);
    if event == "FAIL_FINAL_FLUSH" {
        let dir = Path::new(payload);
        std::fs::rename(dir.join("icx.sqlite3"), dir.join("icx.sqlite3.preserved")).unwrap();
        std::fs::create_dir(dir.join("icx.sqlite3")).unwrap();
    }
    Ok(result.artifact_id.to_string())
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("bus-writer") {
        let bus = SeqlockTelemetryBus::open_shared_file(&args[2]).unwrap();
        for i in 0..100 {
            let ns = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            bus.try_publish(AgentKind::GenericMcp, ArtifactId(i), 77, 0, [0;4], [0;16], &ns.to_string(), 1).unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
        return;
    }
    if matches!(args.get(1).map(String::as_str), Some("daemon" | "daemon-final-flush")) {
        let path = Path::new(&args[2]);
        let socket = path.join("audit.sock");
        let mut mmu = InfinityContextMmu::default();
        mmu.load_workspace_dir(path).unwrap();
        let opts = if args[1] == "daemon-final-flush" { daemon::DaemonOptions {max_requests:Some(1),idle_exit:None} } else { daemon::DaemonOptions::default() };
        daemon::serve(path, &socket, opts, mmu, dispatch).unwrap();
        return;
    }
    let base = Path::new("/tmp/Infinity-Context").join(format!("state-reproduction-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    let mut mmu = InfinityContextMmu::default();
    let panic_result = std::panic::catch_unwind(|| icx_core::chunker::chunk_bytes(b"one byte", icx_core::chunker::ChunkerConfig {min_size:usize::MAX, avg_size:8192, max_size:65536}));
    println!("{}",json!({"probe":"chunk_config_extreme_min","panicked":panic_result.is_err()}));
    for payload in ["first\r\nlast\r\n", "tail\n", "x ⟪y⟫", "multilingual: 日本語 Ελληνικά\n"] {
        let result = mmu.ingest_payload(AgentKind::GenericMcp,"audit-byte-fixture",payload,false);
        assert_eq!(mmu.cas.deref_bytes(result.artifact_id).unwrap(), payload.as_bytes());
    }
    println!("{}",json!({"probe":"post_redaction_byte_roundtrip","fixtures":4,"passed":true}));
    for (name, request) in [
        ("missing_jsonrpc", json!({"id":1,"method":"ping"})),
        ("wrong_jsonrpc", json!({"jsonrpc":"1.0","id":2,"method":"ping"})),
        ("object_id", json!({"jsonrpc":"2.0","id":{"illegal":1},"method":"ping"})),
        ("missing_op", json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"icx","arguments":{}}})),
        ("invalid_op", json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"icx","arguments":{"op":"TYPO"}}})),
    ] {
        let result = McpPagerServer::handle_jsonrpc(&request.to_string(), &mut mmu);
        println!("{}", json!({"probe":name,"request":request,"response":result}));
    }
    let bus = SeqlockTelemetryBus::new_in_memory();
    bus.try_publish(AgentKind::GenericMcp, ArtifactId(42), 77, 0, [0;4], [0;16], "old coherent payload", 1).unwrap();
    // Model a coherent slot whose sequence is outside the sampled header: no payload tearing.
    bus.in_process_layout().unwrap().slots[0].seqlock.store(514, Ordering::Release);
    let (events, lost) = bus.poll_since_with_loss(0);
    println!("{}",json!({"probe":"bus_version_outside_head","head":bus.write_sequence(),"events":events,"lost":lost}));
    // A shorter existing file is treated as a new fabric and header-reset rather than rejected.
    let fabric = base.join("fabric");
    let b = SeqlockTelemetryBus::open_shared_file(&fabric).unwrap();
    b.try_publish(AgentKind::GenericMcp, ArtifactId(8), 77, 0, [0;4], [0;16], "before-truncate", 1).unwrap();
    drop(b);
    std::fs::OpenOptions::new().write(true).open(&fabric).unwrap().set_len(8192).unwrap();
    let b = SeqlockTelemetryBus::open_shared_file(&fabric).unwrap();
    println!("{}",json!({"probe":"truncated_bus_reinitialized","head":b.write_sequence(),"events":b.poll_since(0)}));
    let ipc_path = base.join("ipc-fabric");
    let ipc = SeqlockTelemetryBus::open_shared_file(&ipc_path).unwrap();
    let mut writer = std::process::Command::new(std::env::current_exe().unwrap()).arg("bus-writer").arg(&ipc_path).spawn().unwrap();
    let mut cursor = 0;
    let mut latencies = Vec::new();
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) && latencies.len() < 100 {
        for ev in ipc.poll_since(cursor) {
            cursor = cursor.max(ev.seq);
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            latencies.push(now.saturating_sub(ev.ahtl_packet.parse::<u128>().unwrap()) as u64);
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    writer.wait().unwrap();
    latencies.sort();
    assert_eq!(latencies.len(),100);
    println!("{}",json!({"probe":"separate_process_bus","events":latencies.len(),"p50_ns":latencies[50],"p99_ns":latencies[99],"max_ns":latencies.last(),"poll_sleep_us":50,"clock":"SystemTime UNIX nanos"}));
    let ws = base.join("workspace");
    let mut initial = InfinityContextMmu::default();
    initial.save_workspace_dir(&ws).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap()).arg("daemon").arg(&ws).spawn().unwrap();
    let socket = ws.join("audit.sock");
    for _ in 0..100 {
        if socket.exists() { break; }
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut stream = UnixStream::connect(socket).unwrap();
    let payload = "audit durable acknowledgment fixture";
    let request = json!({"v":1,"agent":"mcp","event":"PostToolUse","payload":payload});
    writeln!(stream,"{request}").unwrap();
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    let mut recovered = InfinityContextMmu::default();
    recovered.load_workspace_dir(&ws).unwrap();
    let aid = ArtifactId::hash_bytes(payload.as_bytes()).0;
    println!("{}",json!({"probe":"daemon_ack_before_flush_crash","response":serde_json::from_str::<serde_json::Value>(&response).unwrap(),"recovered_artifact":recovered.cas.deref_bytes(aid).is_ok(),"artifact":aid.to_string()}));
    let fail_ws = base.join("flush-fail");
    InfinityContextMmu::default().save_workspace_dir(&fail_ws).unwrap();
    let mut failing = std::process::Command::new(std::env::current_exe().unwrap()).arg("daemon-final-flush").arg(&fail_ws).spawn().unwrap();
    let socket = fail_ws.join("audit.sock");
    for _ in 0..100 {
        if socket.exists() { break; }
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut stream = UnixStream::connect(socket).unwrap();
    let request = json!({"v":1,"agent":"mcp","event":"FAIL_FINAL_FLUSH","payload":fail_ws.to_str().unwrap()});
    writeln!(stream,"{request}").unwrap();
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).unwrap();
    let exit = failing.wait().unwrap();
    let response: serde_json::Value = serde_json::from_str(&response).unwrap();
    std::fs::remove_dir(fail_ws.join("icx.sqlite3")).unwrap();
    std::fs::rename(fail_ws.join("icx.sqlite3.preserved"),fail_ws.join("icx.sqlite3")).unwrap();
    let mut recovered = InfinityContextMmu::default();
    recovered.load_workspace_dir(&fail_ws).unwrap();
    let aid = recovered.resolve_handle(response["out"].as_str().unwrap());
    println!("{}",json!({"probe":"daemon_final_flush_failure","response":response,"daemon_exit_success":exit.success(),"recovered_acknowledged_artifact":aid.is_ok(),"injection":"database path temporarily replaced by a directory after ingest"}));
}
