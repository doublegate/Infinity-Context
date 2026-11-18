//! `icx` — Infinity-Context Command-Line Interface & Cross-Agent MMU Entrypoint.

use icx_adapters::{
    AntigravityAdapter, ClaudeCodeAdapter, CodexAdapter, HermesAdapter, InfinityContextMmu,
    McpPagerServer,
};
use icx_codec::{AhtlPacket, StructuralSkeletonizer, WirePacket};
use icx_core::epoch::{CacheEpochController, EpochControllerConfig};
use icx_core::{AgentKind, ArtifactId};
use icx_hdc::hv::Hypervector;
use icx_proxy::{ProviderFormat, WireProxyTransformer};
use std::env;
use std::fs;
use std::io::{self, BufRead};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
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
        "mcp-serve" => serve_mcp_stream(tail, reader, io::stdout().lock()),
        "proxy" => run_proxy_server(tail),
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
            run_status(tail);
            Ok(())
        }
        "doctor" => run_doctor(tail),
        "benchmark" => run_benchmark(),
        _ => {
            print_help();
            Ok(())
        }
    }
}

fn print_help() {
    println!(
        "\
Infinity-Context (icx) v{} — Cross-Agent Virtual Context MMU & AHTL Codec

Usage:
  icx <SUBCOMMAND> [OPTIONS]

Subcommands:",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "\
  init [--dir <PATH>]
      Initialize a workspace `.icx/` CAS store and shared-memory Seqlock fabric
  deref <HANDLE> [--lines <START:END>] [--dir <PATH>]
      Dereference a 64-bit (#icx:<16-hex>) or 48-bit (#icx:<12-hex>) handle from `.icx/`
  hook --agent <claude-code|codex|agy|hermes> --event <EVENT> [--dir <PATH>]
      Execute synchronous <500us lifecycle hook (reads JSON from stdin, writes JSON to stdout)
  mcp-serve [--dir <PATH>]
      Run the Single-Tool Polymorphic MCP Virtual Pager (`icx`) over stdio (JSON-RPC 2.0)
  proxy [--listen <HOST:PORT>] [--provider <anthropic|openai|gemini>] [--upstream <URL>] [--dir <PATH>] [--max-requests <N>]
      Run the Cache-Epoch HTTP/1.1 & SSE reverse proxy (default 127.0.0.1:49380)
  proxy-transform [--provider <anthropic|openai|gemini>] [--dir <PATH>]
      Transform an outbound Anthropic (`/v1/messages`), OpenAI (`/v1/chat/completions`, `/v1/responses`), or Gemini (`/v1beta/models/*`) JSON request
  encode-ahtl <FILE> [--dir <PATH>]
      Compress a source file, diff, or build log into an AHTL Holographic Packet + BLAKE3 handle
  decode-ahtl <AHTL_STRING>
      Parse and inspect an AHTL Holographic Packet's clauses and Braille-256 sketch
  status [--dir <PATH>]
      Print Infinity-Context MMU status and schema overview
  doctor [--dir <PATH>]
      Inspect local agent installations (claude, codex, agy, hermes), /dev/shm, and workspace integrity
  benchmark
      Execute the end-to-end 9-SLO benchmark suite (HDC, Fractal, PPR, AHTL, FastCDC, Seqlock, Hook, NIH)"
    );
}

fn acquire_workspace_lock(
    icx_dir: &Path,
) -> Result<(fs::File, std::sync::MutexGuard<'static, ()>), String> {
    InfinityContextMmu::acquire_workspace_lock(icx_dir).map_err(|e| e.to_string())
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
    let shm_path = env::var("ICX_SHM_PATH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map_or_else(|| icx_dir.join("icx-fabric-v1.shm"), PathBuf::from);
    icx_bus::SeqlockTelemetryBus::open_shared_file(&shm_path).map_err(|e| e.to_string())?;
    Ok(())
}

fn resolve_workspace_dir(cli_dir: Option<&str>) -> PathBuf {
    if let Some(d) = cli_dir.map(str::trim).filter(|s| !s.is_empty()) {
        return PathBuf::from(d);
    }
    if let Ok(env_d) = env::var("ICX_DIR") {
        let trimmed = env_d.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    PathBuf::from(".icx")
}

fn parse_dir_flag(args: &[String]) -> Option<&str> {
    let mut dir_str: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--dir" && i + 1 < args.len() {
            dir_str = Some(args[i + 1].as_str());
            i += 2;
        } else {
            i += 1;
        }
    }
    dir_str
}

fn run_init(args: &[String]) -> Result<(), String> {
    let icx_dir = resolve_workspace_dir(parse_dir_flag(args));
    init_workspace_dir(&icx_dir).map_err(|e| format!("icx init error: {e}"))?;
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
    let mut mmu = InfinityContextMmu::default();
    mmu.load_workspace_dir(icx_dir).map_err(|e| e.to_string())?;
    let id = mmu.resolve_handle(handle).map_err(|e| e.to_string())?;
    if let Some((start, end)) = line_range {
        mmu.deref_lines(id, start, end).map_err(|e| e.to_string())
    } else {
        mmu.deref_text(id).map_err(|e| e.to_string())
    }
}

fn run_deref(args: &[String]) -> Result<(), String> {
    let Some(handle) = args.first() else {
        return Err("Usage: icx deref <HANDLE> [--lines <START:END>] [--dir <PATH>]".into());
    };
    let mut dir_str: Option<&str> = None;
    let mut line_range: Option<(usize, usize)> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" if i + 1 < args.len() => {
                dir_str = Some(args[i + 1].as_str());
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
    let icx_dir = resolve_workspace_dir(dir_str);
    let out = deref_from_workspace_dir(&icx_dir, handle, line_range)
        .map_err(|e| format!("icx deref error: {e}"))?;
    print!("{out}");
    Ok(())
}

fn dispatch_agent_hook(
    agent: AgentKind,
    event_str: &str,
    stdin_buf: &str,
    mmu: &mut InfinityContextMmu,
) -> Result<String, serde_json::Error> {
    match agent {
        AgentKind::Codex => CodexAdapter::handle_hook(event_str, stdin_buf, mmu),
        AgentKind::AntigravityCli => AntigravityAdapter::handle_hook(event_str, stdin_buf, mmu),
        AgentKind::Hermes => HermesAdapter::handle_hook(event_str, stdin_buf, mmu),
        _ => ClaudeCodeAdapter::handle_hook(event_str, stdin_buf, mmu),
    }
}

fn non_replacing_hook_response(agent: AgentKind) -> String {
    match agent {
        AgentKind::AntigravityCli => r#"{"injectSteps":[],"terminationBehavior":""}"#.to_owned(),
        _ => r#"{"continue":true}"#.to_owned(),
    }
}

fn execute_hook(args: &[String], stdin_buf: &str) -> Result<String, serde_json::Error> {
    let mut agent_str = "claude-code";
    let mut event_str = "SessionStart";
    let mut dir_str: Option<&str> = None;
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
            "--dir" if i + 1 < args.len() => {
                dir_str = Some(args[i + 1].as_str());
                i += 2;
            }
            _ => i += 1,
        }
    }

    let mut mmu = InfinityContextMmu::default();
    let agent = AgentKind::parse_agent(agent_str);
    let is_session_start = event_str.eq_ignore_ascii_case("SessionStart")
        || event_str.eq_ignore_ascii_case("session-start");
    let ws_dir = resolve_workspace_dir(dir_str);
    if ws_dir.exists() {
        let Ok(lock_guard) = InfinityContextMmu::acquire_workspace_lock(&ws_dir) else {
            eprintln!(
                "icx hook warning: failed to acquire workspace lock in {}",
                ws_dir.display()
            );
            return if is_session_start {
                dispatch_agent_hook(agent, event_str, stdin_buf, &mut mmu)
            } else {
                Ok(non_replacing_hook_response(agent))
            };
        };
        if let Err(err) = mmu.load_workspace_dir_unlocked(&ws_dir) {
            eprintln!(
                "icx hook warning: failed to load workspace state from {}: {err}",
                ws_dir.display()
            );
            drop(lock_guard);
            return if is_session_start {
                dispatch_agent_hook(agent, event_str, stdin_buf, &mut mmu)
            } else {
                Ok(non_replacing_hook_response(agent))
            };
        }
        let out = dispatch_agent_hook(agent, event_str, stdin_buf, &mut mmu)?;
        if let Err(err) = mmu.save_workspace_dir_unlocked(&ws_dir) {
            eprintln!(
                "icx hook warning: failed to save workspace state to {}: {err}",
                ws_dir.display()
            );
            drop(lock_guard);
            return if is_session_start {
                Ok(out)
            } else {
                Ok(non_replacing_hook_response(agent))
            };
        }
        drop(lock_guard);
        return Ok(out);
    }
    if is_session_start {
        dispatch_agent_hook(agent, event_str, stdin_buf, &mut mmu)
    } else {
        Ok(non_replacing_hook_response(agent))
    }
}

