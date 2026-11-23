//! Resident per-workspace daemon (v1.1.0; ICX-AUD-001, ICX-AUD-047).
//!
//! `icx daemon` keeps one workspace's MMU in memory and serves `icx hook` requests over a Unix
//! socket in the private runtime directory (`$XDG_RUNTIME_DIR/icx`, mode `0700`). Every
//! connection is authenticated with `SO_PEERCRED`: the peer's uid must equal ours.
//!
//! A successful reply is sent only after committing the dispatched request to SQLite. The
//! workspace lock covers reload, dispatch and save, so proxy/MCP writes cannot interleave with
//! an unsaved request. A failed dispatch is never saved, and a failed save is never
//! acknowledged; in both cases the in-memory state is discarded (reloaded from disk before the
//! next request), so the client's in-process retry is the only ingestion. The daemon exits with
//! an error only after `MAX_CONSECUTIVE_SAVE_FAILURES` saves fail in a row. The request (exactly
//! one line per connection; trailing non-whitespace bytes are a protocol error) is read in full
//! under one overall deadline before the lock is taken, and a lock timeout or reload failure
//! fails only that request: the daemon keeps serving.
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

/// Protocol version carried in every request that does not ask for stubs.
pub const PROTOCOL_VERSION: u32 = 1;
/// Version of a request that carries the caller's stub setting (`stub: true`, v1.4.0). An older
/// daemon refuses it, so the client falls back to in-process handling and the setting is never
/// silently dropped (review of #87).
pub const PROTOCOL_VERSION_STUB: u32 = 2;
/// Largest accepted request line (64 MiB payload bound plus envelope).
pub const MAX_REQUEST_BYTES: u64 = icx_core::ICX_MAX_INGEST_BYTES as u64 + 64 * 1024;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Serialize, Deserialize)]
struct Request {
    v: u32,
    agent: String,
    event: String,
    payload: String,
    /// The hook caller asked for stub replacement (version 2 only).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    stub: bool,
}

/// The daemon's hook dispatch: agent, event, payload, the caller's stub setting, and the MMU.
pub type Dispatch =
    fn(AgentKind, &str, &str, bool, &mut InfinityContextMmu) -> Result<String, serde_json::Error>;

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
#[cfg_attr(not(test), allow(dead_code))]
pub fn try_hook(socket: &Path, agent: &str, event: &str, payload: &str) -> Option<String> {
    try_hook_with_stub(socket, agent, event, payload, false)
}

