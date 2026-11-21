//! Resident per-workspace daemon (v1.1.0; ICX-AUD-001, ICX-AUD-047).
//!
//! `icx daemon` keeps one workspace's MMU in memory and serves `icx hook` requests over a Unix
//! socket in the private runtime directory (`$XDG_RUNTIME_DIR/icx`, mode `0700`). Every
//! connection is authenticated with `SO_PEERCRED`: the peer's uid must equal ours.
//!
//! A successful reply is sent only after committing the dispatched request to SQLite. The
//! workspace lock covers reload, dispatch and save, so proxy/MCP writes cannot interleave with
//! an unsaved request. Save failures produce a failed response and terminate the daemon with an
//! error; no acknowledged changes remain in a write-behind window or get replaced by a reload.
//!
//! `icx hook` falls back to in-process handling on any daemon error, so a missing or broken
//! daemon never breaks an agent.

use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::{Duration, Instant};

/// Protocol version carried in every request.
pub const PROTOCOL_VERSION: u32 = 1;
/// Largest accepted request line (64 MiB payload bound plus envelope).
pub const MAX_REQUEST_BYTES: u64 = icx_core::ICX_MAX_INGEST_BYTES as u64 + 64 * 1024;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    v: u32,
    agent: String,
    event: String,
    payload: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Response {
    ok: bool,
    out: String,
}

fn peer_is_same_user(stream: &UnixStream) -> bool {
    rustix::net::sockopt::socket_peercred(stream)
        .is_ok_and(|cred| cred.uid == rustix::process::geteuid())
}