fn mcp_tool_error_response(req_line: &str, err_msg: &str) -> serde_json::Value {
    let parsed: serde_json::Value = serde_json::from_str(req_line).unwrap_or_default();
    let Some(id) = parsed.get("id").filter(|id| !id.is_null()).cloned() else {
        return serde_json::Value::Null;
    };
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": err_msg }],
            "isError": true
        }
    })
}

fn serve_mcp_stream<R: BufRead, W: io::Write>(
    args: &[String],
    reader: R,
    mut writer: W,
) -> Result<(), String> {
    let mut mmu = InfinityContextMmu::default();
    let ws_dir = resolve_workspace_dir(parse_dir_flag(args));
    for line in reader.lines() {
        let Ok(req_line) = line else { break };
        if req_line.trim().is_empty() {
            continue;
        }
        let resp = if ws_dir.exists() && McpPagerServer::requires_workspace_state(&req_line) {
            match acquire_workspace_lock(&ws_dir) {
                Ok(lock_guard) => match mmu.load_workspace_dir_unlocked(&ws_dir) {
                    Ok(()) => {
                        let r = McpPagerServer::handle_jsonrpc(&req_line, &mut mmu);
                        let is_err = r
                            .get("result")
                            .and_then(|res| res.get("isError"))
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false);
                        if McpPagerServer::is_mutating_request(&req_line)
                            && !is_err
                            && let Err(e) = mmu.save_workspace_dir_unlocked(&ws_dir)
                        {
                            eprintln!("icx mcp-serve warning: save error: {e}");
                            drop(lock_guard);
                            mcp_tool_error_response(
                                &req_line,
                                &format!("ERR: failed to persist workspace state: {e}"),
                            )
                        } else {
                            drop(lock_guard);
                            r
                        }
                    }
                    Err(e) => {
                        eprintln!("icx mcp-serve warning: load error: {e}");
                        drop(lock_guard);
                        mcp_tool_error_response(
                            &req_line,
                            &format!("ERR: failed to load workspace state: {e}"),
                        )
                    }
                },
                Err(e) => {
                    eprintln!("icx mcp-serve warning: lock error: {e}");
                    mcp_tool_error_response(
                        &req_line,
                        &format!("ERR: failed to acquire workspace lock: {e}"),
                    )
                }
            }
        } else {
            McpPagerServer::handle_jsonrpc(&req_line, &mut mmu)
        };
        if resp.is_null() {
            continue;
        }
        writeln!(writer, "{resp}").map_err(|e| format!("mcp-serve write error: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("mcp-serve flush error: {e}"))?;
    }
    Ok(())
}

fn run_proxy_server(args: &[String]) -> Result<(), String> {
    let env_bind = env::var("ICX_PROXY_BIND")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let mut listen_addr: &str = env_bind.as_deref().unwrap_or("127.0.0.1:49380");
    let mut provider: Option<ProviderFormat> = None;
    let mut upstream_str: Option<&str> = None;
    let mut dir_str: Option<&str> = None;
    let mut max_requests: Option<usize> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" if i + 1 < args.len() => {
                listen_addr = &args[i + 1];
                i += 2;
            }
            "--provider" if i + 1 < args.len() => {
                let raw = &args[i + 1];
                provider = Some(ProviderFormat::parse_provider(raw).ok_or_else(|| {
                    format!(
                        "proxy error: unknown --provider `{raw}` (expected anthropic, openai, or gemini)"
                    )
                })?);
                i += 2;
            }
            "--upstream" if i + 1 < args.len() => {
                upstream_str = Some(args[i + 1].as_str());
                i += 2;
            }
            "--dir" if i + 1 < args.len() => {
                dir_str = Some(args[i + 1].as_str());
                i += 2;
            }
            "--max-requests" if i + 1 < args.len() => {
                max_requests = Some(
                    args[i + 1]
                        .parse::<usize>()
                        .map_err(|e| format!("proxy error: invalid --max-requests: {e}"))?,
                );
                i += 2;
            }
            _ => i += 1,
        }
    }
    let mut mmu = InfinityContextMmu::default();
    let ws_dir = resolve_workspace_dir(dir_str);
    let env_upstream = env::var("ICX_UPSTREAM_URL")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let effective_upstream = upstream_str.or(env_upstream.as_deref()).or(Some("auto"));
    let listener =
        TcpListener::bind(listen_addr).map_err(|e| format!("proxy bind `{listen_addr}`: {e}"))?;
    WireProxyTransformer::serve_http_listener_with_config(
        &listener,
        provider,
        effective_upstream,
        Some(ws_dir.as_path()),
        max_requests,
        &mut mmu,
    )
    .map_err(|e| format!("proxy serve error: {e}"))?;
    Ok(())
}