/// [`try_hook`] carrying the caller's stub setting; `stub: true` is sent as version 2.
pub fn try_hook_with_stub(
    socket: &Path,
    agent: &str,
    event: &str,
    payload: &str,
    stub: bool,
) -> Option<String> {
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
        v: if stub {
            PROTOCOL_VERSION_STUB
        } else {
            PROTOCOL_VERSION
        },
        agent: agent.to_owned(),
        event: event.to_owned(),
        payload: payload.to_owned(),
        stub,
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
    dispatch: Dispatch,
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

    // A failed dispatch or save resets the MMU to this state, which mirrors the store it was
    // handed with (or no store), so the next request reloads from disk.
    let pristine = mmu.clone();
    let mut last_request = Instant::now();
    let mut served = 0usize;
    let mut save_failures = 0usize;

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
        match handle_connection(stream, ws_dir, &mut mmu, &pristine, dispatch) {
            Outcome::NotSaved => {}
            Outcome::Saved => save_failures = 0,
            Outcome::SaveFailed(error) => {
                save_failures += 1;
                if save_failures >= MAX_CONSECUTIVE_SAVE_FAILURES {
                    let error = format!("{save_failures} consecutive saves failed; last: {error}");
                    eprintln!("icx daemon: error: {error}; exiting");
                    break Err(error);
                }
                eprintln!(
                    "icx daemon: {error} ({save_failures}/{MAX_CONSECUTIVE_SAVE_FAILURES} \
                     consecutive); state reloads from disk, still serving"
                );
            }
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
fn read_request(stream: &UnixStream, deadline: Duration) -> Result<String, ReadError> {
    let started = Instant::now();
    let mut reader = stream;
    let mut line = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let remaining = deadline
            .checked_sub(started.elapsed())
            .ok_or(ReadError::Closed)?;
        if remaining.is_zero() {
            return Err(ReadError::Closed);
        }
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|_| ReadError::Closed)?;
        let n = match reader.read(&mut chunk) {
            Ok(0) => return Err(ReadError::Closed),
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(ReadError::Closed),
        };
        if let Some(pos) = chunk[..n].iter().position(|&b| b == b'\n') {
            // One request per connection: anything but whitespace after the newline would be
            // silently dropped, so it is a protocol error rather than ignored.
            if !chunk[pos + 1..n].iter().all(u8::is_ascii_whitespace) {
                return Err(ReadError::Protocol(
                    "protocol error: unexpected bytes after the request line \
                     (one request per connection)"
                        .into(),
                ));
            }
            line.extend_from_slice(&chunk[..pos]);
            break;
        }
        line.extend_from_slice(&chunk[..n]);
        if line.len() as u64 > MAX_REQUEST_BYTES {
            return Err(ReadError::Closed);
        }
    }
    if line.len() as u64 > MAX_REQUEST_BYTES {
        return Err(ReadError::Closed);
    }
    String::from_utf8(line).map_err(|_| ReadError::Closed)
}

/// Why [`read_request`] produced no request.
#[derive(Debug)]
enum ReadError {
    /// EOF, I/O error, timeout or an oversized request: the connection is dropped unanswered.
    Closed,
    /// The bytes violate the one-request-per-connection framing; answered with a failure.
    Protocol(String),
}

/// Consecutive failed saves after which the daemon gives up and exits with an error.
pub const MAX_CONSECUTIVE_SAVE_FAILURES: usize = 5;

/// Serves one connection. The request is read and parsed before the workspace lock is taken,
/// so a slow or idle client never blocks in-process hooks or `mcp-serve`; the lock covers only
/// reload, dispatch, save and the reply. Every failure (lock timeout, reload, dispatch, save)
/// fails only this request: the client falls back to in-process handling. Whenever the
/// in-memory state may differ from the store without having been committed, `mmu` is reset to
/// `pristine` so the next request reloads from disk.
fn handle_connection(
    stream: UnixStream,
    ws_dir: &Path,
    mmu: &mut InfinityContextMmu,
    pristine: &InfinityContextMmu,
    dispatch: Dispatch,
) -> Outcome {
    let _ = stream.set_write_timeout(Some(CLIENT_TIMEOUT));
    let line = match read_request(&stream, CLIENT_TIMEOUT) {
        Ok(line) => line,
        Err(ReadError::Closed) => return Outcome::NotSaved,
        Err(ReadError::Protocol(error)) => {
            write_response(stream, &failure(error));
            return Outcome::NotSaved;
        }
    };
    let req = match serde_json::from_str::<Request>(line.trim_end()) {
        // Version 1 never asks for stubs, whatever it carries.
        Ok(req) if req.v == PROTOCOL_VERSION => Request { stub: false, ..req },
        Ok(req) if req.v == PROTOCOL_VERSION_STUB => req,
        Ok(_) => {
            write_response(stream, &failure("unsupported protocol version".into()));
            return Outcome::NotSaved;
        }
        Err(e) => {
            write_response(stream, &failure(format!("malformed request: {e}")));
            return Outcome::NotSaved;
        }
    };

    let lock = match InfinityContextMmu::acquire_workspace_lock(ws_dir) {
        Ok(guard) => guard,
        Err(e) => {
            let error = format!("workspace lock failed: {e}");
            eprintln!("icx daemon: {error}; request refused, still serving");
            write_response(stream, &failure(error));
            return Outcome::NotSaved;
        }
    };
    if let Err(e) = mmu.load_workspace_dir_unlocked(ws_dir) {
        // Nothing was dispatched, so nothing is lost. A partially reloaded MMU is discarded so
        // the next request reloads from disk and never saves it.
        *mmu = pristine.clone();
        let error = format!("workspace load failed: {e}");
        eprintln!("icx daemon: {error}; request refused, still serving");
        write_response(stream, &failure(error));
        return Outcome::NotSaved;
    }
    let agent = AgentKind::parse_agent(&req.agent);
    let out = match dispatch(agent, &req.event, &req.payload, req.stub, mmu) {
        Ok(out) => out,
        Err(e) => {
            // The client falls back to in-process handling and retries this request, so the
            // daemon must not persist whatever the failed dispatch changed: discard it and
            // reload from disk on the next request. The retry is the only ingestion.
            *mmu = pristine.clone();
            write_response(stream, &failure(e.to_string()));
            return Outcome::NotSaved;
        }
    };
    if let Err(error) = mmu.save_workspace_dir_unlocked(ws_dir) {
        // Never acknowledged (ICX-CODEX-030): the client falls back and retries in-process,
        // so the unsaved state is discarded rather than committed by a later save.
        *mmu = pristine.clone();
        let error = format!("workspace save failed; request was not acknowledged: {error}");
        write_response(stream, &failure(error.clone()));
        return Outcome::SaveFailed(error);
    }
    write_response(stream, &Response { ok: true, out });
    drop(lock);
    Outcome::Saved
}