/// Sends one hook request to a running daemon. Returns `None` (caller falls back to in-process
/// handling) if no daemon is listening, the daemon is not ours, or anything goes wrong.
pub fn try_hook(socket: &Path, agent: &str, event: &str, payload: &str) -> Option<String> {
    if !socket.exists() {
        return None;
    }
    let mut stream = UnixStream::connect(socket).ok()?;
    if !peer_is_same_user(&stream) {
        return None;
    }
    stream.set_read_timeout(Some(CLIENT_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(CLIENT_TIMEOUT)).ok()?;
    let req = Request {
        v: PROTOCOL_VERSION,
        agent: agent.to_owned(),
        event: event.to_owned(),
        payload: payload.to_owned(),
    };
    let mut line = serde_json::to_vec(&req).ok()?;
    line.push(b'\n');
    stream.write_all(&line).ok()?;
    stream.flush().ok()?;
    let mut buf = String::new();
    stream
        .take(MAX_REQUEST_BYTES)
        .read_to_string(&mut buf)
        .ok()?;
    let resp: Response = serde_json::from_str(buf.trim_end()).ok()?;
    resp.ok.then_some(resp.out)
}

/// Options for [`serve`].
#[derive(Debug, Clone, Copy, Default)]
pub struct DaemonOptions {
    /// Exit after this long without requests (flushing first). `None` runs until killed.
    pub idle_exit: Option<Duration>,
    /// Exit after serving this many requests (tests and benchmarks).
    pub max_requests: Option<usize>,
}

/// Runs the daemon for `ws_dir` on `socket` until idle-exit or `max_requests`.
pub fn serve(
    ws_dir: &Path,
    socket: &Path,
    opts: DaemonOptions,
    mut mmu: InfinityContextMmu,
    dispatch: fn(
        AgentKind,
        &str,
        &str,
        &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error>,
) -> Result<(), String> {
    // `sockaddr_un.sun_path` is 108 bytes on Linux (104 on BSD/macOS), including the NUL.
    if socket.as_os_str().len() >= 104 {
        return Err(format!(
            "socket path {} is too long for a Unix socket ({} bytes, limit 103); \
             set XDG_RUNTIME_DIR to a shorter directory",
            socket.display(),
            socket.as_os_str().len()
        ));
    }
    if UnixStream::connect(socket).is_ok() {
        return Err(format!("a daemon is already serving {}", socket.display()));
    }
    let _ = std::fs::remove_file(socket);
    let listener =
        UnixListener::bind(socket).map_err(|e| format!("bind {}: {e}", socket.display()))?;
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod {}: {e}", socket.display()))?;
    }
    eprintln!(
        "icx daemon: serving {} on {}",
        ws_dir.display(),
        socket.display()
    );

    let mut last_request = Instant::now();
    let mut served = 0usize;

    let result = loop {
        let timeout = opts
            .idle_exit
            .map(|idle| (last_request + idle).saturating_duration_since(Instant::now()));
        let ready = wait_readable(&listener, timeout);
        if !ready {
            if opts.idle_exit.is_some() {
                break Ok(());
            }
            continue;
        }
        let Ok((stream, _)) = listener.accept() else {
            continue;
        };
        if !peer_is_same_user(&stream) {
            continue; // refuse other users silently
        }
        let lock = match InfinityContextMmu::acquire_workspace_lock(ws_dir) {
            Ok(guard) => guard,
            Err(e) => break Err(format!("workspace lock failed: {e}")),
        };
        if let Err(e) = mmu.load_workspace_dir_unlocked(ws_dir) {
            break Err(format!("workspace load failed: {e}"));
        }
        let handled = handle_connection(stream, ws_dir, &mut mmu, dispatch);
        drop(lock);
        if let Err(error) = handled {
            break Err(error);
        }
        last_request = Instant::now();
        served += 1;
        if opts.max_requests.is_some_and(|max| served >= max) {
            break Ok(());
        }
    };
    let _ = std::fs::remove_file(socket);
    result
}

/// Waits until `listener` has a pending connection or `timeout` elapses (`None` = forever).
fn wait_readable(listener: &UnixListener, timeout: Option<Duration>) -> bool {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    let mut fds = [PollFd::new(listener, PollFlags::IN)];
    let ts = timeout.map(|t| Timespec {
        tv_sec: i64::try_from(t.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(t.subsec_nanos()),
    });
    matches!(poll(&mut fds, ts.as_ref()), Ok(n) if n > 0)
}

/// Serves one request with the workspace lock held, committing before a successful response.
fn handle_connection(
    stream: UnixStream,
    ws_dir: &Path,
    mmu: &mut InfinityContextMmu,
    dispatch: fn(
        AgentKind,
        &str,
        &str,
        &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error>,
) -> Result<(), String> {
    let _ = stream.set_read_timeout(Some(CLIENT_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CLIENT_TIMEOUT));
    let Ok(read_half) = stream.try_clone() else {
        return Ok(());
    };
    let mut reader = BufReader::new(read_half.take(MAX_REQUEST_BYTES));
    let mut line = String::new();
    let mut dispatched = false;
    let resp = match reader.read_line(&mut line) {
        Ok(0) | Err(_) => return Ok(()),
        Ok(_) => match serde_json::from_str::<Request>(line.trim_end()) {
            Ok(req) if req.v == PROTOCOL_VERSION => {
                let agent = AgentKind::parse_agent(&req.agent);
                dispatched = true; // even a failed dispatch may have changed state
                match dispatch(agent, &req.event, &req.payload, mmu) {
                    Ok(out) => Response { ok: true, out },
                    Err(e) => Response {
                        ok: false,
                        out: e.to_string(),
                    },
                }
            }
            Ok(_) => Response {
                ok: false,
                out: "unsupported protocol version".into(),
            },
            Err(e) => Response {
                ok: false,
                out: format!("malformed request: {e}"),
            },
        },
    };
    if dispatched && let Err(error) = mmu.save_workspace_dir_unlocked(ws_dir) {
        let error = format!("workspace save failed; request was not acknowledged: {error}");
        write_response(
            stream,
            &Response {
                ok: false,
                out: error.clone(),
            },
        );
        return Err(error);
    }
    write_response(stream, &resp);
    Ok(())
}

fn write_response(mut stream: UnixStream, resp: &Response) {
    if let Ok(mut bytes) = serde_json::to_vec(&resp) {
        bytes.push(b'\n');
        let _ = stream.write_all(&bytes);
        let _ = stream.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "Matches the production dispatch callback type"
    )]
    fn durability_dispatch(
        agent: AgentKind,
        event: &str,
        payload: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        let result = mmu.ingest_payload(
            agent,
            "durability fixture",
            "durability fixture body\n",
            false,
        );
        if event == "fail-save" {
            let ws = Path::new(payload);
            std::fs::rename(ws.join("icx.sqlite3"), ws.join("pre-failure.sqlite3")).unwrap();
            std::fs::create_dir(ws.join("icx.sqlite3")).unwrap();
        }
        Ok(result.artifact_id.to_string())
    }

    fn start_durability_fixture(
        tag: &str,
        max_requests: Option<usize>,
    ) -> (
        std::path::PathBuf,
        std::path::PathBuf,
        std::thread::JoinHandle<Result<(), String>>,
    ) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let ws = Path::new("/tmp/Infinity-Context/v120/daemon-tests").join(format!(
            "{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&ws).unwrap();
        let mut mmu = InfinityContextMmu::default();
        mmu.save_workspace_dir(&ws).unwrap();
        let socket = ws.join("d.sock");
        let (child_ws, child_socket) = (ws.clone(), socket.clone());
        let server = std::thread::spawn(move || {
            serve(
                &child_ws,
                &child_socket,
                DaemonOptions {
                    idle_exit: Some(Duration::from_millis(200)),
                    max_requests,
                },
                mmu,
                durability_dispatch,
            )
        });
        for _ in 0..400 {
            if socket.exists() {
                return (ws, socket, server);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("daemon did not create its test socket");
    }

    #[test]
    fn successful_ack_is_already_committed_before_idle_or_exit() {
        let (ws, socket, server) = start_durability_fixture("ack", None);
        let output = try_hook(&socket, "codex", "ingest", "").unwrap();
        let aid = output.parse::<icx_core::ArtifactId>().unwrap();
        // Deliberately read without the daemon's lock: taking it would wait until the old
        // write-behind flush and conceal the ACK-before-persistence defect.
        let mut reopened = InfinityContextMmu::default();
        reopened.load_workspace_dir_unlocked(&ws).unwrap();
        let recovered = reopened.deref_text(aid);
        server.join().unwrap().unwrap();
        std::fs::remove_dir_all(ws).unwrap();
        assert_eq!(recovered.unwrap(), "durability fixture body\n");
    }

    #[test]
    fn save_failure_is_not_acknowledged_and_serve_returns_error() {
        let (ws, socket, server) = start_durability_fixture("save-error", Some(1));
        let output = try_hook(&socket, "codex", "fail-save", ws.to_str().unwrap());
        let result = server.join().unwrap();
        std::fs::remove_dir_all(ws).unwrap();
        assert!(
            output.is_none(),
            "failed save returned a successful handle: {output:?}"
        );
        assert!(
            result.is_err(),
            "failed persistence was reported as successful daemon exit"
        );
    }
}
