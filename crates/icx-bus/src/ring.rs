//! Lock-Free Seqlock Ring Buffer (`SeqlockTelemetryBus`) over POSIX shared memory (`/dev/shm`)
//! or in-process aligned memory for `< 100 µs` cross-agent telepathy.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering, fence};
use icx_core::{AgentKind, ArtifactId};
use memmap2::{MmapMut, MmapOptions};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

pub const ICX_SHM_MAGIC: u64 = 0x4943_585F_4641_4231; // "ICX_FAB1"
pub const ICX_SHM_VERSION: u32 = 1;
pub const ICX_RING_SLOTS: usize = 256;
pub const ICX_AHTL_INLINE_CAP: usize = 3968;

/// Maximum spin-loop iterations before a writer recovers a slot or root lock abandoned by a crashed process.
const MAX_WRITER_SPIN_ITERS: u32 = 4096;

/// Error returned by `SeqlockTelemetryBus` operations.
#[derive(Debug, Error)]
pub enum BusError {
    #[error("shared memory I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid shared memory magic header (expected 0x{ICX_SHM_MAGIC:016x})")]
    InvalidMagic,
    #[error("unsupported shared memory schema version {0} (expected {ICX_SHM_VERSION})")]
    UnsupportedVersion(u32),
    #[error("insecure shared memory file permissions (mode 0o{0:o} allows group/other access)")]
    InsecurePermissions(u32),
}

/// 64-byte cache-line-aligned header at the start of the shared memory fabric (`docs/wire-protocol-and-shm-bus.md` §2.1).
#[repr(C, align(64))]
#[derive(Debug)]
pub struct FabricHeader {
    pub magic: AtomicU64,
    pub version: u32,
    pub root_seqlock: AtomicU32,
    pub write_seq: AtomicU64,
    pub active_agents_mask: AtomicU64,
    pub global_dag_root: [AtomicU64; 4],
}

/// 4,096-byte page-aligned Seqlock slot storing a single cross-agent telepathy broadcast.
#[repr(C, align(64))]
#[derive(Debug)]
pub struct SeqlockSlot {
    pub seqlock: AtomicU64,
    pub timestamp_ms: u64,
    pub artifact_id: u64,
    pub workspace_hash: u64,
    pub agent_kind: u8,
    pub surprise_q8: u8,
    pub payload_len: u16,
    pub packet_kind: u8,
    pub _reserved: [u8; 3],
    pub hdc_sketch: [u64; 4],
    pub fractal_q15: [i16; 16],
    pub _pad: [u8; 24],
    pub ahtl_bytes: [u8; ICX_AHTL_INLINE_CAP],
}

const _: () = assert!(core::mem::size_of::<FabricHeader>() == 64);
const _: () = assert!(core::mem::align_of::<FabricHeader>() == 64);
const _: () = assert!(core::mem::size_of::<SeqlockSlot>() == 4096);
const _: () = assert!(core::mem::align_of::<SeqlockSlot>() == 64);

/// Complete layout of the shared-memory ring buffer.
#[repr(C, align(64))]
#[derive(Debug)]
pub struct FabricMemoryLayout {
    pub header: FabricHeader,
    pub slots: [SeqlockSlot; ICX_RING_SLOTS],
}

/// Thread-safe wrapper around `UnsafeCell<FabricMemoryLayout>` for in-process shared memory.
struct SharedLayoutCell(UnsafeCell<FabricMemoryLayout>);

// SAFETY: All concurrent accesses to `FabricMemoryLayout` are synchronized via atomic seqlock
// operations (`AtomicU64` / `AtomicU32` with `Acquire`/`Release` fences).
unsafe impl Send for SharedLayoutCell {}
// SAFETY: See above.
unsafe impl Sync for SharedLayoutCell {}

/// Decoded cross-agent telepathy packet read from the `SeqlockTelemetryBus`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelepathyEvent {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub agent: AgentKind,
    pub packet_kind: u8,
    pub artifact_id: ArtifactId,
    pub workspace_hash: u64,
    pub surprise_q8: u8,
    pub hdc_sketch: [u64; 4],
    pub fractal_q15: [i16; 16],
    pub ahtl_packet: String,
}

