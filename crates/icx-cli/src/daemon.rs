//! Resident per-workspace daemon (v1.1.0; ICX-AUD-001, ICX-AUD-047).
//!
//! `icx daemon` keeps one workspace's MMU in memory and serves `icx hook` requests over a Unix
//! socket in the private runtime directory (`$XDG_RUNTIME_DIR/icx`, mode `0700`). Every
//! connection is authenticated with `SO_PEERCRED`: the peer's uid must equal ours.
//!
//! Persistence is write-behind: the reply is sent first, then the daemon keeps the workspace lock
//! while it holds unsaved changes and flushes after `FLUSH_QUIET` of inactivity (at most
//! `FLUSH_MAX` after the first unsaved change). Holding the lock across that window makes the
//! daemon the single writer, so the proxy and MCP server (which take the same lock) never
//! interleave with unsaved state. Each request revalidates the store generation, which picks up
//! their writes cheaply.
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
const FLUSH_QUIET: Duration = Duration::from_millis(50);
const FLUSH_MAX: Duration = Duration::from_secs(1);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
/// Consecutive failed flushes after which unsaved changes are dropped with an error.
const MAX_FLUSH_FAILURES: u32 = 5;

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

    let mut held_lock = None;
    let mut first_dirty: Option<Instant> = None;
    let mut last_request = Instant::now();
    let mut served = 0usize;
    let mut flush_failures = 0u32;

    let result = loop {
        let timeout = match first_dirty {
            Some(first) => {
                let quiet_deadline = last_request + FLUSH_QUIET;
                let max_deadline = first + FLUSH_MAX;
                Some(
                    quiet_deadline
                        .min(max_deadline)
                        .saturating_duration_since(Instant::now()),
                )
            }
            None => opts
                .idle_exit
                .map(|idle| (last_request + idle).saturating_duration_since(Instant::now())),
        };
        let ready = wait_readable(&listener, timeout);
        if flush_due(first_dirty, ready, Instant::now()) {
            let saved = flush(ws_dir, &mut mmu, &mut held_lock);
            held_lock = None; // never hold the workspace lock across a retry wait
            match saved {
                Ok(()) => {
                    first_dirty = None;
                    flush_failures = 0;
                }
                Err(e) => {
                    // Keep the unsaved state in memory and retry after `FLUSH_MAX`. Other icx
                    // processes can use the store meanwhile; the retry reloads only if one of them
                    // advanced it. A persistent failure (full disk, read-only store) is given up
                    // after `MAX_FLUSH_FAILURES` so the daemon can still idle-exit (PR #20 review).
                    flush_failures += 1;
                    if flush_failures >= MAX_FLUSH_FAILURES {
                        eprintln!(
                            "icx daemon: ERROR: giving up after {flush_failures} failed flushes; \
                             unsaved workspace changes are lost: {e}"
                        );
                        first_dirty = None;
                        flush_failures = 0;
                    } else {
                        eprintln!(
                            "icx daemon: flush failed ({flush_failures}/{MAX_FLUSH_FAILURES}), \
                             retrying in {}s: {e}",
                            FLUSH_MAX.as_secs()
                        );
                        let now = Instant::now();
                        first_dirty = Some(now);
                        last_request = now + FLUSH_MAX;
                    }
                }
            }
            continue;
        }
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
        if held_lock.is_none() {
            match InfinityContextMmu::acquire_workspace_lock(ws_dir) {
                Ok(guard) => held_lock = Some(guard),
                Err(e) => {
                    eprintln!("icx daemon: workspace lock failed: {e}");
                    continue;
                }
            }
            if let Err(e) = mmu.load_workspace_dir_unlocked(ws_dir) {
                eprintln!("icx daemon: load failed: {e}");
                held_lock = None;
                continue;
            }
        }
        let dispatched = handle_connection(stream, &mut mmu, dispatch);
        last_request = last_request.max(Instant::now());
        if dispatched {
            first_dirty.get_or_insert(Instant::now());
        } else if first_dirty.is_none() {
            held_lock = None; // nothing changed: do not hold the lock for the write-behind window
        }
        served += 1;
        if opts.max_requests.is_some_and(|max| served >= max) {
            break Ok(());
        }
    };
    if first_dirty.is_some()
        && let Err(e) = flush(ws_dir, &mut mmu, &mut held_lock)
    {
        eprintln!("icx daemon: ERROR: final flush failed; unsaved workspace changes are lost: {e}");
    }
    drop(held_lock);
    let _ = std::fs::remove_file(socket);
    result
}

/// Saves `mmu`, (re)taking the workspace lock first if a failed flush released it. The load is a
/// no-op unless another writer advanced the store since this MMU last loaded or saved it.
fn flush(
    ws_dir: &Path,
    mmu: &mut InfinityContextMmu,
    held_lock: &mut Option<(std::fs::File, std::sync::MutexGuard<'static, ()>)>,
) -> Result<(), icx_core::IcxError> {
    if held_lock.is_none() {
        *held_lock = Some(InfinityContextMmu::acquire_workspace_lock(ws_dir)?);
        mmu.load_workspace_dir_unlocked(ws_dir)?;
    }
    mmu.save_workspace_dir_unlocked(ws_dir)
}

/// True when unsaved changes must be flushed now: after a quiet period (`ready == false` once a
/// deadline passed) or, regardless of queued connections, once `FLUSH_MAX` has elapsed since the
/// first unsaved change. Flushing releases the workspace lock, so a steady stream of hooks can
/// never hold it past `FLUSH_MAX` (PR #20 review).
fn flush_due(first_dirty: Option<Instant>, ready: bool, now: Instant) -> bool {
    first_dirty.is_some_and(|first| !ready || now.saturating_duration_since(first) >= FLUSH_MAX)
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

/// Serves one request; returns `true` if it reached `dispatch` (and so may have changed state).
fn handle_connection(
    stream: UnixStream,
    mmu: &mut InfinityContextMmu,
    dispatch: fn(
        AgentKind,
        &str,
        &str,
        &mut InfinityContextMmu,
    ) -> Result<String, serde_json::Error>,
) -> bool {
    let _ = stream.set_read_timeout(Some(CLIENT_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CLIENT_TIMEOUT));
    let Ok(read_half) = stream.try_clone() else {
        return false;
    };
    let mut reader = BufReader::new(read_half.take(MAX_REQUEST_BYTES));
    let mut line = String::new();
    let mut dispatched = false;
    let resp = match reader.read_line(&mut line) {
        Ok(0) | Err(_) => return false,
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
    let mut w = stream;
    if let Ok(mut bytes) = serde_json::to_vec(&resp) {
        bytes.push(b'\n');
        let _ = w.write_all(&bytes);
        let _ = w.flush();
    }
    dispatched
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flush_max_is_enforced_even_while_connections_are_queued() {
        // PR #20 review (Antigravity): under back-to-back traffic `poll` always reports ready,
        // so the 1 s write-behind ceiling must not depend on a quiet period.
        let now = Instant::now();
        let first = now
            .checked_sub(FLUSH_MAX + Duration::from_millis(1))
            .unwrap();
        assert!(
            flush_due(Some(first), true, now),
            "backlog must not defer past FLUSH_MAX"
        );
        assert!(flush_due(Some(now), false, now), "quiet period flushes");
        assert!(
            !flush_due(Some(now), true, now),
            "fresh changes under load wait"
        );
        assert!(!flush_due(None, false, now), "nothing to flush");
    }
}
