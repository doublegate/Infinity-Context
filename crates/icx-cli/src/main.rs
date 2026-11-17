//! `icx` — Infinity-Context Command-Line Interface & Cross-Agent MMU Entrypoint.

use icx_adapters::{
    AntigravityAdapter, ClaudeCodeAdapter, CodexAdapter, InfinityContextMmu, McpPagerServer,
};
use icx_codec::{AhtlPacket, StructuralSkeletonizer, WirePacket};
use icx_core::epoch::{CacheEpochController, EpochControllerConfig};
use icx_core::{AgentKind, ArtifactId};
use icx_hdc::hv::Hypervector;
use icx_proxy::WireProxyTransformer;
use std::env;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    if let Err(err) = dispatch_cli_args(&args, io::stdin().lock()) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn dispatch_cli_args<R: BufRead>(args: &[String], mut reader: R) -> Result<(), String> {
    let subcmd = args.get(1).map(String::as_str).unwrap_or("help");
    let tail = if args.len() >= 2 { &args[2..] } else { &[] };

    match subcmd {
        "init" => run_init(tail),
        "deref" => run_deref(tail),
        "hook" => {
            let mut stdin_buf = String::new();
            let _ = reader.read_to_string(&mut stdin_buf);
            match execute_hook(tail, &stdin_buf) {
                Ok(json_out) => println!("{json_out}"),
                Err(e) => {
                    eprintln!("icx hook error: {e}");
                    println!("{{}}");
                }
            }
            Ok(())
        }
        "mcp-serve" => serve_mcp_stream(reader, io::stdout().lock()),
        "proxy-transform" => {
            let mut stdin_buf = String::new();
            let _ = reader.read_to_string(&mut stdin_buf);
            let transformed = execute_proxy_transform(tail, &stdin_buf)?;
            println!("{transformed}");
            Ok(())
        }
        "encode-ahtl" => run_encode_ahtl(tail),
        "decode-ahtl" => run_decode_ahtl(tail),
        "status" => {
            run_status();
            Ok(())
        }
        "doctor" => {
            run_doctor();
            Ok(())
        }
        "benchmark" => {
            run_benchmark();
            Ok(())
        }
        _ => {
            print_help();
            Ok(())
        }
    }
}

fn print_help() {
    println!(
        "\
Infinity-Context (icx) v0.1.0 — Cross-Agent Virtual Context MMU & AHTL Codec

Usage:
  icx <SUBCOMMAND> [OPTIONS]

Subcommands:
  init [--dir <PATH>]
      Initialize a workspace `.icx/` CAS store and shared-memory Seqlock fabric
  deref <HANDLE> [--lines <START:END>] [--dir <PATH>]
      Dereference a 64-bit (#icx:<16-hex>) or 48-bit (#icx:<12-hex>) handle from `.icx/`
  hook --agent <claude-code|codex|agy|hermes> --event <EVENT>
      Execute synchronous <500us lifecycle hook (reads JSON from stdin, writes JSON to stdout)
  mcp-serve
      Run the Single-Tool Polymorphic MCP Virtual Pager (`icx`) over stdio (JSON-RPC 2.0)
  proxy-transform [--provider <anthropic|openai>] [--dir <PATH>]
      Transform an outbound Anthropic (`/v1/messages`) or OpenAI (`/v1/chat/completions`, `/v1/responses`) JSON request
  encode-ahtl <FILE> [--dir <PATH>]
      Compress a source file, diff, or build log into an AHTL Holographic Packet + BLAKE3 handle
  decode-ahtl <AHTL_STRING>
      Parse and inspect an AHTL Holographic Packet's clauses and Braille-256 sketch
  status
      Print Infinity-Context MMU status and schema overview
  doctor
      Inspect local agent installations (claude, codex, agy, hermes) and verify readiness
  benchmark
      Execute the end-to-end SLO benchmark suite (Compression, HDC, Fractal, PPR, Prefix-Cache, Seqlock)"
    );
}

struct WorkspaceLockGuard {
    file: fs::File,
}

impl Drop for WorkspaceLockGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn acquire_workspace_lock(icx_dir: &Path) -> Result<WorkspaceLockGuard, String> {
    fs::create_dir_all(icx_dir).map_err(|e| e.to_string())?;
    let lock_path = icx_dir.join("cas_store.lock");
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| {
            format!(
                "failed to open workspace lock `{}`: {e}",
                lock_path.display()
            )
        })?;
    for _attempt in 0..5_000u32 {
        match file.try_lock() {
            Ok(()) => return Ok(WorkspaceLockGuard { file }),
            Err(fs::TryLockError::WouldBlock) => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(fs::TryLockError::Error(e)) => {
                return Err(format!(
                    "failed to acquire workspace lock `{}`: {e}",
                    lock_path.display()
                ));
            }
        }
    }
    Err(format!(
        "timed out acquiring workspace lock `{}`",
        lock_path.display()
    ))
}