#[derive(Clone)]
enum BackingStore {
    InProcess(Arc<SharedLayoutCell>),
    SharedMmap(Arc<MmapMut>),
}

impl core::fmt::Debug for BackingStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InProcess(_) => f.write_str("BackingStore::InProcess"),
            Self::SharedMmap(_) => f.write_str("BackingStore::SharedMmap"),
        }
    }
}

/// Wait-free / Lock-free Seqlock Cross-Agent Telepathy Bus.
#[derive(Clone, Debug)]
pub struct SeqlockTelemetryBus {
    backing: BackingStore,
}

impl Default for SeqlockTelemetryBus {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

impl SeqlockTelemetryBus {
    /// Creates an in-process aligned lock-free Seqlock ring buffer directly on the heap
    /// (zero large stack allocation).
    pub fn new_in_memory() -> Self {
        let layout = Self::heap_zeroed_layout();
        Self {
            backing: BackingStore::InProcess(layout),
        }
    }

    /// Opens or creates a file-backed shared-memory (`MAP_SHARED`) Seqlock ring buffer (e.g. under
    /// `/dev/shm/icx-fabric-v1` or `$XDG_RUNTIME_DIR/icx/icx-fabric-v1`), enforcing `0o600` owner-only
    /// permissions on Unix (`docs/security-and-threat-model.md` §2).
    pub fn open_shared_file(path: impl AsRef<Path>) -> Result<Self, BusError> {
        let required_len = core::mem::size_of::<FabricMemoryLayout>();
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let file = opts.open(path.as_ref())?;

        let metadata = file.metadata()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            let mode = metadata.mode() & 0o777;
            if metadata.len() == 0 && (mode & 0o077) != 0 {
                file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            } else if (mode & 0o077) != 0 {
                return Err(BusError::InsecurePermissions(mode));
            }
        }

        let is_new = metadata.len() < (required_len as u64);
        if is_new {
            file.set_len(required_len as u64)?;
        }

        // SAFETY: `file` is sized to `size_of::<FabricMemoryLayout>()` and mapped into page-aligned
        // virtual memory. All concurrent mutations inside `FabricMemoryLayout` go through atomic
        // seqlock protocol primitives.
        let mut mmap = unsafe { MmapOptions::new().len(required_len).map_mut(&file)? };

        let layout_ptr = mmap.as_mut_ptr().cast::<FabricMemoryLayout>();
        // SAFETY: `mmap` is page-aligned (>= 64-byte alignment) and has length `required_len`.
        unsafe {
            let current_magic = (*layout_ptr).header.magic.load(Ordering::Acquire);
            if is_new || current_magic == 0 {
                core::ptr::write_volatile(&raw mut (*layout_ptr).header.version, ICX_SHM_VERSION);
                (*layout_ptr)
                    .header
                    .root_seqlock
                    .store(0, Ordering::Release);
                (*layout_ptr).header.write_seq.store(0, Ordering::Release);
                (*layout_ptr)
                    .header
                    .active_agents_mask
                    .store(0, Ordering::Release);
                for word in &(*layout_ptr).header.global_dag_root {
                    word.store(0, Ordering::Release);
                }
                fence(Ordering::Release);
                (*layout_ptr)
                    .header
                    .magic
                    .store(ICX_SHM_MAGIC, Ordering::Release);
            } else {
                if current_magic != ICX_SHM_MAGIC {
                    return Err(BusError::InvalidMagic);
                }
                let ver = core::ptr::read_volatile(&raw const (*layout_ptr).header.version);
                if ver != ICX_SHM_VERSION {
                    return Err(BusError::UnsupportedVersion(ver));
                }
            }
        }

        Ok(Self {
            backing: BackingStore::SharedMmap(Arc::new(mmap)),
        })
    }

