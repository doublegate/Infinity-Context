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
//! The request is read in full (under one overall deadline) before the lock is taken, and a
//! lock timeout or reload failure fails only that request: the daemon keeps serving.
//!
//! `icx hook` falls back to in-process handling on any daemon error, so a missing or broken
//! daemon never breaks an agent.

use icx_adapters::InfinityContextMmu;
use icx_core::AgentKind;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
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
        let handled = handle_connection(stream, ws_dir, &mut mmu, dispatch);
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

/// Reads one newline-terminated request from `stream`, bounded by `MAX_REQUEST_BYTES` and by an
/// overall `deadline` (not only a per-`read` timeout, which a trickling client could renew
/// indefinitely). Returns `None` on EOF, I/O error, timeout or an oversized request.
fn read_request(stream: &UnixStream, deadline: Duration) -> Option<String> {
    let started = Instant::now();
    let mut reader = stream;
    let mut line = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let remaining = deadline.checked_sub(started.elapsed())?;
        if remaining.is_zero() {
            return None;
        }
        stream.set_read_timeout(Some(remaining)).ok()?;
        let n = match reader.read(&mut chunk) {
            Ok(0) => return None,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return None,
        };
        if let Some(pos) = chunk[..n].iter().position(|&b| b == b'\n') {
            line.extend_from_slice(&chunk[..pos]);
            break;
        }
        line.extend_from_slice(&chunk[..n]);
        if line.len() as u64 > MAX_REQUEST_BYTES {
            return None;
        }
    }
    if line.len() as u64 > MAX_REQUEST_BYTES {
        return None;
    }
    String::from_utf8(line).ok()
}

/// Serves one connection. The request is read and parsed before the workspace lock is taken,
/// so a slow or idle client never blocks in-process hooks or `mcp-serve`; the lock covers only
/// reload, dispatch, save and the reply. A lock timeout or reload failure fails this request
/// (the client falls back to in-process handling) and the daemon keeps serving. Only a failed
/// save after a dispatch returns `Err`, because acknowledged state must never be lost.
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
    let _ = stream.set_write_timeout(Some(CLIENT_TIMEOUT));
    let Some(line) = read_request(&stream, CLIENT_TIMEOUT) else {
        return Ok(());
    };
    let req = match serde_json::from_str::<Request>(line.trim_end()) {
        Ok(req) if req.v == PROTOCOL_VERSION => req,
        Ok(_) => {
            write_response(stream, &failure("unsupported protocol version".into()));
            return Ok(());
        }
        Err(e) => {
            write_response(stream, &failure(format!("malformed request: {e}")));
            return Ok(());
        }
    };

    let lock = match InfinityContextMmu::acquire_workspace_lock(ws_dir) {
        Ok(guard) => guard,
        Err(e) => {
            let error = format!("workspace lock failed: {e}");
            eprintln!("icx daemon: {error}; request refused, still serving");
            write_response(stream, &failure(error));
            return Ok(());
        }
    };
    if let Err(e) = mmu.load_workspace_dir_unlocked(ws_dir) {
        // Nothing was dispatched, so nothing is lost; the next request reloads again before
        // dispatching, so a partially reloaded MMU is never saved.
        let error = format!("workspace load failed: {e}");
        eprintln!("icx daemon: {error}; request refused, still serving");
        write_response(stream, &failure(error));
        return Ok(());
    }
    let agent = AgentKind::parse_agent(&req.agent);
    // Even a failed dispatch may have changed state, so it is saved either way.
    let resp = match dispatch(agent, &req.event, &req.payload, mmu) {
        Ok(out) => Response { ok: true, out },
        Err(e) => failure(e.to_string()),
    };
    if let Err(error) = mmu.save_workspace_dir_unlocked(ws_dir) {
        let error = format!("workspace save failed; request was not acknowledged: {error}");
        write_response(stream, &failure(error.clone()));
        return Err(error);
    }
    write_response(stream, &resp);
    drop(lock);
    Ok(())
}

fn failure(out: String) -> Response {
    Response { ok: false, out }
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

    /// Takes the OS `flock` on `cas_store.lock` through a separate open file description, as
    /// another process would, without the in-process `WORKSPACE_IO_LOCK`.
    fn hold_foreign_flock(ws: &Path) -> std::fs::File {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(ws.join("cas_store.lock"))
            .unwrap();
        file.lock().unwrap();
        file
    }

    /// The daemon spins on the foreign `flock` for ~5 s while holding the process-global
    /// `WORKSPACE_IO_LOCK`, which would stall every other workspace test in this binary, so the
    /// scenario runs in a child process of the test binary.
    #[test]
    fn lock_timeout_fails_that_request_and_daemon_keeps_serving() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "daemon::tests::lock_timeout_scenario",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("ICX_DAEMON_LOCK_TIMEOUT_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success(), "lock-timeout scenario failed: {status}");
    }

    #[test]
    #[ignore = "run in a child process by lock_timeout_fails_that_request_and_daemon_keeps_serving"]
    fn lock_timeout_scenario() {
        if std::env::var_os("ICX_DAEMON_LOCK_TIMEOUT_CHILD").is_none() {
            return;
        }
        let (ws, socket, server) = start_durability_fixture("lock-timeout", Some(2));
        let foreign = hold_foreign_flock(&ws);
        // The daemon gives up on the lock after ~5 s and must answer with a failure (so the
        // client falls back to in-process handling) instead of exiting.
        let refused = try_hook(&socket, "codex", "ingest", "");
        drop(foreign);
        let served = try_hook(&socket, "codex", "ingest", "");
        let result = server.join().unwrap();
        std::fs::remove_dir_all(ws).unwrap();
        assert!(
            refused.is_none(),
            "lock timeout was acknowledged: {refused:?}"
        );
        assert!(
            served.is_some(),
            "daemon stopped serving after a lock timeout"
        );
        assert!(result.is_ok(), "lock timeout ended the daemon: {result:?}");
    }

    #[test]
    fn silent_client_does_not_hold_the_workspace_lock() {
        let (ws, socket, server) = start_durability_fixture("silent", Some(2));
        // Connect and send nothing: the daemon is now blocked reading this request.
        let silent = UnixStream::connect(&socket).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let started = Instant::now();
        let guard = InfinityContextMmu::acquire_workspace_lock(&ws);
        let waited = started.elapsed();
        drop(guard);
        drop(silent);
        let served = try_hook(&socket, "codex", "ingest", "");
        let result = server.join().unwrap();
        std::fs::remove_dir_all(ws).unwrap();
        assert!(
            waited < Duration::from_secs(1),
            "in-process lock waited {waited:?} behind a silent daemon client"
        );
        assert!(served.is_some());
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn request_read_is_bounded_by_an_overall_deadline() {
        let (mut client, server) = UnixStream::pair().unwrap();
        let writer = std::thread::spawn(move || {
            // Trickle one byte every 50 ms: each read() succeeds quickly, so only an overall
            // deadline (not the per-read timeout) can end this request.
            for _ in 0..40 {
                if client.write_all(b"{").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let started = Instant::now();
        let read = read_request(&server, Duration::from_millis(300));
        let elapsed = started.elapsed();
        drop(server);
        writer.join().unwrap();
        assert!(read.is_none(), "trickled request was accepted");
        assert!(
            elapsed < Duration::from_millis(1_000),
            "request read took {elapsed:?} despite a 300 ms deadline"
        );
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