/// What [`handle_connection`] did with the workspace store.
enum Outcome {
    /// Nothing was dispatched, or the dispatch failed and its state was discarded.
    NotSaved,
    /// The request was committed and acknowledged.
    Saved,
    /// The save failed; the client was told so and the in-memory state was discarded.
    SaveFailed(String),
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

    fn durability_dispatch(
        agent: AgentKind,
        event: &str,
        payload: &str,
        _stub: bool,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error> {
        let body = format!("{event} fixture body\n");
        let result = mmu.ingest_payload(agent, "durability fixture", &body, false);
        let ws = Path::new(payload);
        match event {
            "fail-save" => {
                std::fs::write(ws.join("last-aid"), result.artifact_id.to_string()).unwrap();
                std::fs::rename(ws.join("icx.sqlite3"), ws.join("pre-failure.sqlite3")).unwrap();
                std::fs::create_dir(ws.join("icx.sqlite3")).unwrap();
            }
            "fail-dispatch" => {
                std::fs::write(ws.join("last-aid"), result.artifact_id.to_string()).unwrap();
                return Err(serde_json::from_str::<u8>("not json").unwrap_err());
            }
            "conflict" => {
                // Another writer commits between the daemon's reload and its save, so the save
                // fails its optimistic-concurrency check while the store stays loadable.
                let mut other = InfinityContextMmu::default();
                other.load_workspace_dir_unlocked(ws).unwrap();
                other.ingest_payload(agent, "other writer", "other writer body\n", false);
                other.save_workspace_dir_unlocked(ws).unwrap();
            }
            _ => {}
        }
        Ok(result.artifact_id.to_string())
    }

    fn last_aid(ws: &Path) -> icx_core::ArtifactId {
        std::fs::read_to_string(ws.join("last-aid"))
            .unwrap()
            .parse()
            .unwrap()
    }

    fn committed_text(ws: &Path, aid: icx_core::ArtifactId) -> Option<String> {
        let mut reopened = InfinityContextMmu::default();
        reopened.load_workspace_dir_unlocked(ws).unwrap();
        reopened.deref_text(aid).ok()
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
        let ws = std::env::temp_dir()
            .join(format!(
                "icx-v120-tests-{}",
                std::env::var("USER").unwrap_or_default()
            ))
            .join("daemon-tests")
            .join(format!(
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
        assert_eq!(recovered.unwrap(), "ingest fixture body\n");
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

    /// The measurement is a wall-clock bound on taking the workspace lock, which also takes the
    /// process-global `WORKSPACE_IO_LOCK`; other tests in this binary (the lock-timeout scenario
    /// holds it for ~5 s) would be measured with it, so the scenario runs in a child process.
    #[test]
    fn silent_client_does_not_hold_the_workspace_lock() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "daemon::tests::silent_client_scenario",
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("ICX_DAEMON_SILENT_CLIENT_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success(), "silent-client scenario failed: {status}");
    }

    #[test]
    #[ignore = "run in a child process by silent_client_does_not_hold_the_workspace_lock"]
    fn silent_client_scenario() {
        if std::env::var_os("ICX_DAEMON_SILENT_CLIENT_CHILD").is_none() {
            return;
        }
        // Three requests: the readiness check, the silent client and the final hook.
        let (ws, socket, server) = start_durability_fixture("silent", Some(3));
        // One served request first: the daemon holds the lock while it loads the store at
        // startup, which takes longer than a fixed sleep under coverage instrumentation.
        assert!(
            try_hook(&socket, "codex", "ingest", "").is_some(),
            "daemon ready"
        );
        // The daemon may still be persisting that request under the lock; taking the lock once
        // waits it out, so the measurement below covers only the silent client.
        drop(InfinityContextMmu::acquire_workspace_lock(&ws));
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
        assert!(read.is_err(), "trickled request was accepted");
        assert!(
            elapsed < Duration::from_millis(1_000),
            "request read took {elapsed:?} despite a 300 ms deadline"
        );
    }

    #[test]
    fn save_failure_is_not_acknowledged_and_unsaved_state_is_discarded() {
        let (ws, socket, server) = start_durability_fixture("save-error", Some(2));
        let ws_str = ws.to_str().unwrap();
        let output = try_hook(&socket, "codex", "fail-save", ws_str);
        let failed_aid = last_aid(&ws);
        // The fault was transient: put the store back exactly as it was (same generation).
        std::fs::remove_dir(ws.join("icx.sqlite3")).unwrap();
        std::fs::rename(ws.join("pre-failure.sqlite3"), ws.join("icx.sqlite3")).unwrap();
        let next = try_hook(&socket, "codex", "ingest", ws_str);
        let result = server.join().unwrap();
        let leaked = committed_text(&ws, failed_aid);
        let next_text = next
            .as_deref()
            .and_then(|s| s.parse().ok())
            .and_then(|aid| committed_text(&ws, aid));
        std::fs::remove_dir_all(ws).unwrap();
        assert!(
            output.is_none(),
            "failed save returned a successful handle: {output:?}"
        );
        assert_eq!(next_text.as_deref(), Some("ingest fixture body\n"));
        assert!(
            leaked.is_none(),
            "the unacknowledged request was committed by a later save: {leaked:?}"
        );
        assert!(
            result.is_ok(),
            "one transient save failure ended the daemon: {result:?}"
        );
    }

    #[test]
    fn repeated_save_failures_end_the_daemon_with_an_error() {
        let (ws, socket, server) = start_durability_fixture("save-repeat", None);
        let ws_str = ws.to_str().unwrap();
        let mut replies = Vec::new();
        for _ in 0..MAX_CONSECUTIVE_SAVE_FAILURES {
            replies.push(try_hook(&socket, "codex", "conflict", ws_str));
        }
        let result = server.join().unwrap();
        std::fs::remove_dir_all(ws).unwrap();
        assert!(
            replies.iter().all(Option::is_none),
            "a failed save was acknowledged: {replies:?}"
        );
        assert!(
            result.is_err(),
            "repeated save failures were reported as a successful daemon exit"
        );
    }

    #[test]
    fn save_failure_streak_is_reset_by_a_successful_save() {
        let (ws, socket, server) =
            start_durability_fixture("save-reset", Some(2 * MAX_CONSECUTIVE_SAVE_FAILURES));
        let ws_str = ws.to_str().unwrap();
        let mut acked = 0;
        for _ in 0..2 {
            for _ in 0..MAX_CONSECUTIVE_SAVE_FAILURES - 1 {
                assert!(try_hook(&socket, "codex", "conflict", ws_str).is_none());
            }
            acked += usize::from(try_hook(&socket, "codex", "ingest", ws_str).is_some());
        }
        let result = server.join().unwrap();
        std::fs::remove_dir_all(ws).unwrap();
        assert_eq!(acked, 2);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn failed_dispatch_is_not_saved() {
        let (ws, socket, server) = start_durability_fixture("dispatch-error", Some(2));
        let ws_str = ws.to_str().unwrap();
        let generation = InfinityContextMmu::workspace_generation(&ws).unwrap();
        let output = try_hook(&socket, "codex", "fail-dispatch", ws_str);
        let failed_aid = last_aid(&ws);
        let after_failure = committed_text(&ws, failed_aid);
        let generation_after = InfinityContextMmu::workspace_generation(&ws).unwrap();
        // The client retries in-process; the daemon must not also commit the failed request
        // when it saves the next one.
        let next = try_hook(&socket, "codex", "ingest", ws_str);
        let result = server.join().unwrap();
        let after_next = committed_text(&ws, failed_aid);
        std::fs::remove_dir_all(ws).unwrap();
        assert!(output.is_none(), "failed dispatch was acknowledged");
        assert_eq!(generation, generation_after, "a failed dispatch was saved");
        assert!(after_failure.is_none(), "{after_failure:?}");
        assert!(next.is_some());
        assert!(
            after_next.is_none(),
            "failed dispatch state leaked into a later save: {after_next:?}"
        );
        assert!(result.is_ok(), "{result:?}");
    }

    fn raw_exchange(socket: &Path, bytes: &[u8]) -> Response {
        let mut stream = UnixStream::connect(socket).unwrap();
        stream.set_read_timeout(Some(CLIENT_TIMEOUT)).unwrap();
        stream.write_all(bytes).unwrap();
        let mut buf = String::new();
        stream.read_to_string(&mut buf).unwrap();
        serde_json::from_str(buf.trim_end()).unwrap()
    }

    #[test]
    fn trailing_bytes_after_the_request_line_are_rejected() {
        let (ws, socket, server) = start_durability_fixture("trailing", Some(2));
        let generation = InfinityContextMmu::workspace_generation(&ws).unwrap();
        let req = serde_json::to_string(&Request {
            v: PROTOCOL_VERSION,
            agent: "codex".into(),
            event: "ingest".into(),
            payload: String::new(),
            stub: false,
        })
        .unwrap();
        let rejected = raw_exchange(&socket, format!("{req}\n{req}\n").as_bytes());
        let generation_after = InfinityContextMmu::workspace_generation(&ws).unwrap();
        // Trailing whitespace is not a second request.
        let accepted = raw_exchange(&socket, format!("{req}\n \r\n").as_bytes());
        let result = server.join().unwrap();
        std::fs::remove_dir_all(ws).unwrap();
        assert!(
            !rejected.ok,
            "trailing bytes were ignored: {}",
            rejected.out
        );
        assert!(rejected.out.contains("protocol error"), "{}", rejected.out);
        assert_eq!(
            generation, generation_after,
            "rejected request was dispatched"
        );
        assert!(accepted.ok, "{}", accepted.out);
        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn read_request_rejects_bytes_after_the_newline() {
        let (mut client, server) = UnixStream::pair().unwrap();
        client.write_all(b"{\"v\":1}\ntrailing").unwrap();
        let read = read_request(&server, Duration::from_secs(2));
        assert!(
            matches!(read, Err(ReadError::Protocol(_))),
            "trailing bytes accepted: {read:?}"
        );
    }
}