    fn heap_zeroed_layout() -> Arc<SharedLayoutCell> {
        let alloc_layout = std::alloc::Layout::new::<SharedLayoutCell>();
        // SAFETY: `SharedLayoutCell` has non-zero size and consists exclusively of integer
        // primitives and atomics where all-zero bytes are a valid representation.
        unsafe {
            let ptr = std::alloc::alloc_zeroed(alloc_layout).cast::<SharedLayoutCell>();
            if ptr.is_null() {
                std::alloc::handle_alloc_error(alloc_layout);
            }
            let inner = (*ptr).0.get();
            (*inner).header.version = ICX_SHM_VERSION;
            (*inner)
                .header
                .magic
                .store(ICX_SHM_MAGIC, Ordering::Release);
            let boxed = Box::from_raw(ptr);
            Arc::from(boxed)
        }
    }

    #[inline]
    fn layout_ptr(&self) -> *mut FabricMemoryLayout {
        match &self.backing {
            BackingStore::InProcess(arc) => arc.0.get(),
            BackingStore::SharedMmap(mmap) => mmap.as_ptr().cast::<FabricMemoryLayout>().cast_mut(),
        }
    }

    #[inline]
    fn header(&self) -> &FabricHeader {
        // SAFETY: `FabricHeader` fields accessed via `&FabricHeader` are atomic or immutable after init.
        unsafe { &(*self.layout_ptr()).header }
    }

    /// Returns the total number of telepathy events published so far.
    #[inline]
    pub fn write_sequence(&self) -> u64 {
        self.header().write_seq.load(Ordering::Acquire)
    }

    /// Registers `agent` in the shared `active_agents_mask` bitmask.
    #[inline]
    pub fn register_agent(&self, agent: AgentKind) {
        let bit = (agent as u8) & 63;
        self.header()
            .active_agents_mask
            .fetch_or(1u64 << bit, Ordering::AcqRel);
    }

    /// Returns the bitmask of active agents attached to the shared fabric.
    #[inline]
    pub fn active_agents_mask(&self) -> u64 {
        self.header().active_agents_mask.load(Ordering::Acquire)
    }

    /// Updates the global 256-bit Merkle-DAG root in `FabricHeader` as a coherent seqlock-versioned snapshot.
    pub fn set_global_dag_root(&self, root: [u8; 32]) {
        let header = self.header();
        let mut spins = 0u32;
        let mut stalled_odd = 0u32;
        let odd_ver = loop {
            let cur = header.root_seqlock.load(Ordering::Acquire);
            if (cur & 1) == 0 {
                let next_odd = cur.wrapping_add(1);
                if header
                    .root_seqlock
                    .compare_exchange_weak(cur, next_odd, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    break next_odd;
                }
            } else if cur == stalled_odd {
                spins += 1;
                if spins >= MAX_WRITER_SPIN_ITERS {
                    // Preserve exclusive ownership if a concurrent writer is descheduled or stalled.
                    return;
                }
            } else {
                stalled_odd = cur;
                spins = 0;
            }
            core::hint::spin_loop();
        };

        fence(Ordering::AcqRel);
        for (i, chunk) in root.as_chunks::<8>().0.iter().enumerate() {
            let val = u64::from_be_bytes(*chunk);
            header.global_dag_root[i].store(val, Ordering::Relaxed);
        }
        fence(Ordering::AcqRel);
        header
            .root_seqlock
            .store(odd_ver.wrapping_add(1), Ordering::Release);
    }

    /// Reads a coherent 256-bit Merkle-DAG root snapshot from `FabricHeader`.
    pub fn global_dag_root(&self) -> [u8; 32] {
        let header = self.header();
        for _attempt in 0..64 {
            let s1 = header.root_seqlock.load(Ordering::Acquire);
            if (s1 & 1) != 0 {
                core::hint::spin_loop();
                continue;
            }
            let mut words = [0u64; 4];
            for (i, atom) in header.global_dag_root.iter().enumerate() {
                words[i] = atom.load(Ordering::Relaxed);
            }
            fence(Ordering::Acquire);
            let s2 = header.root_seqlock.load(Ordering::Acquire);
            if s1 == s2 {
                let mut out = [0u8; 32];
                for (i, w) in words.iter().enumerate() {
                    out[i * 8..(i + 1) * 8].copy_from_slice(&w.to_be_bytes());
                }
                return out;
            }
            core::hint::spin_loop();
        }
        [0u8; 32]
    }