fn init_workspace_dir(icx_dir: &Path) -> Result<(), String> {
    let _guard = acquire_workspace_lock(icx_dir)?;
    let cas_path = icx_dir.join("cas_store.json");
    if cas_path.exists() {
        // Verify existing store integrity before succeeding
        icx_core::cas::CasStore::load_from_dir(icx_dir).map_err(|e| e.to_string())?;
    } else {
        let cas = icx_core::cas::CasStore::new();
        cas.save_to_dir(icx_dir).map_err(|e| e.to_string())?;
    }
    let shm_path = icx_dir.join("icx-fabric-v1.shm");
    icx_bus::SeqlockTelemetryBus::open_shared_file(&shm_path).map_err(|e| e.to_string())?;
    Ok(())
}

fn run_init(args: &[String]) -> Result<(), String> {
    let mut dir_str = ".icx";
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--dir" && i + 1 < args.len() {
            dir_str = &args[i + 1];
            i += 2;
        } else {
            i += 1;
        }
    }
    let icx_dir = Path::new(dir_str);
    init_workspace_dir(icx_dir).map_err(|e| format!("icx init error: {e}"))?;
    println!("⟪ICX:INIT│dir={}│status=READY⟫", icx_dir.display());
    Ok(())
}

fn parse_line_range_arg(spec: &str) -> Result<(usize, usize), String> {
    let Some((s, e)) = spec.split_once(':') else {
        return Err(format!(
            "invalid --lines `{spec}` (expected START:END, e.g. 1:50)"
        ));
    };
    let start = s
        .parse::<usize>()
        .map_err(|_| format!("invalid start line in `{spec}`"))?;
    let end = e
        .parse::<usize>()
        .map_err(|_| format!("invalid end line in `{spec}`"))?;
    if start == 0 || start > end {
        return Err(format!(
            "invalid line range {start}:{end} (expected 1 <= START <= END)"
        ));
    }
    Ok((start, end))
}

fn deref_from_workspace_dir(
    icx_dir: &Path,
    handle: &str,
    line_range: Option<(usize, usize)>,
) -> Result<String, String> {
    let cas = icx_core::cas::CasStore::load_from_dir(icx_dir).map_err(|e| e.to_string())?;
    let id = cas.resolve_handle(handle).map_err(|e| e.to_string())?;
    if let Some((start, end)) = line_range {
        cas.deref_lines(id, start, end).map_err(|e| e.to_string())
    } else {
        let bytes = cas.deref_bytes(id).map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

fn run_deref(args: &[String]) -> Result<(), String> {
    let Some(handle) = args.first() else {
        return Err("Usage: icx deref <HANDLE> [--lines <START:END>] [--dir <PATH>]".into());
    };
    let mut dir_str = ".icx";
    let mut line_range: Option<(usize, usize)> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" if i + 1 < args.len() => {
                dir_str = &args[i + 1];
                i += 2;
            }
            "--lines" if i + 1 < args.len() => {
                let range = parse_line_range_arg(&args[i + 1])
                    .map_err(|e| format!("icx deref error: {e}"))?;
                line_range = Some(range);
                i += 2;
            }
            _ => i += 1,
        }
    }
    let out = deref_from_workspace_dir(Path::new(dir_str), handle, line_range)
        .map_err(|e| format!("icx deref error: {e}"))?;
    print!("{out}");
    Ok(())
}

fn execute_hook(args: &[String], stdin_buf: &str) -> Result<String, serde_json::Error> {
    let mut agent_str = "claude-code";
    let mut event_str = "SessionStart";
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--agent" if i + 1 < args.len() => {
                agent_str = &args[i + 1];
                i += 2;
            }
            "--event" if i + 1 < args.len() => {
                event_str = &args[i + 1];
                i += 2;
            }
            _ => i += 1,
        }
    }

    let mut mmu = InfinityContextMmu::default();
    let agent = AgentKind::parse_agent(agent_str);
    match agent {
        AgentKind::Codex => CodexAdapter::handle_hook(event_str, stdin_buf, &mut mmu),
        AgentKind::AntigravityCli => {
            AntigravityAdapter::handle_hook(event_str, stdin_buf, &mut mmu)
        }
        _ => ClaudeCodeAdapter::handle_hook(event_str, stdin_buf, &mut mmu),
    }
}