fn execute_proxy_transform(args: &[String], stdin_buf: &str) -> Result<String, String> {
    let mut provider: Option<ProviderFormat> = None;
    let mut dir_str: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--provider" && i + 1 < args.len() {
            let raw = args[i + 1].as_str();
            provider = Some(ProviderFormat::parse_provider(raw).ok_or_else(|| {
                format!(
                    "proxy-transform error: unknown --provider `{raw}` (expected anthropic, openai, or gemini)"
                )
            })?);
            i += 2;
        } else if args[i] == "--dir" && i + 1 < args.len() {
            dir_str = Some(args[i + 1].as_str());
            i += 2;
        } else {
            i += 1;
        }
    }
    let mut mmu = InfinityContextMmu::default();
    let icx_dir = resolve_workspace_dir(dir_str);
    if icx_dir.exists() {
        return mmu
            .with_locked_workspace(&icx_dir, |mmu| {
                WireProxyTransformer::transform_for_provider(provider, "/", stdin_buf, mmu)
                    .map_err(icx_adapters::AdapterError::from)
            })
            .map_err(|e| format!("proxy-transform error: {e}"));
    }

    WireProxyTransformer::transform_for_provider(provider, "/", stdin_buf, &mut mmu)
        .map_err(|e| format!("proxy-transform error: {e}"))
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

    let mut mmu = InfinityContextMmu::default();
    let res = mmu
        .with_locked_workspace(icx_dir, |mmu| {
            Ok::<_, icx_core::IcxError>(mmu.ingest_payload(
                AgentKind::GenericMcp,
                subject,
                content,
                false,
            ))
        })
        .map_err(|e| e.to_string())?;
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
    let mut dir_str: Option<&str> = None;
    let mut profile_a = false;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                if i + 1 >= args.len() {
                    return Err("icx encode-ahtl error: missing value for --dir".into());
                }
                dir_str = Some(args[i + 1].as_str());
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

    let icx_dir = resolve_workspace_dir(dir_str);
    let compressed_ahtl =
        encode_payload_into_workspace_profile(path_str, &content, &icx_dir, profile_a)
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