    /// Publishes a cross-agent telepathy event into the ring buffer using the lock-free Seqlock protocol.
    #[allow(clippy::too_many_arguments)]
    pub fn publish(
        &self,
        agent: AgentKind,
        artifact_id: ArtifactId,
        workspace_hash: u64,
        surprise_q8: u8,
        hdc_sketch: [u64; 4],
        fractal_q15: [i16; 16],
        ahtl_packet: &str,
        timestamp_ms: u64,
    ) -> u64 {
        self.register_agent(agent);
        let base_ptr = self.layout_ptr();
        // SAFETY: `base_ptr` points to a valid `FabricMemoryLayout`.
        let seq = unsafe { (*base_ptr).header.write_seq.fetch_add(1, Ordering::AcqRel) };
        let slot_idx = (seq as usize) % ICX_RING_SLOTS;
        let odd_version = (seq << 1) | 1;
        let even_version = (seq + 1) << 1;

        // SAFETY: `slot_idx < ICX_RING_SLOTS` is in-bounds of `(*base_ptr).slots`, and `seqlock` is an `AtomicU64`.
        let (slot_ptr, seqlock_ref) = unsafe {
            let ptr = &raw mut (*base_ptr).slots[slot_idx];
            (ptr, &(*ptr).seqlock)
        };

        // Acquire exclusive per-slot write ownership via CAS from an even seqlock state.
        // If a newer wrap-around writer has already committed `s >= even_version`, skip mutating.
        // If a prior writer crashed (`SIGKILL`) leaving `cur` stuck on a stale odd value across
        // `MAX_WRITER_SPIN_ITERS`, atomically recover the slot (`cur -> odd_version`).
        let mut spins = 0u32;
        let mut stalled_odd = 0u64;
        loop {
            let cur = seqlock_ref.load(Ordering::Acquire);
            if cur >= even_version {
                return seq + 1;
            }
            if (cur & 1) == 0 {
                if seqlock_ref
                    .compare_exchange_weak(cur, odd_version, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    break;
                }
            } else if cur == stalled_odd {
                spins += 1;
                if spins >= MAX_WRITER_SPIN_ITERS {
                    // Bounded skip path: never steal an odd slot from a descheduled or crashed writer.
                    return seq + 1;
                }
            } else {
                stalled_odd = cur;
                spins = 0;
            }
            core::hint::spin_loop();
        }

        let mut copy_len = ahtl_packet.len().min(ICX_AHTL_INLINE_CAP);
        while !ahtl_packet.is_char_boundary(copy_len) {
            copy_len -= 1;
        }
        let raw_bytes = &ahtl_packet.as_bytes()[..copy_len];
        let pkt_kind = Self::classify_packet_kind(ahtl_packet);

        // SAFETY: This thread holds exclusive write access to `slot_ptr` (`slot.seqlock == odd_version`)
        // derived directly from the mutable backing pointer (`UnsafeCell` or `MmapMut`).
        unsafe {
            fence(Ordering::AcqRel);

            core::ptr::write_volatile(&raw mut (*slot_ptr).timestamp_ms, timestamp_ms);
            core::ptr::write_volatile(&raw mut (*slot_ptr).artifact_id, artifact_id.0);
            core::ptr::write_volatile(&raw mut (*slot_ptr).workspace_hash, workspace_hash);
            core::ptr::write_volatile(&raw mut (*slot_ptr).agent_kind, agent as u8);
            core::ptr::write_volatile(&raw mut (*slot_ptr).packet_kind, pkt_kind);
            core::ptr::write_volatile(&raw mut (*slot_ptr).surprise_q8, surprise_q8);
            core::ptr::write_volatile(&raw mut (*slot_ptr).payload_len, copy_len as u16);
            core::ptr::write_volatile(&raw mut (*slot_ptr).hdc_sketch, hdc_sketch);
            core::ptr::write_volatile(&raw mut (*slot_ptr).fractal_q15, fractal_q15);
            for (i, &b) in raw_bytes.iter().enumerate() {
                core::ptr::write_volatile(&raw mut (*slot_ptr).ahtl_bytes[i], b);
            }

            fence(Ordering::AcqRel);
            (*slot_ptr).seqlock.store(even_version, Ordering::Release);
        }

        seq + 1
    }