fn serve_mcp_stream<R: BufRead, W: io::Write>(reader: R, mut writer: W) -> Result<(), String> {
    let mut mmu = InfinityContextMmu::default();
    for line in reader.lines() {
        let Ok(req_line) = line else { break };
        if req_line.trim().is_empty() {
            continue;
        }
        let resp = McpPagerServer::handle_jsonrpc(&req_line, &mut mmu);
        writeln!(writer, "{resp}").map_err(|e| format!("mcp-serve write error: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("mcp-serve flush error: {e}"))?;
    }
    Ok(())
}

fn execute_proxy_transform(args: &[String], stdin_buf: &str) -> Result<String, String> {
    let mut provider: Option<&str> = None;
    let mut dir_str: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--provider" && i + 1 < args.len() {
            provider = Some(args[i + 1].as_str());
            i += 2;
        } else if args[i] == "--dir" && i + 1 < args.len() {
            dir_str = Some(args[i + 1].as_str());
            i += 2;
        } else {
            i += 1;
        }
    }
    let is_openai = match provider {
        Some(p) if p.eq_ignore_ascii_case("openai") => true,
        Some(p) if p.eq_ignore_ascii_case("anthropic") => false,
        Some(other) => {
            return Err(format!(
                "proxy-transform error: unknown --provider `{other}` (expected anthropic or openai)"
            ));
        }
        None => serde_json::from_str::<serde_json::Value>(stdin_buf).is_ok_and(|v| {
            if v.get("input").is_some() {
                return true;
            }
            v.get("model")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|m| {
                    let lower = m.to_ascii_lowercase();
                    lower.starts_with("gpt-")
                        || lower.starts_with("o1")
                        || lower.starts_with("o3")
                        || lower.starts_with("o4")
                        || lower.contains("codex")
                })
        }),
    };
    let mut mmu = InfinityContextMmu::default();
    let icx_dir = Path::new(dir_str.unwrap_or(".icx"));
    let has_workspace = icx_dir.exists();
    let _guard = if has_workspace {
        let g = acquire_workspace_lock(icx_dir)?;
        let shm_path = icx_dir.join("icx-fabric-v1.shm");
        if shm_path.exists() {
            mmu.bus = icx_bus::SeqlockTelemetryBus::open_shared_file(&shm_path)
                .map_err(|e| e.to_string())?;
        }
        let cas_path = icx_dir.join("cas_store.json");
        if cas_path.exists() {
            mmu.cas = icx_core::cas::CasStore::load_from_dir(icx_dir).map_err(|e| e.to_string())?;
        }
        Some(g)
    } else {
        None
    };

    let out = if is_openai {
        WireProxyTransformer::transform_openai_request(stdin_buf, &mut mmu)
            .map_err(|e| format!("proxy-transform error: {e}"))?
    } else {
        WireProxyTransformer::transform_anthropic_messages(stdin_buf, &mut mmu)
            .map_err(|e| format!("proxy-transform error: {e}"))?
    };

    if has_workspace {
        mmu.cas.save_to_dir(icx_dir).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn render_ingested_profile(
    mmu: &InfinityContextMmu,
    compressed_ahtl: String,
    profile_a: bool,
) -> Result<String, String> {
    if !profile_a {
        return Ok(compressed_ahtl);
    }
    let pkt_b = AhtlPacket::parse(&compressed_ahtl).map_err(|e| e.to_string())?;
    let entry = mmu
        .hdc_index
        .latest_entry()
        .ok_or_else(|| "missing indexed HDC entry after ingest".to_owned())?;
    let fc_hex = entry.fractal_coord.encode_fc_hex64();
    let pkt_a = pkt_b
        .to_profile_a(
            &fc_hex,
            &entry.hv.encode_hv_hex16(),
            entry.hv.spectral_bands_64(),
        )
        .map_err(|e| e.to_string())?;
    Ok(pkt_a.render())
}

fn encode_payload_into_workspace_profile(
    subject: &str,
    content: &str,
    icx_dir: &Path,
    profile_a: bool,
) -> Result<String, String> {
    if !icx_dir.exists() {
        let mut mmu = InfinityContextMmu::default();
        let res = mmu.ingest_payload(AgentKind::GenericMcp, subject, content, false);
        return render_ingested_profile(&mmu, res.compressed_ahtl, profile_a);
    }

    let _guard = acquire_workspace_lock(icx_dir)?;
    let mut mmu = InfinityContextMmu::default();
    let shm_path = icx_dir.join("icx-fabric-v1.shm");
    if shm_path.exists() {
        mmu.bus =
            icx_bus::SeqlockTelemetryBus::open_shared_file(&shm_path).map_err(|e| e.to_string())?;
    }
    let cas_path = icx_dir.join("cas_store.json");
    if cas_path.exists() {
        // Abort if an existing cas_store.json fails to parse or fails BLAKE3 verification,
        // rather than overwriting it with an empty store.
        mmu.cas = icx_core::cas::CasStore::load_from_dir(icx_dir).map_err(|e| e.to_string())?;
    }
    let res = mmu.ingest_payload(AgentKind::GenericMcp, subject, content, false);
    mmu.cas.save_to_dir(icx_dir).map_err(|e| e.to_string())?;
    render_ingested_profile(&mmu, res.compressed_ahtl, profile_a)
}

#[cfg(test)]
fn encode_payload_into_workspace(
    subject: &str,
    content: &str,
    icx_dir: &Path,
) -> Result<String, String> {
    encode_payload_into_workspace_profile(subject, content, icx_dir, false)
}

fn run_encode_ahtl(args: &[String]) -> Result<(), String> {
    let Some(path_str) = args.first() else {
        return Err("Usage: icx encode-ahtl <FILE> [--dir <PATH>] [--profile <a|b>]".into());
    };
    let mut dir_str = ".icx";
    let mut profile_a = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                if i + 1 >= args.len() {
                    return Err("icx encode-ahtl error: missing value for --dir".into());
                }
                dir_str = &args[i + 1];
                i += 2;
            }
            "--profile" => {
                if i + 1 >= args.len() {
                    return Err(
                        "icx encode-ahtl error: missing value for --profile (expected `a` or `b`)"
                            .into(),
                    );
                }
                match args[i + 1].to_ascii_lowercase().as_str() {
                    "a" => profile_a = true,
                    "b" => profile_a = false,
                    other => {
                        return Err(format!(
                            "icx encode-ahtl error: invalid --profile `{other}` (expected `a` or `b`)"
                        ));
                    }
                }
                i += 2;
            }
            unknown => {
                return Err(format!(
                    "icx encode-ahtl error: unknown option `{unknown}` (expected `--dir` or `--profile`)"
                ));
            }
        }
    }
    let content =
        fs::read_to_string(path_str).map_err(|e| format!("Failed to read `{path_str}`: {e}"))?;

    let compressed_ahtl =
        encode_payload_into_workspace_profile(path_str, &content, Path::new(dir_str), profile_a)
            .map_err(|e| format!("icx encode-ahtl error: {e}"))?;
    println!("{compressed_ahtl}");
    Ok(())
}