fn run_status(args: &[String]) {
    let mut mmu = InfinityContextMmu::default();
    let ws_dir = resolve_workspace_dir(parse_dir_flag(args));
    if ws_dir.exists()
        && let Err(err) = mmu.load_workspace_dir(&ws_dir)
    {
        eprintln!(
            "icx status warning: failed to load workspace state from {}: {err}",
            ws_dir.display()
        );
    }
    let status = McpPagerServer::execute_icx_op(&serde_json::json!({"op": "status"}), &mut mmu);
    println!("{status}");
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn find_executable_in_path(bin_name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(bin_name);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

static DOCTOR_PROBE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn is_dir_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let seq = DOCTOR_PROBE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let probe_path = dir.join(format!(".icx_doctor_probe_{}_{seq}", std::process::id()));
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let Ok(f) = opts.open(&probe_path) else {
        return false;
    };
    drop(f);
    let _ = fs::remove_file(&probe_path);
    true
}

fn parse_doctor_dir_flag(args: &[String]) -> Result<Option<&str>, String> {
    let mut dir_str: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                if i + 1 >= args.len() {
                    return Err("icx doctor error: missing value for --dir".into());
                }
                dir_str = Some(args[i + 1].as_str());
                i += 2;
            }
            unknown => {
                return Err(format!(
                    "icx doctor error: unknown option `{unknown}` (expected `--dir <PATH>`)"
                ));
            }
        }
    }
    Ok(dir_str)
}

fn run_doctor(args: &[String]) -> Result<(), String> {
    let explicit_dir = parse_doctor_dir_flag(args)?;
    println!("=== Infinity-Context (icx) Cross-Agent Workstation Doctor ===");
    let home = env::var("HOME").unwrap_or_else(|_| ".".into());
    let resolve_bin = |default_subpath: &str, bin_name: &str| -> String {
        let primary = format!("{home}/{default_subpath}");
        if is_executable_file(Path::new(&primary)) {
            return primary;
        }
        find_executable_in_path(bin_name).map_or(primary, |p| p.display().to_string())
    };
    let probes = [
        (
            "Claude Code Binary",
            resolve_bin(".local/bin/claude", "claude"),
            true,
        ),
        (
            "Claude Settings",
            format!("{home}/.claude/settings.json"),
            false,
        ),
        (
            "OpenAI Codex Binary",
            resolve_bin(".local/bin/codex", "codex"),
            true,
        ),
        (
            "Codex Hooks Config",
            format!("{home}/.codex/hooks.json"),
            false,
        ),
        (
            "Codex State SQLite",
            format!("{home}/.codex/state_5.sqlite"),
            false,
        ),
        (
            "Antigravity CLI (agy)",
            resolve_bin(".local/bin/agy", "agy"),
            true,
        ),
        (
            "Antigravity Hooks",
            format!("{home}/.gemini/config/hooks.json"),
            false,
        ),
        (
            "Hermes Agent Source",
            format!("{home}/.hermes/hermes-agent/agent/context_engine.py"),
            false,
        ),
        (
            "HTTPS Proxy Transport (curl)",
            find_executable_in_path("curl")
                .map_or_else(|| "/usr/bin/curl".into(), |p| p.display().to_string()),
            true,
        ),
    ];

    for (label, path, require_exec) in probes {
        let p = Path::new(&path);
        let exists = if require_exec {
            is_executable_file(p)
        } else {
            p.is_file()
        };
        let status = if exists { "DETECTED" } else { "ABSENT" };
        println!("  [{status:8}] {label:32} -> {path}");
    }

    let shm_dir = Path::new("/dev/shm");
    let shm_writable = is_dir_writable(shm_dir);
    let shm_status = if shm_writable {
        "DETECTED"
    } else if shm_dir.is_dir() {
        "READONLY"
    } else {
        "ABSENT"
    };
    println!(
        "  [{shm_status:8}] {:32} -> /dev/shm (writable={shm_writable})",
        "POSIX Shared Memory (/dev/shm)"
    );

    let mut doctor_status = if shm_writable { "READY" } else { "DEGRADED" };
    let mut doctor_err: Option<String> =
        (!shm_writable).then(|| "icx doctor error: /dev/shm is absent or not writable".into());
    let ws_dir = resolve_workspace_dir(explicit_dir);
    if ws_dir.exists() {
        let mut mmu = InfinityContextMmu::default();
        // Note: `InfinityContextMmu::load_workspace_dir` acquires `cas_store.lock` + `WORKSPACE_IO_LOCK`
        // before reading `cas_store.json`, `bitemporal_graph.json`, and `mmu_state.json`.
        match mmu.load_workspace_dir(&ws_dir) {
            Ok(()) => {
                let cas_ok = mmu.cas.verify_all_integrity();
                let dag_ok = mmu.dag.verify_all_merkle_integrity_with_cas(Some(&mmu.cas));
                match (cas_ok, dag_ok) {
                    (Ok(artifacts), Ok(nodes)) => {
                        println!(
                            "  [VERIFIED] {:32} -> {} (cas_artifacts={artifacts}, dag_nodes={nodes}, graph_edges={})",
                            "Workspace Storage & Merkle-DAG",
                            ws_dir.display(),
                            mmu.graph.edges().len()
                        );
                    }
                    (Err(e), _) | (_, Err(e)) => {
                        doctor_status = "CORRUPT";
                        println!(
                            "  [CORRUPT ] {:32} -> {} ({e})",
                            "Workspace Storage & Merkle-DAG",
                            ws_dir.display()
                        );
                        doctor_err = Some(format!(
                            "icx doctor error: workspace {} is corrupt: {e}",
                            ws_dir.display()
                        ));
                    }
                }
            }
            Err(e) => {
                doctor_status = "DEGRADED";
                println!(
                    "  [WARNING ] {:32} -> {} ({e})",
                    "Workspace Storage & Merkle-DAG",
                    ws_dir.display()
                );
                doctor_err = Some(format!(
                    "icx doctor error: workspace {} failed to load: {e}",
                    ws_dir.display()
                ));
            }
        }
    } else if explicit_dir.is_some() {
        doctor_status = "DEGRADED";
        println!(
            "  [ABSENT  ] {:32} -> {} (not found)",
            "Workspace Storage & Merkle-DAG",
            ws_dir.display()
        );
        doctor_err = Some(format!(
            "icx doctor error: workspace directory {} does not exist",
            ws_dir.display()
        ));
    }

    println!("Doctor status: {doctor_status}");
    if let Some(err) = doctor_err {
        return Err(err);
    }
    Ok(())
}