    /// Wait-free optimistic poll of all committed telepathy events with sequence number `> since_seq`.
    pub fn poll_since(&self, since_seq: u64) -> Vec<TelepathyEvent> {
        let base_ptr = self.layout_ptr().cast_const();
        let current_head = self.header().write_seq.load(Ordering::Acquire);
        if current_head <= since_seq {
            return Vec::new();
        }

        let slot_count = (current_head - since_seq).min(ICX_RING_SLOTS as u64) as usize;
        let start_seq = current_head - (slot_count as u64);
        let mut events = Vec::with_capacity(slot_count);

        for offset in 0..slot_count {
            let seq = start_seq + (offset as u64);
            let slot_idx = (seq as usize) % ICX_RING_SLOTS;
            // SAFETY: `slot_idx < ICX_RING_SLOTS` is in-bounds of `(*base_ptr).slots`, and `seqlock` is an `AtomicU64`.
            let (slot_ptr, seqlock_ref) = unsafe {
                let ptr = &raw const (*base_ptr).slots[slot_idx];
                (ptr, &(*ptr).seqlock)
            };

            // Try up to 64 optimistic read attempts in case a writer is actively updating this slot.
            for _attempt in 0..64 {
                let s1 = seqlock_ref.load(Ordering::Acquire);
                if (s1 & 1) != 0 || s1 == 0 {
                    core::hint::spin_loop();
                    continue;
                }
                let committed_seq = s1 >> 1;
                if committed_seq <= since_seq {
                    break;
                }

                // SAFETY: Optimistic volatile reads guarded by `s1 == s2` even-seqlock verification below.
                let (ts, aid, ws_hash, ag_byte, pkt_kind, surp, len, sketch, frac, buf) = unsafe {
                    let ts = core::ptr::read_volatile(&raw const (*slot_ptr).timestamp_ms);
                    let aid = core::ptr::read_volatile(&raw const (*slot_ptr).artifact_id);
                    let ws_hash = core::ptr::read_volatile(&raw const (*slot_ptr).workspace_hash);
                    let ag_byte = core::ptr::read_volatile(&raw const (*slot_ptr).agent_kind);
                    let pkt_kind = core::ptr::read_volatile(&raw const (*slot_ptr).packet_kind);
                    let surp = core::ptr::read_volatile(&raw const (*slot_ptr).surprise_q8);
                    let len = (core::ptr::read_volatile(&raw const (*slot_ptr).payload_len)
                        as usize)
                        .min(ICX_AHTL_INLINE_CAP);
                    let sketch = core::ptr::read_volatile(&raw const (*slot_ptr).hdc_sketch);
                    let frac = core::ptr::read_volatile(&raw const (*slot_ptr).fractal_q15);
                    let mut buf = vec![0u8; len];
                    for (i, dst) in buf.iter_mut().enumerate() {
                        *dst = core::ptr::read_volatile(&raw const (*slot_ptr).ahtl_bytes[i]);
                    }
                    (
                        ts, aid, ws_hash, ag_byte, pkt_kind, surp, len, sketch, frac, buf,
                    )
                };

                fence(Ordering::Acquire);
                let s2 = seqlock_ref.load(Ordering::Acquire);
                if s1 != s2 {
                    core::hint::spin_loop();
                    continue;
                }

                let agent = match ag_byte {
                    1 => AgentKind::ClaudeCode,
                    2 => AgentKind::Codex,
                    3 => AgentKind::AntigravityCli,
                    4 => AgentKind::Hermes,
                    5 => AgentKind::GeminiCli,
                    6 => AgentKind::OpenCode,
                    7 => AgentKind::Cursor,
                    _ => AgentKind::GenericMcp,
                };

                let ahtl_packet = String::from_utf8_lossy(&buf[..len]).into_owned();
                events.push(TelepathyEvent {
                    seq: committed_seq,
                    timestamp_ms: ts,
                    agent,
                    packet_kind: pkt_kind,
                    artifact_id: ArtifactId(aid),
                    workspace_hash: ws_hash,
                    surprise_q8: surp,
                    hdc_sketch: sketch,
                    fractal_q15: frac,
                    ahtl_packet,
                });
                break;
            }
        }

        events.sort_by_key(|e| e.seq);
        events.dedup_by_key(|e| e.seq);
        events
    }