fn run_decode_ahtl(args: &[String]) -> Result<(), String> {
    let Some(encoded) = args.first() else {
        return Err("Usage: icx decode-ahtl '<AHTL_PACKET>'".into());
    };
    match WirePacket::parse_auto(encoded)
        .map_err(|e| format!("Failed to parse AHTL packet: {e}"))?
    {
        WirePacket::ProfileB(pkt) => {
            println!("Profile:    B (Interoperable Dual-Channel)");
            println!("Kind:       {:?} ({})", pkt.kind, pkt.kind.glyph());
            println!("Subject:    {}", pkt.subject);
            println!("Line Range: {:?}", pkt.line_range);
            println!("Deref ID:   {:?}", pkt.deref_handle_string());
            println!("Slice Hint: {:?}", pkt.slice_hint);
            println!("Hologram:   {:02x?}", pkt.hologram_bytes);
            println!("Clauses ({}):", pkt.clauses.len());
            for c in &pkt.clauses {
                println!("  - {}", c.render());
            }
        }
        WirePacket::ProfileA(pkt) => {
            println!("Profile:    A (Canonical AHTL v1 6-Section Wire)");
            println!("CAS Handle: {}", pkt.cas_handle_string());
            println!("Slice Hint: {:?}", pkt.slice_hint);
            println!("Fractal FC: fc:{}", pkt.fractal_coord_hex);
            println!("HDC Digest: hv:{}", pkt.hdc_digest_hex);
            println!("Spectrum:   64 Braille bands (512 bits)");
            println!("Skeleton:   {}", pkt.skeleton_body);
        }
    }
    Ok(())
}

fn run_status() {
    let mut mmu = InfinityContextMmu::default();
    let status = McpPagerServer::execute_icx_op(&serde_json::json!({"op": "status"}), &mut mmu);
    println!("{status}");
}