fn check_slo(passed: bool, label: &str, measured: &str, failures: &mut Vec<String>) {
    let status = if passed { "PASS" } else { "FAIL" };
    println!("  [{status}] {label}: {measured}");
    if !passed {
        failures.push(format!("{label} ({measured})"));
    }
}

fn run_benchmark() -> Result<(), String> {
    println!("=== Infinity-Context (icx) SLO & Accuracy Benchmark Suite ===");
    // Note: Under debug/coverage (`cfg!(debug_assertions)`) builds, compiler optimizations are
    // disabled (`opt-level = 0`) and LLVM coverage instrumentation is injected (`100x` allowance);
    // on shared virtualized CI runners (`CI=true` / `GITHUB_ACTIONS=true`), wall-clock ceilings
    // allow a `3x` VM jitter factor while dedicated workstations enforce the raw `1.0x` ceilings
    // from `docs/performance.md` §1.
    let is_ci = env::var_os("CI").is_some() || env::var_os("GITHUB_ACTIONS").is_some();
    let scale = if cfg!(debug_assertions) {
        100.0
    } else if is_ci {
        3.0
    } else {
        1.0
    };
    let mut failures = Vec::new();

    // 1. 10,240-bit HDC Bind (^) + Permute Latency Benchmark (ceiling: <= 100 ns)
    let hv_a = Hypervector::from_seed("bench", "role_key");
    let hv_b = Hypervector::from_seed("bench", "filler_val");
    let t_bind = Instant::now();
    let mut hv_acc = hv_a;
    for shift in 0..1_000usize {
        hv_acc = (hv_acc ^ hv_b).permute(shift & 63);
    }
    let bind_ns = (t_bind.elapsed().as_nanos() as f64) / 1_000.0;
    assert_ne!(hv_acc.popcount(), 0);
    check_slo(
        bind_ns <= 100.0 * scale,
        "10,240-bit HDC Bind (^) + Permute",
        &format!("{bind_ns:.2} ns / op (1,000 ops)"),
        &mut failures,
    );

    // 2. 10,240-bit HDC Hamming Scan Benchmark (ceiling: <= 150 us / 1,000 nodes)
    let query_hv = Hypervector::from_seed("bench", "query_target");
    let candidates: Vec<Hypervector> = (0..1_000)
        .map(|i| Hypervector::from_seed("bench", &format!("cand_{i}")))
        .collect();
    let t0 = Instant::now();
    let mut min_dist = u32::MAX;
    for c in &candidates {
        min_dist = min_dist.min(query_hv.hamming_distance_simd(c));
    }
    let hdc_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
    let per_10k_us = hdc_us * 10.0;
    check_slo(
        hdc_us <= 150.0 * scale,
        "10,240-bit HDC Hamming Scan      ",
        &format!(
            "{hdc_us:.2} us / 1,000 nodes ({per_10k_us:.2} us / 10k nodes, min_dist={min_dist})"
        ),
        &mut failures,
    );

    // 3. Fractal IFS Trajectory Suffix Match Benchmark (ceiling: <= 50 ns / pair)
    let mut traj_a = icx_hdc::fractal::FractalCoord::default();
    let mut traj_b = icx_hdc::fractal::FractalCoord::default()
        .step(&Hypervector::from_seed("bench", "divergent_prefix"));
    for step in ["edit", "compile", "test", "fail", "fix", "verify"] {
        let step_hv = Hypervector::from_seed("crates/icx-core/src/dag.rs", step);
        traj_a = traj_a.step(&step_hv);
        traj_b = traj_b.step(&step_hv);
    }
    let t_ifs = Instant::now();
    let mut shared_depth = 0u32;
    for _ in 0..1_000 {
        shared_depth = std::hint::black_box(
            std::hint::black_box(&traj_a).shared_suffix_depth(std::hint::black_box(&traj_b)),
        );
    }
    let ifs_ns = (t_ifs.elapsed().as_nanos() as f64) / 1_000.0;
    check_slo(
        shared_depth >= 6 && ifs_ns <= 50.0 * scale,
        "Fractal IFS Suffix Match         ",
        &format!("{ifs_ns:.2} ns / pair (shared_depth={shared_depth})"),
        &mut failures,
    );

    // 4. Two-Stage HdcAssociativeIndex & CSR BiTemporalGraph PPR Benchmark
    let mut bench_idx = icx_hdc::HdcAssociativeIndex::new();
    for i in 0..250u64 {
        let _ = bench_idx.observe_event(
            ArtifactId(i),
            &format!("src/module_{i}.rs"),
            &format!("pub fn helper_{i}() -> usize {{ {i} }}"),
            false,
        );
    }
    let _ = bench_idx.observe_event_with_exit_code(
        ArtifactId(99_999),
        "crates/icx-store/src/graph_ppr.rs",
        "pub fn personalized_pagerank() { panic!(\"E0502 csr_fault\"); }",
        Some(1),
        true,
    );
    let t_idx = Instant::now();
    let stage_hits = bench_idx.query_associative("graph_ppr personalized_pagerank", 5);
    let idx_us = t_idx.elapsed().as_secs_f64() * 1_000_000.0;
    assert_eq!(stage_hits[0].artifact_id, ArtifactId(99_999));

    let mut bench_graph = icx_store::BiTemporalGraph::new();
    for i in 0..200u64 {
        let u = format!("mod:m_{i}");
        let v = format!("mod:m_{}", i + 1);
        bench_graph.upsert_edge(&u, "calls", &v, false, 100 + i, ArtifactId(i), 1.0);
    }
    let t_ppr = Instant::now();
    let ppr_hits = bench_graph.personalized_pagerank(&["mod:m_0"], None, 0.15, 8, 5);
    let ppr_us = t_ppr.elapsed().as_secs_f64() * 1_000_000.0;
    assert_ne!(ppr_hits.len(), 0);
    check_slo(
        idx_us <= 200.0 * scale && ppr_us <= 400.0 * scale,
        "Two-Stage HDC + CSR PPR          ",
        &format!("{idx_us:.2} us (two-stage query), {ppr_us:.2} us (201-node CSR PPR)"),
        &mut failures,
    );

    // 5. AHTL Log Compression Ratio & Latency Benchmark (ceiling: <= 400 us, ratio >= 10x)
    let mut log_fixture = String::from("running 1200 tests\n");
    for i in 0..1_199 {
        log_fixture.push_str(&format!(
            "test crate_core::module_{i}::test_case_{i:04} ... ok\n"
        ));
    }
    log_fixture.push_str("test crate_core::mmu::test_epoch_seal ... FAILED\n");
    log_fixture.push_str(
        "thread 'crate_core::mmu::test_epoch_seal' panicked at crates/icx-core/src/epoch.rs:142:9: assertion failed\n",
    );

    let (id, _) = ArtifactId::hash_bytes(log_fixture.as_bytes());
    let t1 = Instant::now();
    let pkt = StructuralSkeletonizer::compress_sanitized_to_packet(
        "cargo_test",
        &log_fixture,
        Some(id),
        [0x42; 8],
    );
    let rendered = pkt.render();
    let codec_us = t1.elapsed().as_secs_f64() * 1_000_000.0;
    let ratio = (log_fixture.len() as f64) / (rendered.len() as f64);
    check_slo(
        ratio >= 10.0 && codec_us <= 400.0 * scale,
        "AHTL Compression Ratio           ",
        &format!(
            "{ratio:.1}x ({} B -> {} B in {codec_us:.2} us)",
            log_fixture.len(),
            rendered.len()
        ),
        &mut failures,
    );

    // 6. FastCDC + BLAKE3 CAS Ingest (64 KB) Benchmark (release ceiling: <= 250 us)
    let mut cas_bench = icx_core::cas::CasStore::new();
    let cas_payload_64k = "pub fn fastcdc_kernel_step(x: u64) -> u64 { x.wrapping_mul(6364136223846793005).wrapping_add(1) }\n".repeat(660);
    let t_cas = Instant::now();
    let (cas_aid, _) = cas_bench
        .put_bytes_checked(cas_payload_64k.as_bytes())
        .expect("64 KB CAS ingest succeeds");
    let cas_us = t_cas.elapsed().as_secs_f64() * 1_000_000.0;
    assert_eq!(
        cas_bench.deref_bytes(cas_aid).unwrap().len(),
        cas_payload_64k.len()
    );
    check_slo(
        cas_us <= 250.0 * scale,
        "FastCDC + BLAKE3 CAS Ingest (64K)",
        &format!(
            "{cas_us:.2} us ({} B across {} chunks)",
            cas_payload_64k.len(),
            cas_bench.unique_chunk_count()
        ),
        &mut failures,
    );

    // 7. In-Process Atomic Seqlock Broadcast Latency Benchmark (release ceiling: <= 5,000 ns)
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
    check_slo(
        bus_ns_per_op <= 5_000.0 * scale,
        "Atomic Seqlock Broadcast         ",
        &format!("{bus_ns_per_op:.1} ns / slot (1,000 publishes)"),
        &mut failures,
    );

    // 8. Full Synchronous Hook Round-Trip (`ClaudeCodeAdapter::handle_hook`) Benchmark (release ceiling: <= 1,000 us)
    let hook_cfg = EpochControllerConfig::anthropic_default();
    let mut hook_mmu =
        InfinityContextMmu::new(hook_cfg, icx_bus::SeqlockTelemetryBus::new_in_memory());
    let hook_payload = format!(
        "running 24 tests\n{}test crate_core::mmu::test_epoch_seal ... FAILED\n",
        "test crate_core::suite::verify_invariant ... ok\n".repeat(23)
    );
    let hook_stdin = serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": "cargo test --workspace"},
        "tool_response": &hook_payload
    })
    .to_string();
    let t_hook = Instant::now();
    let hook_out =
        ClaudeCodeAdapter::handle_hook("PostToolUse", &hook_stdin, &mut hook_mmu).unwrap();
    let hook_us = t_hook.elapsed().as_secs_f64() * 1_000_000.0;
    assert!(hook_out.contains("updatedToolOutput"));
    check_slo(
        hook_us <= 1_000.0 * scale,
        "Full Synchronous Hook Round-Trip ",
        &format!("{hook_us:.2} us (PostToolUse virtualization)"),
        &mut failures,
    );

    // 9. Multi-Turn Prefix-Cache Hit Ratio & Lossless Needle-in-a-Haystack Benchmark
    let t2 = Instant::now();
    let epoch_cfg = EpochControllerConfig {
        epoch_rollover_tokens: 1000,
        protect_last_n_turns: 2,
        tail_slot_max_tokens: 256,
    };
    let mut mmu = InfinityContextMmu::new(epoch_cfg, icx_bus::SeqlockTelemetryBus::new_in_memory());
    mmu.epochs = CacheEpochController::new(
        "⟪ICX:PINNED_HEAD│system=infinity-context-mmu-v1│cache_breakpoint=0⟫\n".repeat(300),
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
    check_slo(
        avg_cache_hit >= 90.0,
        "50-Turn NIH + Cache-Epoch        ",
        &format!(
            "{avg_cache_hit:.2}% prefix cache hit rate, 100% lossless passkey recovery ({:.2} ms)",
            sim_elapsed.as_secs_f64() * 1_000.0
        ),
        &mut failures,
    );

    if !failures.is_empty() {
        return Err(format!(
            "icx benchmark error: {} SLO gate(s) failed: {}",
            failures.len(),
            failures.join("; ")
        ));
    }
    println!("All Infinity-Context SLO gates PASSED.");
    Ok(())
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
        assert!(find_executable_in_path("__nonexistent_icx_test_binary__").is_none());
        assert!(!is_executable_file(&tmp_dir));
        assert!(!is_executable_file(&sample_file));
        assert!(!is_dir_writable(&sample_file));
        // Verify `icx doctor --dir <PATH>` against a valid initialized workspace
        dispatch_cli_args(
            &[
                "icx".into(),
                "doctor".into(),
                "--dir".into(),
                dir_arg.clone(),
            ],
            io::Cursor::new(""),
        )
        .unwrap();
        // Verify `[CORRUPT ]` branch of `icx doctor --dir <PATH>` when a DAG node's digest is tampered
        let tampered_ws = tmp_dir.join("tampered_dag_ws");
        init_workspace_dir(&tampered_ws).unwrap();
        let _ = encode_payload_into_workspace("src/t.rs", "pub fn t() {}\n", &tampered_ws).unwrap();
        let mmu_json_path = tampered_ws.join("mmu_state.json");
        let mut mmu_val: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&mmu_json_path).unwrap()).unwrap();
        if let Some(nodes) = mmu_val
            .get_mut("dag")
            .and_then(|d| d.get_mut("nodes"))
            .and_then(serde_json::Value::as_object_mut)
            && let Some((_, first_node)) = nodes.iter_mut().next()
            && let Some(turn_obj) = first_node.get_mut("Turn")
            && let Some(arr) = turn_obj
                .get_mut("digest")
                .and_then(serde_json::Value::as_array_mut)
            && let Some(b0) = arr.first_mut()
        {
            *b0 = serde_json::json!(255);
        }
        fs::write(&mmu_json_path, serde_json::to_string(&mmu_val).unwrap()).unwrap();
        assert!(
            dispatch_cli_args(
                &[
                    "icx".into(),
                    "doctor".into(),
                    "--dir".into(),
                    tampered_ws.to_string_lossy().into_owned(),
                ],
                io::Cursor::new(""),
            )
            .is_err()
        );
        // Verify `icx doctor --dir <PATH>` fails closed when `cas_store.json` is deleted while
        // `mmu_state.json` still references DAG turn artifacts, or when `--dir` does not exist
        let missing_cas_ws = tmp_dir.join("missing_cas_ws");
        init_workspace_dir(&missing_cas_ws).unwrap();
        let _ =
            encode_payload_into_workspace("src/m.rs", "pub fn m() {}\n", &missing_cas_ws).unwrap();
        fs::remove_file(missing_cas_ws.join("cas_store.json")).unwrap();
        assert!(
            dispatch_cli_args(
                &[
                    "icx".into(),
                    "doctor".into(),
                    "--dir".into(),
                    missing_cas_ws.to_string_lossy().into_owned(),
                ],
                io::Cursor::new(""),
            )
            .is_err()
        );
        assert!(
            dispatch_cli_args(
                &[
                    "icx".into(),
                    "doctor".into(),
                    "--dir".into(),
                    tmp_dir
                        .join("no_such_doctor_dir")
                        .to_string_lossy()
                        .into_owned(),
                ],
                io::Cursor::new(""),
            )
            .is_err()
        );
        assert!(
            dispatch_cli_args(
                &["icx".into(), "doctor".into(), "--dir".into()],
                io::Cursor::new(""),
            )
            .is_err()
        );
        assert!(
            dispatch_cli_args(
                &["icx".into(), "doctor".into(), "--unknown".into()],
                io::Cursor::new(""),
            )
            .is_err()
        );
        let mut fail_vec = Vec::new();
        check_slo(false, "synthetic_gate", "9999 us", &mut fail_vec);
        assert_eq!(fail_vec.len(), 1);
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

        // Test hook dispatch for claude-code, codex, agy, hermes, and malformed JSON
        for agent in ["claude-code", "codex", "agy", "hermes"] {
            dispatch_cli_args(
                &[
                    "icx".into(),
                    "hook".into(),
                    "--agent".into(),
                    agent.into(),
                    "--event".into(),
                    "SessionStart".into(),
                    "--dir".into(),
                    dir_arg.clone(),
                    "--ignore".into(),
                ],
                io::Cursor::new("{}"),
            )
            .unwrap();
        }
        dispatch_cli_args(
            &[
                "icx".into(),
                "status".into(),
                "--dir".into(),
                dir_arg.clone(),
                "--ignore".into(),
            ],
            io::Cursor::new(""),
        )
        .unwrap();
        dispatch_cli_args(
            &["icx".into(), "hook".into()],
            io::Cursor::new("{malformed_json"),
        )
        .unwrap();

        // Test proxy-transform dispatch (Anthropic, OpenAI, Gemini) and `icx proxy`
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
            &[
                "icx".into(),
                "proxy-transform".into(),
                "--provider".into(),
                "gemini".into(),
            ],
            io::Cursor::new(r#"{"contents":[{"role":"user","parts":[{"text":"hi"}]}]}"#),
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

        // Test `icx proxy` subcommand with `--max-requests 0`, `--upstream`, and error paths
        dispatch_cli_args(
            &[
                "icx".into(),
                "proxy".into(),
                "--listen".into(),
                "127.0.0.1:0".into(),
                "--provider".into(),
                "anthropic".into(),
                "--upstream".into(),
                "http://127.0.0.1:49381".into(),
                "--dir".into(),
                dir_arg.clone(),
                "--max-requests".into(),
                "0".into(),
                "--unused".into(),
            ],
            io::Cursor::new(""),
        )
        .unwrap();
        assert!(
            dispatch_cli_args(
                &[
                    "icx".into(),
                    "proxy".into(),
                    "--provider".into(),
                    "invalid".into()
                ],
                io::Cursor::new("")
            )
            .is_err()
        );
        assert!(
            dispatch_cli_args(
                &[
                    "icx".into(),
                    "proxy".into(),
                    "--max-requests".into(),
                    "not_a_number".into()
                ],
                io::Cursor::new("")
            )
            .is_err()
        );

        // Test mcp-serve dispatch with `--dir` (including mutating request) and without `--dir`
        let mcp_input = "\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"icx\",\"arguments\":{\"op\":\"fold\",\"id\":\"cli_fold\",\"q\":\"folded\"}}}\n";
        dispatch_cli_args(
            &[
                "icx".into(),
                "mcp-serve".into(),
                "--dir".into(),
                dir_arg.clone(),
                "--unused".into(),
            ],
            io::Cursor::new(mcp_input),
        )
        .unwrap();
        dispatch_cli_args(
            &[
                "icx".into(),
                "mcp-serve".into(),
                "--dir".into(),
                tmp_dir.join("no_such_ws").to_string_lossy().into_owned(),
            ],
            io::Cursor::new("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n"),
        )
        .unwrap();

        // Test execute_hook fail-open behavior when workspace state file is corrupted
        let corrupt_ws = tmp_dir.join("corrupt_ws");
        fs::create_dir_all(&corrupt_ws).unwrap();
        fs::write(corrupt_ws.join("mmu_state.json"), "{corrupt_json").unwrap();
        assert!(
            dispatch_cli_args(
                &[
                    "icx".into(),
                    "doctor".into(),
                    "--dir".into(),
                    corrupt_ws.to_string_lossy().into_owned(),
                ],
                io::Cursor::new(""),
            )
            .is_err()
        );
        let corrupt_hook_out = execute_hook(
            &[
                "--agent".into(),
                "claude".into(),
                "--event".into(),
                "SessionStart".into(),
                "--dir".into(),
                corrupt_ws.to_string_lossy().into_owned(),
            ],
            "{}",
        )
        .unwrap();
        assert!(corrupt_hook_out.contains("ICX:LEGEND"));
        assert_eq!(
            fs::read_to_string(corrupt_ws.join("mmu_state.json")).unwrap(),
            "{corrupt_json"
        );
        let corrupt_post_tool = execute_hook(
            &[
                "--agent".into(),
                "claude".into(),
                "--event".into(),
                "PostToolUse".into(),
                "--dir".into(),
                corrupt_ws.to_string_lossy().into_owned(),
            ],
            r#"{"tool_name":"Bash","tool_response":"big output"}"#,
        )
        .unwrap();
        assert_eq!(corrupt_post_tool, r#"{"continue":true}"#);
        let corrupt_agy_post_tool = execute_hook(
            &[
                "--agent".into(),
                "agy".into(),
                "--event".into(),
                "PostToolUse".into(),
                "--dir".into(),
                corrupt_ws.to_string_lossy().into_owned(),
            ],
            r#"{"toolName":"bash","toolOutput":"big output"}"#,
        )
        .unwrap();
        assert_eq!(
            corrupt_agy_post_tool,
            r#"{"injectSteps":[],"terminationBehavior":""}"#
        );

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
        let err_rpc =
            mcp_tool_error_response(r#"{"jsonrpc":"2.0","id":9,"method":"tools/call"}"#, "ERR");
        assert_eq!(err_rpc["result"]["isError"], true);
        let notif_rpc =
            mcp_tool_error_response(r#"{"jsonrpc":"2.0","method":"tools/call"}"#, "ERR");
        assert!(notif_rpc.is_null());

        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