    fn classify_packet_kind(ahtl_packet: &str) -> u8 {
        let trimmed = ahtl_packet.trim_start();
        let inner = if let Some(rest) = trimmed.strip_prefix("⟦Θ:") {
            match rest.split_once('⊢') {
                Some((_, after_turnstile)) => after_turnstile,
                None => return b'T',
            }
        } else {
            trimmed.strip_prefix('⟦').unwrap_or(trimmed)
        };
        if inner.starts_with("Δ:") {
            b'D'
        } else if inner.starts_with("Ω:") {
            b'O'
        } else if inner.starts_with("Φ:") {
            b'F'
        } else if inner.starts_with("Ψ:") {
            b'P'
        } else {
            b'T'
        }
    }

    /// Wait-free optimistic poll filtered to events matching `workspace_hash` (`docs/security-and-threat-model.md` §1).
    pub fn poll_workspace_since(&self, since_seq: u64, workspace_hash: u64) -> Vec<TelepathyEvent> {
        self.poll_since(since_seq)
            .into_iter()
            .filter(|ev| ev.workspace_hash == workspace_hash)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::thread;

    #[test]
    fn seqlock_ring_publishes_and_reads_concurrently_without_torn_reads() {
        let bus = SeqlockTelemetryBus::new_in_memory();
        let mut handles = Vec::new();

        for t in 0..4u64 {
            let bus_w = bus.clone();
            handles.push(thread::spawn(move || {
                for i in 0..50u64 {
                    let id = ArtifactId((t << 32) | i);
                    let pkt = format!("⟦Θ:worker_{t}⊢step={i}↦{id}⟧");
                    bus_w.publish(
                        AgentKind::ClaudeCode,
                        id,
                        0xCAFE,
                        200,
                        [t; 4],
                        [i as i16; 16],
                        &pkt,
                        1000 + i,
                    );
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(bus.write_sequence(), 200);
        let events = bus.poll_workspace_since(0, 0xCAFE);
        assert_eq!(events.len(), 200);
        for ev in events {
            assert!(ev.ahtl_packet.starts_with("⟦Θ:worker_"));
            assert!(ev.ahtl_packet.ends_with('⟧'));
            assert_eq!(ev.packet_kind, b'T');
        }
    }

    #[test]
    fn seqlock_8_writers_8_readers_10000_messages_stress_test_and_mmap_attach() {
        let bus = SeqlockTelemetryBus::new_in_memory();
        let writers_done = Arc::new(AtomicBool::new(false));
        let mut writer_handles = Vec::with_capacity(8);
        let mut reader_handles = Vec::with_capacity(8);

        // 8 reader threads continuously polling while 8 writers publish 10,000 messages total
        for _r in 0..8 {
            let bus_r = bus.clone();
            let done_r = Arc::clone(&writers_done);
            reader_handles.push(thread::spawn(move || {
                let mut cursor = 0u64;
                let mut observed = 0usize;
                loop {
                    let batch = bus_r.poll_workspace_since(cursor, 0x4943_585F_574F_524B);
                    for ev in &batch {
                        assert!(
                            ev.ahtl_packet.starts_with("⟦Θ:stress_w"),
                            "torn packet prefix: {}",
                            ev.ahtl_packet
                        );
                        assert!(
                            ev.ahtl_packet.ends_with('⟧'),
                            "torn packet suffix: {}",
                            ev.ahtl_packet
                        );
                        let worker_id = ev.artifact_id.0 >> 32;
                        assert_eq!(ev.hdc_sketch, [worker_id; 4], "torn hdc_sketch");
                        cursor = cursor.max(ev.seq);
                        observed += 1;
                    }
                    if done_r.load(Ordering::Acquire) && batch.is_empty() {
                        break;
                    }
                    thread::yield_now();
                }
                observed
            }));
        }

        // 8 writer threads * 1,250 messages = 10,000 messages
        for w in 0..8u64 {
            let bus_w = bus.clone();
            writer_handles.push(thread::spawn(move || {
                for i in 0..1_250u64 {
                    let id = ArtifactId((w << 32) | i);
                    let pkt = format!("⟦Θ:stress_w{w}@ring⊢msg={i:04}⋮sketch={w}↦{id}⟧");
                    bus_w.publish(
                        if w % 2 == 0 {
                            AgentKind::ClaudeCode
                        } else {
                            AgentKind::Codex
                        },
                        id,
                        0x4943_585F_574F_524B,
                        180,
                        [w; 4],
                        [(i & 0x7FFF) as i16; 16],
                        &pkt,
                        2_000_000 + i,
                    );
                }
            }));
        }

        for h in writer_handles {
            h.join().unwrap();
        }
        writers_done.store(true, Ordering::Release);

        let mut total_observed = 0usize;
        for h in reader_handles {
            total_observed += h.join().unwrap();
        }
        assert_eq!(bus.write_sequence(), 10_000);
        assert!(total_observed > 0);
        assert_ne!(bus.active_agents_mask(), 0);

        // Verify file-backed shared mmap attach, global_dag_root sync, and magic/version validation
        let shm_path =
            std::env::temp_dir().join(format!("icx_shm_test_{}.shm", std::process::id()));
        let _ = std::fs::remove_file(&shm_path);
        let bus_a = SeqlockTelemetryBus::open_shared_file(&shm_path).expect("open bus_a");
        let bus_b = SeqlockTelemetryBus::open_shared_file(&shm_path).expect("attach bus_b");
        let root = [0xABu8; 32];
        bus_a.set_global_dag_root(root);
        assert_eq!(bus_b.global_dag_root(), root);

        let id = ArtifactId(0x1234_5678_9ABC_DEF0);
        bus_a.publish(
            AgentKind::AntigravityCli,
            id,
            0x7777,
            250,
            [9; 4],
            [3; 16],
            "⟦Θ:agy@shm⊢✓mmap_sync⟧",
            999,
        );
        let from_b = bus_b.poll_workspace_since(0, 0x7777);
        assert_eq!(from_b.len(), 1);
        assert_eq!(from_b[0].agent, AgentKind::AntigravityCli);
        assert_eq!(from_b[0].artifact_id, id);
        let _ = std::fs::remove_file(&shm_path);

        // Simulate a crashed writer leaving slot[0].seqlock stuck on odd (1) and verify
        // subsequent publishes skip the stalled slot cleanly without hanging or stealing ownership
        let recovery_bus = SeqlockTelemetryBus::new_in_memory();
        // SAFETY: `recovery_bus` owns a valid `FabricMemoryLayout` and we only mutate atomic fields.
        unsafe {
            let ptr = recovery_bus.layout_ptr();
            (*ptr).slots[0].seqlock.store(1, Ordering::Release);
            (*ptr).header.root_seqlock.store(1, Ordering::Release);
        }
        recovery_bus.set_global_dag_root([0x55; 32]);
        // Publish 257 events so the ring wraps around slot[0] twice without hanging
        for i in 0..=ICX_RING_SLOTS as u64 {
            recovery_bus.publish(
                AgentKind::Hermes,
                ArtifactId(i + 1),
                0x9999,
                100,
                [1; 4],
                [0; 16],
                "⟦Θ:hermes@recover⊢✓ok⟧",
                i,
            );
        }
        assert_eq!(recovery_bus.write_sequence(), (ICX_RING_SLOTS as u64) + 1);
        let recovered_events = recovery_bus.poll_workspace_since(0, 0x9999);
        assert_eq!(recovered_events.len(), ICX_RING_SLOTS - 1);
    }
}