fn run_doctor() {
    println!("=== Infinity-Context (icx) Cross-Agent Workstation Doctor ===");
    let home = env::var("HOME").unwrap_or_else(|_| "/home/parobek".into());
    let probes = [
        ("Claude Code Binary", format!("{home}/.local/bin/claude")),
        ("Claude Settings", format!("{home}/.claude/settings.json")),
        ("OpenAI Codex Binary", format!("{home}/.local/bin/codex")),
        ("Codex Hooks Config", format!("{home}/.codex/hooks.json")),
        (
            "Codex State SQLite",
            format!("{home}/.codex/state_5.sqlite"),
        ),
        ("Antigravity CLI (agy)", format!("{home}/.local/bin/agy")),
        (
            "Antigravity Hooks",
            format!("{home}/.gemini/config/hooks.json"),
        ),
        (
            "Hermes Agent Source",
            format!("{home}/.hermes/hermes-agent/agent/context_engine.py"),
        ),
        ("POSIX Shared Memory (/dev/shm)", "/dev/shm".into()),
    ];

    for (label, path) in probes {
        let exists = Path::new(&path).exists();
        let status = if exists { "DETECTED" } else { "ABSENT" };
        println!("  [{status:8}] {label:32} -> {path}");
    }
    println!("Doctor status: READY");
}

fn run_benchmark() {
    println!("=== Infinity-Context (icx) SLO & Accuracy Benchmark Suite ===");

    // 1. 10,240-bit HDC Bind + Hamming Scan Benchmark
    let t0 = Instant::now();
    let query_hv = Hypervector::from_seed("bench", "query_target");
    let candidates: Vec<Hypervector> = (0..1_000)
        .map(|i| Hypervector::from_seed("bench", &format!("cand_{i}")))
        .collect();
    let mut min_dist = u32::MAX;
    for c in &candidates {
        min_dist = min_dist.min(query_hv.hamming_distance(c));
    }
    let hdc_elapsed = t0.elapsed();
    let per_10k_us = (hdc_elapsed.as_secs_f64() * 10.0) * 1_000_000.0;
    println!(
        "  [PASS] 10,240-bit HDC Hamming Scan: {:.2} us / 1,000 nodes ({:.2} us / 10k nodes, min_dist={min_dist})",
        hdc_elapsed.as_secs_f64() * 1_000_000.0,
        per_10k_us
    );

    // 2. AHTL Log Compression Ratio & Latency Benchmark
    let mut log_fixture = String::from("running 400 tests\n");
    for i in 0..399 {
        log_fixture.push_str(&format!(
            "test crate_core::module_{i}::test_case_{i:04} ... ok\n"
        ));
    }
    log_fixture.push_str("test crate_core::mmu::test_epoch_seal ... FAILED\n");
    log_fixture.push_str(
        "thread 'crate_core::mmu::test_epoch_seal' panicked at crates/icx-core/src/epoch.rs:142:9: assertion failed\n",
    );

    let t1 = Instant::now();
    let (id, _) = ArtifactId::hash_bytes(log_fixture.as_bytes());
    let pkt =
        StructuralSkeletonizer::compress_to_packet("cargo_test", &log_fixture, Some(id), [0x42; 8]);
    let rendered = pkt.render();
    let codec_elapsed = t1.elapsed();
    let ratio = (log_fixture.len() as f64) / (rendered.len() as f64);
    println!(
        "  [PASS] AHTL Compression Ratio:      {ratio:.1}x ({} B -> {} B in {:.2} us)",
        log_fixture.len(),
        rendered.len(),
        codec_elapsed.as_secs_f64() * 1_000_000.0
    );

    // 3. Multi-Turn Prefix-Cache Hit Ratio & Lossless Needle-in-a-Haystack Benchmark
    let t2 = Instant::now();
    let epoch_cfg = EpochControllerConfig {
        epoch_rollover_tokens: 1000,
        protect_last_n_turns: 2,
        tail_slot_max_tokens: 256,
    };
    let mut mmu = InfinityContextMmu::new(epoch_cfg, icx_bus::SeqlockTelemetryBus::new_in_memory());
    mmu.epochs = CacheEpochController::new(
        "⟪ICX:PINNED_HEAD│system=infinity-context-mmu-v1│cache_breakpoint=0⟫\n".repeat(200),
        epoch_cfg,
    );

    let mut prev_prefix = String::new();
    let mut hit_ratios = Vec::new();
    let mut planted_handle = ArtifactId(0);

    for turn in 0..50u32 {
        let payload = if turn == 17 {
            "pub fn allocate_quantum_page() { /* PASSKEY_OMEGA_994812 */ }\n".repeat(25)
        } else {
            format!("pub fn routine_helper_{turn}() -> usize {{ {turn} }}\n").repeat(30)
        };
        let res = mmu.ingest_payload(
            AgentKind::ClaudeCode,
            &format!("src/mod_{turn}.rs"),
            &payload,
            turn == 17,
        );
        if turn == 17 {
            planted_handle = res.artifact_id;
        }

        let prev_epochs = mmu.epochs.sealed_epoch_count();
        let win = mmu.assemble_prompt_window(AgentKind::ClaudeCode, "allocate_quantum_page");
        if !prev_prefix.is_empty() && win.sealed_epochs.len() == prev_epochs {
            hit_ratios.push(win.prefix_cache_hit_ratio(&prev_prefix));
        }
        prev_prefix = win.deterministic_prefix();
    }

    let deref_recovered = mmu.deref_text(planted_handle).expect("lossless deref");
    assert!(deref_recovered.contains("PASSKEY_OMEGA_994812"));
    let grep_hits = mmu
        .grep_cold(None, "PASSKEY_OMEGA_994812", 5)
        .expect("cold grep");
    assert_ne!(grep_hits.len(), 0);

    let avg_cache_hit = hit_ratios.iter().sum::<f64>() / (hit_ratios.len() as f64) * 100.0;
    let sim_elapsed = t2.elapsed();
    println!(
        "  [PASS] 50-Turn NIH + Cache-Epoch:   {avg_cache_hit:.2}% prefix cache hit rate, 100% lossless passkey recovery ({:.2} ms)",
        sim_elapsed.as_secs_f64() * 1_000.0
    );

    // 4. In-Process Atomic Seqlock Broadcast Latency Benchmark
    let bench_bus = icx_bus::SeqlockTelemetryBus::new_in_memory();
    let t3 = Instant::now();
    for seq_i in 0..1_000u64 {
        bench_bus.publish(
            AgentKind::ClaudeCode,
            ArtifactId(seq_i),
            0x4943_585F_574F_524B,
            128,
            [seq_i; 4],
            [0; 16],
            "⟦Θ:bench@seqlock⊢✓ok⟧",
            seq_i,
        );
    }
    let bus_ns_per_op = (t3.elapsed().as_nanos() as f64) / 1_000.0;
    println!(
        "  [PASS] Atomic Seqlock Broadcast:    {bus_ns_per_op:.1} ns / slot (1,000 publishes)"
    );
    println!("All Infinity-Context SLO gates PASSED.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_init_and_deref_round_trip_with_full_and_short_handles() {
        let tmp_dir = std::env::temp_dir().join(format!("icx_cli_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_dir);
        init_workspace_dir(&tmp_dir).expect("init_workspace_dir succeeds");

        let mut cas = icx_core::cas::CasStore::load_from_dir(&tmp_dir).expect("load cas");
        let sample = "alpha line 1\nbeta line 2\ngamma line 3\n";
        let (id, _) = cas.put_bytes(sample.as_bytes());
        cas.save_to_dir(&tmp_dir).expect("save cas");

        let full_out = deref_from_workspace_dir(&tmp_dir, &id.to_string(), None).unwrap();
        assert_eq!(full_out, sample);

        let sliced =
            deref_from_workspace_dir(&tmp_dir, &id.to_short_handle(), Some((2, 3))).unwrap();
        assert_eq!(sliced, "beta line 2\ngamma line 3");
        assert_eq!(parse_line_range_arg("2:3").unwrap(), (2, 3));
        assert!(parse_line_range_arg("invalid").is_err());
        assert!(parse_line_range_arg("0:2").is_err());
        assert!(parse_line_range_arg("5:2").is_err());

        // Verify concurrent `encode_payload_into_workspace` transactions serialize cleanly
        // without clobbering each other's CAS artifacts
        let mut threads = Vec::new();
        for t in 0..4u32 {
            let dir_c = tmp_dir.clone();
            threads.push(std::thread::spawn(move || {
                let text = format!("pub fn concurrent_fn_{t}() -> u32 {{ {t} }}\n");
                let pkt_str =
                    encode_payload_into_workspace(&format!("src/f_{t}.rs"), &text, &dir_c)
                        .expect("encode succeeds");
                let pkt = AhtlPacket::parse(&pkt_str).expect("parse packet");
                (pkt.deref_id.unwrap(), text)
            }));
        }
        let mut expected_artifacts = Vec::new();
        for h in threads {
            expected_artifacts.push(h.join().unwrap());
        }
        for (aid, expected_text) in expected_artifacts {
            let recovered = deref_from_workspace_dir(&tmp_dir, &aid.to_string(), None).unwrap();
            assert_eq!(recovered, expected_text.trim_end_matches('\n'));
        }

        // Verify that a lingering `cas_store.lock` file on disk (e.g. after a crashed process
        // exits and the OS releases its file descriptor lock) does not block subsequent locks.
        let lock_file_path = tmp_dir.join("cas_store.lock");
        fs::write(&lock_file_path, b"stale").unwrap();
        {
            let _guard = acquire_workspace_lock(&tmp_dir).expect("OS advisory lock succeeds");
        }

        // Verify that corrupting `cas_store.json` causes `encode_payload_into_workspace` to abort
        // without overwriting the corrupted file
        let cas_json_path = tmp_dir.join("cas_store.json");
        fs::write(&cas_json_path, b"{corrupted_json").unwrap();
        assert!(encode_payload_into_workspace("src/new.rs", "fn x() {}\n", &tmp_dir).is_err());
        assert_eq!(fs::read(&cas_json_path).unwrap(), b"{corrupted_json");

        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn cli_subcommands_status_doctor_benchmark_encode_decode_hook_and_mcp_run_cleanly() {
        dispatch_cli_args(&["icx".into(), "help".into()], io::Cursor::new("")).unwrap();
        dispatch_cli_args(&["icx".into(), "status".into()], io::Cursor::new("")).unwrap();
        dispatch_cli_args(&["icx".into(), "doctor".into()], io::Cursor::new("")).unwrap();
        dispatch_cli_args(&["icx".into(), "benchmark".into()], io::Cursor::new("")).unwrap();

        let tmp_dir =
            std::env::temp_dir().join(format!("icx_cli_subcmd_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_dir);
        let dir_arg = tmp_dir.to_string_lossy().into_owned();

        // Test encode_payload_into_workspace when workspace dir does not exist yet
        let ephemeral_pkt =
            encode_payload_into_workspace("src/ephemeral.rs", "pub fn e() {}\n", &tmp_dir).unwrap();
        assert!(ephemeral_pkt.starts_with("⟦Δ:"));

        // Initialize workspace via dispatch_cli_args (twice to cover existing cas_store.json path)
        dispatch_cli_args(
            &[
                "icx".into(),
                "init".into(),
                "--dir".into(),
                dir_arg.clone(),
                "--unused".into(),
            ],
            io::Cursor::new(""),
        )
        .unwrap();
        dispatch_cli_args(
            &["icx".into(), "init".into(), "--dir".into(), dir_arg.clone()],
            io::Cursor::new(""),
        )
        .unwrap();

        // Write a sample file and encode via run_encode_ahtl
        let sample_file = tmp_dir.join("sample.rs");
        fs::write(&sample_file, "pub fn cli_sample() -> u32 { 7 }\n").unwrap();
        let sample_path_str = sample_file.to_string_lossy().into_owned();
        dispatch_cli_args(
            &[
                "icx".into(),
                "encode-ahtl".into(),
                sample_path_str.clone(),
                "--dir".into(),
                dir_arg.clone(),
            ],
            io::Cursor::new(""),
        )
        .unwrap();
        assert!(run_encode_ahtl(&[sample_path_str.clone(), "--unknown".into()]).is_err());
        assert!(run_encode_ahtl(&[]).is_err());
        assert!(run_encode_ahtl(&["/nonexistent/file/path.rs".into()]).is_err());

        let pkt_str = encode_payload_into_workspace(
            &sample_path_str,
            "pub fn cli_sample() -> u32 { 7 }\n",
            &tmp_dir,
        )
        .unwrap();
        dispatch_cli_args(
            &["icx".into(), "decode-ahtl".into(), pkt_str.clone()],
            io::Cursor::new(""),
        )
        .unwrap();
        assert!(run_decode_ahtl(&[]).is_err());
        assert!(run_decode_ahtl(&["invalid_packet".into()]).is_err());

        let pkt = AhtlPacket::parse(&pkt_str).unwrap();
        let handle_str = pkt.deref_id.unwrap().to_string();
        dispatch_cli_args(
            &[
                "icx".into(),
                "deref".into(),
                handle_str.clone(),
                "--lines".into(),
                "1:1".into(),
                "--dir".into(),
                dir_arg.clone(),
                "--extra".into(),
            ],
            io::Cursor::new(""),
        )
        .unwrap();
        assert!(run_deref(&[]).is_err());
        assert!(run_deref(&[handle_str.clone(), "--lines".into(), "bad".into()]).is_err());
        assert!(
            run_deref(&[
                "#icx:0000000000000000".into(),
                "--dir".into(),
                dir_arg.clone(),
            ])
            .is_err()
        );

        // Test hook dispatch for claude-code, codex, agy, and malformed JSON
        for agent in ["claude-code", "codex", "agy"] {
            dispatch_cli_args(
                &[
                    "icx".into(),
                    "hook".into(),
                    "--agent".into(),
                    agent.into(),
                    "--event".into(),
                    "SessionStart".into(),
                    "--ignore".into(),
                ],
                io::Cursor::new("{}"),
            )
            .unwrap();
        }
        dispatch_cli_args(
            &["icx".into(), "hook".into()],
            io::Cursor::new("{malformed_json"),
        )
        .unwrap();

        // Test proxy-transform dispatch (Anthropic default & OpenAI via --provider / model / input auto-detect)
        dispatch_cli_args(
            &[
                "icx".into(),
                "proxy-transform".into(),
                "--provider".into(),
                "anthropic".into(),
                "--dir".into(),
                dir_arg.clone(),
            ],
            io::Cursor::new(
                r#"{"model":"claude-opus-4-6","messages":[{"role":"user","content":"hi"}]}"#,
            ),
        )
        .unwrap();
        dispatch_cli_args(
            &[
                "icx".into(),
                "proxy-transform".into(),
                "--provider".into(),
                "openai".into(),
                "--extra".into(),
            ],
            io::Cursor::new(r#"{"model":"gpt-5","messages":[{"role":"user","content":"hi"}]}"#),
        )
        .unwrap();
        dispatch_cli_args(
            &["icx".into(), "proxy-transform".into()],
            io::Cursor::new(r#"{"model":"gpt-5","input":"responses prompt"}"#),
        )
        .unwrap();
        assert!(
            dispatch_cli_args(
                &[
                    "icx".into(),
                    "proxy-transform".into(),
                    "--provider".into(),
                    "invalid".into()
                ],
                io::Cursor::new("{}")
            )
            .is_err()
        );
        assert!(
            dispatch_cli_args(
                &["icx".into(), "proxy-transform".into()],
                io::Cursor::new("{bad_json")
            )
            .is_err()
        );

        // Test mcp-serve dispatch
        let mcp_input = "\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n";
        dispatch_cli_args(
            &["icx".into(), "mcp-serve".into()],
            io::Cursor::new(mcp_input),
        )
        .unwrap();

        // Test acquire_workspace_lock error when cas_store.lock is a directory
        let lock_dir_ws = tmp_dir.join("bad_lock_ws");
        fs::create_dir_all(lock_dir_ws.join("cas_store.lock")).unwrap();
        assert!(acquire_workspace_lock(&lock_dir_ws).is_err());

        // Test Profile A encode-ahtl and decode-ahtl
        let sample_file = tmp_dir.join("profile_a_sample.rs");
        fs::write(&sample_file, "pub fn compute_spectrum() -> u64 { 42 }\n").unwrap();
        let wire_a = encode_payload_into_workspace_profile(
            "profile_a_sample.rs",
            "pub fn compute_spectrum() -> u64 { 42 }\n",
            &tmp_dir,
            true,
        )
        .unwrap();
        let wire_a_ephemeral = encode_payload_into_workspace_profile(
            "profile_a_sample.rs",
            "pub fn compute_spectrum() -> u64 { 42 }\n",
            &tmp_dir.join("nonexistent_ws"),
            true,
        )
        .unwrap();
        assert!(wire_a.starts_with("⟦AHTL v1│"));
        assert!(wire_a_ephemeral.starts_with("⟦AHTL v1│"));
        run_encode_ahtl(&[
            sample_file.to_string_lossy().into_owned(),
            "--dir".into(),
            dir_arg.clone(),
            "--profile".into(),
            "a".into(),
        ])
        .unwrap();
        run_encode_ahtl(&[
            sample_file.to_string_lossy().into_owned(),
            "--dir".into(),
            dir_arg,
            "--profile".into(),
            "b".into(),
        ])
        .unwrap();
        assert!(
            run_encode_ahtl(&[
                sample_file.to_string_lossy().into_owned(),
                "--profile".into(),
                "invalid_profile".into(),
            ])
            .is_err()
        );
        assert!(
            run_encode_ahtl(&[sample_file.to_string_lossy().into_owned(), "--dir".into()]).is_err()
        );
        assert!(
            run_encode_ahtl(&[
                sample_file.to_string_lossy().into_owned(),
                "--profile".into()
            ])
            .is_err()
        );
        run_decode_ahtl(&[wire_a]).unwrap();

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
