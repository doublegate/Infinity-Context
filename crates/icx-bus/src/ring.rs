//! Lock-Free Seqlock Ring Buffer (`SeqlockTelemetryBus`) over POSIX shared memory (`/dev/shm`)
//! or in-process aligned atomic memory for `< 100 µs` cross-agent telepathy (`#![forbid(unsafe_code)]`).

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering, fence};
use icx_core::{AgentKind, ArtifactId};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;

pub const ICX_SHM_MAGIC: u64 = 0x4943_585F_4641_4231; // "ICX_FAB1"
pub const ICX_SHM_VERSION: u32 = 1;
pub const ICX_RING_SLOTS: usize = 256;
pub const ICX_AHTL_INLINE_CAP: usize = 3968;
pub const ICX_AHTL_INLINE_WORDS: usize = ICX_AHTL_INLINE_CAP / 8;

const HEADER_BYTES: usize = 64;
const SLOT_BYTES: usize = 4096;
const FABRIC_FILE_BYTES: u64 = (HEADER_BYTES + ICX_RING_SLOTS * SLOT_BYTES) as u64;

/// Maximum spin-loop iterations before a writer skips a slot or root lock stalled on an odd version.
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
    /// The writer's slot was already overwritten by a later lap; this event was dropped.
    #[error("telepathy event {0} dropped: ring slot already lapped by a newer writer")]
    SlotLapped(u64),
    /// The slot stayed locked by a stalled writer; this event was dropped.
    #[error("telepathy event {0} dropped: ring slot held by a stalled writer")]
    SlotContended(u64),
}

/// 64-byte cache-line-aligned header at the start of the shared memory fabric (`docs/wire-protocol-and-shm-bus.md` §2.1).
#[repr(C, align(64))]
#[derive(Debug)]
pub struct FabricHeader {
    pub magic: AtomicU64,
    pub version: AtomicU32,
    pub root_seqlock: AtomicU32,
    pub write_seq: AtomicU64,
    pub active_agents_mask: AtomicU64,
    pub global_dag_root: [AtomicU64; 4],
}

impl FabricHeader {
    /// Creates an initialized 64-byte `FabricHeader`.
    pub fn new() -> Self {
        Self {
            magic: AtomicU64::new(ICX_SHM_MAGIC),
            version: AtomicU32::new(ICX_SHM_VERSION),
            root_seqlock: AtomicU32::new(0),
            write_seq: AtomicU64::new(0),
            active_agents_mask: AtomicU64::new(0),
            global_dag_root: core::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl Default for FabricHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// 4,096-byte page-aligned Seqlock slot storing a single cross-agent telepathy broadcast using
/// 100% safe atomic word cells (`#![forbid(unsafe_code)]`).
#[repr(C, align(64))]
#[derive(Debug)]
pub struct SeqlockSlot {
    pub seqlock: AtomicU64,
    pub timestamp_ms: AtomicU64,
    pub artifact_id: AtomicU64,
    pub workspace_hash: AtomicU64,
    /// Packed metadata word:
    /// - bits `[0..8)`: `agent_kind: u8`
    /// - bits `[8..16)`: `surprise_q8: u8`
    /// - bits `[16..32)`: `payload_len: u16`
    /// - bits `[32..40)`: `packet_kind: u8`
    /// - bits `[40..64)`: reserved (`0`)
    pub packed_meta: AtomicU64,
    pub hdc_sketch: [AtomicU64; 4],
    pub fractal_q15_words: [AtomicU64; 4],
    pub reserved_pad: [AtomicU64; 3],
    pub ahtl_words: [AtomicU64; ICX_AHTL_INLINE_WORDS],
}

impl SeqlockSlot {
    /// Creates a zero-initialized 4,096-byte `SeqlockSlot`.
    pub fn new() -> Self {
        Self {
            seqlock: AtomicU64::new(0),
            timestamp_ms: AtomicU64::new(0),
            artifact_id: AtomicU64::new(0),
            workspace_hash: AtomicU64::new(0),
            packed_meta: AtomicU64::new(0),
            hdc_sketch: core::array::from_fn(|_| AtomicU64::new(0)),
            fractal_q15_words: core::array::from_fn(|_| AtomicU64::new(0)),
            reserved_pad: core::array::from_fn(|_| AtomicU64::new(0)),
            ahtl_words: core::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl Default for SeqlockSlot {
    fn default() -> Self {
        Self::new()
    }
}

const _: () = assert!(core::mem::size_of::<FabricHeader>() == HEADER_BYTES);
const _: () = assert!(core::mem::align_of::<FabricHeader>() == 64);
const _: () = assert!(core::mem::size_of::<SeqlockSlot>() == SLOT_BYTES);
const _: () = assert!(core::mem::align_of::<SeqlockSlot>() == 64);

/// Complete layout of the in-process atomic Seqlock ring buffer (heap-allocated slot slice).
#[repr(C, align(64))]
#[derive(Debug)]
pub struct FabricMemoryLayout {
    pub header: FabricHeader,
    pub slots: Box<[SeqlockSlot]>,
}

impl FabricMemoryLayout {
    /// Allocates the `FabricHeader` and `ICX_RING_SLOTS` page-aligned `SeqlockSlot`s directly on the heap.
    pub fn new() -> Self {
        let slots: Box<[SeqlockSlot]> = std::iter::repeat_with(SeqlockSlot::new)
            .take(ICX_RING_SLOTS)
            .collect();
        Self {
            header: FabricHeader::new(),
            slots,
        }
    }
}

impl Default for FabricMemoryLayout {
    fn default() -> Self {
        Self::new()
    }
}

struct FileUnlockGuard<'a>(&'a File);

impl Drop for FileUnlockGuard<'_> {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// Safe positional `/dev/shm` file store synchronizing cross-process telepathy over the 1 MiB binary layout.
#[derive(Debug)]
struct SharedFileStore {
    file: File,
    write_guard: Mutex<()>,
    #[cfg(not(unix))]
    cursor_guard: Mutex<()>,
}

impl SharedFileStore {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;
            self.file.read_exact_at(buf, offset)
        }
        #[cfg(not(unix))]
        {
            use std::io::{Read, Seek, SeekFrom};
            let _guard = self
                .cursor_guard
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut f = &self.file;
            f.seek(SeekFrom::Start(offset))?;
            f.read_exact(buf)
        }
    }

    fn write_at(&self, offset: u64, buf: &[u8]) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileExt;
            self.file.write_all_at(buf, offset)
        }
        #[cfg(not(unix))]
        {
            use std::io::{Seek, SeekFrom, Write};
            let _guard = self
                .cursor_guard
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut f = &self.file;
            f.seek(SeekFrom::Start(offset))?;
            f.write_all(buf)
        }
    }

    fn with_exclusive_lock<T>(&self, f: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
        let _thread_guard = self
            .write_guard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for _attempt in 0..5_000u32 {
            match self.file.try_lock() {
                Ok(()) => {
                    let _unlock = FileUnlockGuard(&self.file);
                    return f();
                }
                Err(std::fs::TryLockError::WouldBlock) => {
                    std::thread::sleep(std::time::Duration::from_micros(50));
                }
                Err(std::fs::TryLockError::Error(e)) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "timed out acquiring shared memory file lock",
        ))
    }
}

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

#[derive(Clone, Debug)]
enum BackingStore {
    InProcess(Arc<FabricMemoryLayout>),
    SharedFile(Arc<SharedFileStore>),
}

/// Wait-free / Lock-free Seqlock Cross-Agent Telepathy Bus (`#![forbid(unsafe_code)]`).
#[derive(Clone, Debug)]
pub struct SeqlockTelemetryBus {
    backing: BackingStore,
    /// Publishes this handle had to drop (lapped or contended slot), ICX-AUD-054.
    dropped: Arc<std::sync::atomic::AtomicU64>,
}

impl Default for SeqlockTelemetryBus {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

#[inline]
fn pack_meta(agent_kind: u8, surprise_q8: u8, payload_len: u16, packet_kind: u8) -> u64 {
    u64::from(agent_kind)
        | (u64::from(surprise_q8) << 8)
        | (u64::from(payload_len) << 16)
        | (u64::from(packet_kind) << 32)
}

#[inline]
fn unpack_meta(word: u64) -> (u8, u8, usize, u8) {
    let agent_kind = (word & 0xFF) as u8;
    let surprise_q8 = ((word >> 8) & 0xFF) as u8;
    let payload_len = (((word >> 16) & 0xFFFF) as usize).min(ICX_AHTL_INLINE_CAP);
    let packet_kind = ((word >> 32) & 0xFF) as u8;
    (agent_kind, surprise_q8, payload_len, packet_kind)
}

#[inline]
fn pack_fractal_q15(fractal_q15: [i16; 16]) -> [u64; 4] {
    let mut out = [0u64; 4];
    for (w_idx, dst) in out.iter_mut().enumerate() {
        let base = w_idx * 4;
        let b0 = (fractal_q15[base] as u16).to_le_bytes();
        let b1 = (fractal_q15[base + 1] as u16).to_le_bytes();
        let b2 = (fractal_q15[base + 2] as u16).to_le_bytes();
        let b3 = (fractal_q15[base + 3] as u16).to_le_bytes();
        *dst = u64::from_le_bytes([b0[0], b0[1], b1[0], b1[1], b2[0], b2[1], b3[0], b3[1]]);
    }
    out
}

#[inline]
fn unpack_fractal_q15(words: [u64; 4]) -> [i16; 16] {
    let mut out = [0i16; 16];
    for (w_idx, &word) in words.iter().enumerate() {
        let bytes = word.to_le_bytes();
        let base = w_idx * 4;
        out[base] = i16::from_le_bytes([bytes[0], bytes[1]]);
        out[base + 1] = i16::from_le_bytes([bytes[2], bytes[3]]);
        out[base + 2] = i16::from_le_bytes([bytes[4], bytes[5]]);
        out[base + 3] = i16::from_le_bytes([bytes[6], bytes[7]]);
    }
    out
}

#[inline]
fn decode_agent_kind(ag_byte: u8) -> AgentKind {
    match ag_byte {
        1 => AgentKind::ClaudeCode,
        2 => AgentKind::Codex,
        3 => AgentKind::AntigravityCli,
        4 => AgentKind::Hermes,
        5 => AgentKind::GeminiCli,
        6 => AgentKind::OpenCode,
        7 => AgentKind::Cursor,
        _ => AgentKind::GenericMcp,
    }
}

impl SeqlockTelemetryBus {
    /// Creates an in-process aligned lock-free Seqlock ring buffer directly on the heap
    /// (zero large stack allocation, zero `unsafe`).
    pub fn new_in_memory() -> Self {
        Self {
            backing: BackingStore::InProcess(Arc::new(FabricMemoryLayout::new())),
            dropped: Arc::default(),
        }
    }

    /// Opens or creates a file-backed shared-memory Seqlock ring buffer (e.g. under
    /// `/dev/shm/icx-fabric-v1` or `.icx/icx-fabric-v1.shm`), enforcing `0o600` owner-only
    /// permissions on Unix (`docs/security-and-threat-model.md` §2).
    pub fn open_shared_file(path: impl AsRef<Path>) -> Result<Self, BusError> {
        let mut opts = OpenOptions::new();
        opts.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
            // ICX-AUD-019: never follow a planted symlink; don't leak the fd into children.
            let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC;
            opts.custom_flags(flags.bits().cast_signed());
        }
        let file = opts.open(path.as_ref())?;

        let metadata = file.metadata()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if !metadata.file_type().is_file() {
                return Err(BusError::InsecurePermissions(metadata.mode() & 0o7777));
            }
            if metadata.uid() != rustix::process::geteuid().as_raw() {
                return Err(BusError::InsecurePermissions(metadata.mode() & 0o777));
            }
            let mode = metadata.mode() & 0o777;
            if (mode & 0o077) != 0 {
                // Refuse instead of silently tightening a file someone else may have prepared.
                return Err(BusError::InsecurePermissions(mode));
            }
        }

        let store = Arc::new(SharedFileStore {
            file,
            write_guard: Mutex::new(()),
            #[cfg(not(unix))]
            cursor_guard: Mutex::new(()),
        });

        let mut validation_err: Option<BusError> = None;
        store.with_exclusive_lock(|| {
            let current_len = store.file.metadata()?.len();
            let is_new = current_len < FABRIC_FILE_BYTES;
            if is_new {
                store.file.set_len(FABRIC_FILE_BYTES)?;
            }

            let mut hdr_buf = [0u8; HEADER_BYTES];
            store.read_at(0, &mut hdr_buf)?;
            let current_magic = u64::from_le_bytes([
                hdr_buf[0], hdr_buf[1], hdr_buf[2], hdr_buf[3], hdr_buf[4], hdr_buf[5], hdr_buf[6],
                hdr_buf[7],
            ]);

            if is_new || current_magic == 0 {
                let mut init_hdr = [0u8; HEADER_BYTES];
                init_hdr[0..8].copy_from_slice(&ICX_SHM_MAGIC.to_le_bytes());
                init_hdr[8..12].copy_from_slice(&ICX_SHM_VERSION.to_le_bytes());
                store.write_at(0, &init_hdr)?;
            } else {
                if current_magic != ICX_SHM_MAGIC {
                    validation_err = Some(BusError::InvalidMagic);
                    return Ok(());
                }
                let ver = u32::from_le_bytes([hdr_buf[8], hdr_buf[9], hdr_buf[10], hdr_buf[11]]);
                if ver != ICX_SHM_VERSION {
                    validation_err = Some(BusError::UnsupportedVersion(ver));
                    return Ok(());
                }
            }
            Ok(())
        })?;

        if let Some(err) = validation_err {
            return Err(err);
        }

        Ok(Self {
            backing: BackingStore::SharedFile(store),
            dropped: Arc::default(),
        })
    }

    /// Returns a reference to the in-process `FabricMemoryLayout` when backed by in-process memory.
    pub fn in_process_layout(&self) -> Option<&FabricMemoryLayout> {
        match &self.backing {
            BackingStore::InProcess(layout) => Some(layout),
            BackingStore::SharedFile(_) => None,
        }
    }

    /// Returns the total number of telepathy events published so far.
    #[inline]
    pub fn write_sequence(&self) -> u64 {
        match &self.backing {
            BackingStore::InProcess(layout) => layout.header.write_seq.load(Ordering::Acquire),
            BackingStore::SharedFile(store) => {
                let mut buf = [0u8; 8];
                if store.read_at(16, &mut buf).is_ok() {
                    u64::from_le_bytes(buf)
                } else {
                    0
                }
            }
        }
    }

    /// Registers `agent` in the shared `active_agents_mask` bitmask.
    #[inline]
    pub fn register_agent(&self, agent: AgentKind) {
        let bit = (agent as u8) & 63;
        match &self.backing {
            BackingStore::InProcess(layout) => {
                layout
                    .header
                    .active_agents_mask
                    .fetch_or(1u64 << bit, Ordering::AcqRel);
            }
            BackingStore::SharedFile(store) => {
                let _ = store.with_exclusive_lock(|| {
                    let mut buf = [0u8; 8];
                    store.read_at(24, &mut buf)?;
                    let mask = u64::from_le_bytes(buf) | (1u64 << bit);
                    store.write_at(24, &mask.to_le_bytes())
                });
            }
        }
    }

    /// Returns the bitmask of active agents attached to the shared fabric.
    #[inline]
    pub fn active_agents_mask(&self) -> u64 {
        match &self.backing {
            BackingStore::InProcess(layout) => {
                layout.header.active_agents_mask.load(Ordering::Acquire)
            }
            BackingStore::SharedFile(store) => {
                let mut buf = [0u8; 8];
                if store.read_at(24, &mut buf).is_ok() {
                    u64::from_le_bytes(buf)
                } else {
                    0
                }
            }
        }
    }

    /// Updates the global 256-bit Merkle-DAG root in `FabricHeader` as a coherent seqlock-versioned snapshot.
    pub fn set_global_dag_root(&self, root: [u8; 32]) {
        match &self.backing {
            BackingStore::InProcess(layout) => {
                let header = &layout.header;
                let mut spins = 0u32;
                let mut stalled_odd = 0u32;
                let odd_ver = loop {
                    let cur = header.root_seqlock.load(Ordering::Acquire);
                    if (cur & 1) == 0 {
                        let next_odd = cur.wrapping_add(1);
                        if header
                            .root_seqlock
                            .compare_exchange_weak(
                                cur,
                                next_odd,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            )
                            .is_ok()
                        {
                            break next_odd;
                        }
                    } else if cur == stalled_odd {
                        spins += 1;
                        if spins >= MAX_WRITER_SPIN_ITERS {
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
            BackingStore::SharedFile(store) => {
                let mut encoded_root = [0u8; 32];
                for (i, chunk) in root.as_chunks::<8>().0.iter().enumerate() {
                    let word = u64::from_be_bytes(*chunk);
                    encoded_root[i * 8..(i + 1) * 8].copy_from_slice(&word.to_le_bytes());
                }
                let _ = store.with_exclusive_lock(|| {
                    let mut seq_buf = [0u8; 4];
                    store.read_at(12, &mut seq_buf)?;
                    let cur = u32::from_le_bytes(seq_buf);
                    let odd_ver = (cur & !1).wrapping_add(1);
                    store.write_at(12, &odd_ver.to_le_bytes())?;
                    store.write_at(32, &encoded_root)?;
                    let even_ver = odd_ver.wrapping_add(1);
                    store.write_at(12, &even_ver.to_le_bytes())
                });
            }
        }
    }

    /// Reads a coherent 256-bit Merkle-DAG root snapshot from `FabricHeader`.
    pub fn global_dag_root(&self) -> [u8; 32] {
        match &self.backing {
            BackingStore::InProcess(layout) => {
                let header = &layout.header;
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
            BackingStore::SharedFile(store) => {
                for _attempt in 0..64 {
                    let mut seq_buf1 = [0u8; 4];
                    if store.read_at(12, &mut seq_buf1).is_err() {
                        break;
                    }
                    let s1 = u32::from_le_bytes(seq_buf1);
                    if (s1 & 1) != 0 {
                        core::hint::spin_loop();
                        continue;
                    }
                    let mut raw_words = [0u8; 32];
                    if store.read_at(32, &mut raw_words).is_err() {
                        break;
                    }
                    let mut seq_buf2 = [0u8; 4];
                    if store.read_at(12, &mut seq_buf2).is_err() {
                        break;
                    }
                    let s2 = u32::from_le_bytes(seq_buf2);
                    if s1 == s2 {
                        let mut out = [0u8; 32];
                        for (i, chunk) in raw_words.as_chunks::<8>().0.iter().enumerate() {
                            let word = u64::from_le_bytes(*chunk);
                            out[i * 8..(i + 1) * 8].copy_from_slice(&word.to_be_bytes());
                        }
                        return out;
                    }
                    core::hint::spin_loop();
                }
                [0u8; 32]
            }
        }
    }

    /// Publishes a cross-agent telepathy event into the ring buffer, returning `Result<u64, BusError>`.
    #[allow(clippy::too_many_arguments)]
    pub fn try_publish(
        &self,
        agent: AgentKind,
        artifact_id: ArtifactId,
        workspace_hash: u64,
        surprise_q8: u8,
        hdc_sketch: [u64; 4],
        fractal_q15: [i16; 16],
        ahtl_packet: &str,
        timestamp_ms: u64,
    ) -> Result<u64, BusError> {
        let mut copy_len = ahtl_packet.len().min(ICX_AHTL_INLINE_CAP);
        while !ahtl_packet.is_char_boundary(copy_len) {
            copy_len -= 1;
        }
        let raw_bytes = &ahtl_packet.as_bytes()[..copy_len];
        let pkt_kind = Self::classify_packet_kind(ahtl_packet);
        let meta_word = pack_meta(agent as u8, surprise_q8, copy_len as u16, pkt_kind);
        let fractal_words = pack_fractal_q15(fractal_q15);

        match &self.backing {
            BackingStore::InProcess(layout) => {
                self.register_agent(agent);
                let seq = layout.header.write_seq.fetch_add(1, Ordering::AcqRel);
                let slot_idx = (seq as usize) % ICX_RING_SLOTS;
                let odd_version = (seq << 1) | 1;
                let even_version = (seq + 1) << 1;
                let slot = &layout.slots[slot_idx];

                let mut spins = 0u32;
                let mut stalled_odd = 0u64;
                loop {
                    let cur = slot.seqlock.load(Ordering::Acquire);
                    if cur >= even_version {
                        return Err(BusError::SlotLapped(seq + 1));
                    }
                    if (cur & 1) == 0 {
                        if slot
                            .seqlock
                            .compare_exchange_weak(
                                cur,
                                odd_version,
                                Ordering::AcqRel,
                                Ordering::Acquire,
                            )
                            .is_ok()
                        {
                            break;
                        }
                    } else if cur == stalled_odd {
                        spins += 1;
                        if spins >= MAX_WRITER_SPIN_ITERS {
                            return Err(BusError::SlotContended(seq + 1));
                        }
                    } else {
                        stalled_odd = cur;
                        spins = 0;
                    }
                    core::hint::spin_loop();
                }

                fence(Ordering::AcqRel);
                slot.timestamp_ms.store(timestamp_ms, Ordering::Relaxed);
                slot.artifact_id.store(artifact_id.0, Ordering::Relaxed);
                slot.workspace_hash.store(workspace_hash, Ordering::Relaxed);
                slot.packed_meta.store(meta_word, Ordering::Relaxed);
                for (i, &w) in hdc_sketch.iter().enumerate() {
                    slot.hdc_sketch[i].store(w, Ordering::Relaxed);
                }
                for (i, &w) in fractal_words.iter().enumerate() {
                    slot.fractal_q15_words[i].store(w, Ordering::Relaxed);
                }

                let full_words = copy_len / 8;
                let (chunks, rem) = raw_bytes.as_chunks::<8>();
                for (i, chunk) in chunks.iter().enumerate() {
                    slot.ahtl_words[i].store(u64::from_le_bytes(*chunk), Ordering::Relaxed);
                }
                if !rem.is_empty() {
                    let mut tail = [0u8; 8];
                    tail[..rem.len()].copy_from_slice(rem);
                    slot.ahtl_words[full_words].store(u64::from_le_bytes(tail), Ordering::Relaxed);
                }

                fence(Ordering::AcqRel);
                slot.seqlock.store(even_version, Ordering::Release);
                Ok(seq + 1)
            }
            BackingStore::SharedFile(store) => {
                let bit = (agent as u8) & 63;
                let seq = store.with_exclusive_lock(|| {
                    let mut hdr_slice = [0u8; 16];
                    store.read_at(16, &mut hdr_slice)?;
                    let seq = u64::from_le_bytes([
                        hdr_slice[0],
                        hdr_slice[1],
                        hdr_slice[2],
                        hdr_slice[3],
                        hdr_slice[4],
                        hdr_slice[5],
                        hdr_slice[6],
                        hdr_slice[7],
                    ]);
                    let mask = u64::from_le_bytes([
                        hdr_slice[8],
                        hdr_slice[9],
                        hdr_slice[10],
                        hdr_slice[11],
                        hdr_slice[12],
                        hdr_slice[13],
                        hdr_slice[14],
                        hdr_slice[15],
                    ]) | (1u64 << bit);

                    let next_seq = seq + 1;
                    let slot_idx = (seq as usize) % ICX_RING_SLOTS;
                    let slot_offset = (HEADER_BYTES + slot_idx * SLOT_BYTES) as u64;
                    let odd_version = (seq << 1) | 1;
                    let even_version = next_seq << 1;

                    // 1. Mark slot as odd at offset 0..8 and write the payload bytes at offset 8..128+copy_len
                    // so concurrent readers never observe even_version while payload bytes are in flight.
                    store.write_at(slot_offset, &odd_version.to_le_bytes())?;

                    let mut slot_buf = [0u8; SLOT_BYTES];
                    slot_buf[8..16].copy_from_slice(&timestamp_ms.to_le_bytes());
                    slot_buf[16..24].copy_from_slice(&artifact_id.0.to_le_bytes());
                    slot_buf[24..32].copy_from_slice(&workspace_hash.to_le_bytes());
                    slot_buf[32..40].copy_from_slice(&meta_word.to_le_bytes());
                    for (i, &w) in hdc_sketch.iter().enumerate() {
                        slot_buf[40 + i * 8..48 + i * 8].copy_from_slice(&w.to_le_bytes());
                    }
                    for (i, &w) in fractal_words.iter().enumerate() {
                        slot_buf[72 + i * 8..80 + i * 8].copy_from_slice(&w.to_le_bytes());
                    }
                    slot_buf[128..128 + copy_len].copy_from_slice(raw_bytes);
                    store.write_at(slot_offset + 8, &slot_buf[8..128 + copy_len])?;

                    // 2. Commit even_version to slot_offset (0..8) only after all payload bytes are written.
                    store.write_at(slot_offset, &even_version.to_le_bytes())?;

                    // 3. Advance header.write_seq to next_seq only after the slot is committed.
                    let mut updated_hdr = [0u8; 16];
                    updated_hdr[0..8].copy_from_slice(&next_seq.to_le_bytes());
                    updated_hdr[8..16].copy_from_slice(&mask.to_le_bytes());
                    store.write_at(16, &updated_hdr)?;
                    Ok(next_seq)
                })?;
                Ok(seq)
            }
        }
    }

    /// Publishes a cross-agent telepathy event into the ring buffer using the Seqlock protocol.
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
        self.try_publish(
            agent,
            artifact_id,
            workspace_hash,
            surprise_q8,
            hdc_sketch,
            fractal_q15,
            ahtl_packet,
            timestamp_ms,
        )
        .unwrap_or_else(|_| {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            self.write_sequence()
        })
    }

    /// Number of publishes this handle dropped because the slot was lapped or held by a stalled
    /// writer (ICX-AUD-054). `publish` swallows those errors; this makes them observable.
    pub fn dropped_publishes(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Like [`Self::poll_since`], but also returns how many sequence numbers after `since_seq`
    /// could not be delivered (overwritten by the ring before this reader caught up, or torn and
    /// skipped). A non-zero count means the reader must not assume it saw every event.
    pub fn poll_since_with_loss(&self, since_seq: u64) -> (Vec<TelepathyEvent>, u64) {
        let head = self.write_sequence();
        let events = self.poll_since(since_seq);
        let expected = head.saturating_sub(since_seq);
        let lost = expected.saturating_sub(events.len() as u64);
        (events, lost)
    }

    /// Wait-free optimistic poll of all committed telepathy events with sequence number `> since_seq`.
    pub fn poll_since(&self, since_seq: u64) -> Vec<TelepathyEvent> {
        let current_head = self.write_sequence();
        if current_head <= since_seq {
            return Vec::new();
        }

        let slot_count = (current_head - since_seq).min(ICX_RING_SLOTS as u64) as usize;
        let start_seq = current_head - (slot_count as u64);
        let mut events = Vec::with_capacity(slot_count);

        match &self.backing {
            BackingStore::InProcess(layout) => {
                for offset in 0..slot_count {
                    let seq = start_seq + (offset as u64);
                    let slot_idx = (seq as usize) % ICX_RING_SLOTS;
                    let slot = &layout.slots[slot_idx];

                    for _attempt in 0..64 {
                        let s1 = slot.seqlock.load(Ordering::Acquire);
                        if (s1 & 1) != 0 || s1 == 0 {
                            core::hint::spin_loop();
                            continue;
                        }
                        let committed_seq = s1 >> 1;
                        if committed_seq <= since_seq {
                            break;
                        }

                        let ts = slot.timestamp_ms.load(Ordering::Relaxed);
                        let aid = slot.artifact_id.load(Ordering::Relaxed);
                        let ws_hash = slot.workspace_hash.load(Ordering::Relaxed);
                        let meta = slot.packed_meta.load(Ordering::Relaxed);
                        let (ag_byte, surp, len, pkt_kind) = unpack_meta(meta);

                        let mut sketch = [0u64; 4];
                        for (i, atom) in slot.hdc_sketch.iter().enumerate() {
                            sketch[i] = atom.load(Ordering::Relaxed);
                        }
                        let mut frac_words = [0u64; 4];
                        for (i, atom) in slot.fractal_q15_words.iter().enumerate() {
                            frac_words[i] = atom.load(Ordering::Relaxed);
                        }
                        let word_count = len.div_ceil(8);
                        let mut buf = vec![0u8; word_count * 8];
                        for i in 0..word_count {
                            let w = slot.ahtl_words[i].load(Ordering::Relaxed);
                            buf[i * 8..(i + 1) * 8].copy_from_slice(&w.to_le_bytes());
                        }

                        fence(Ordering::Acquire);
                        let s2 = slot.seqlock.load(Ordering::Acquire);
                        if s1 != s2 {
                            core::hint::spin_loop();
                            continue;
                        }

                        let ahtl_packet = String::from_utf8_lossy(&buf[..len]).into_owned();
                        events.push(TelepathyEvent {
                            seq: committed_seq,
                            timestamp_ms: ts,
                            agent: decode_agent_kind(ag_byte),
                            packet_kind: pkt_kind,
                            artifact_id: ArtifactId(aid),
                            workspace_hash: ws_hash,
                            surprise_q8: surp,
                            hdc_sketch: sketch,
                            fractal_q15: unpack_fractal_q15(frac_words),
                            ahtl_packet,
                        });
                        break;
                    }
                }
            }
            BackingStore::SharedFile(store) => {
                let mut slot_buf = [0u8; SLOT_BYTES];
                for offset in 0..slot_count {
                    let seq = start_seq + (offset as u64);
                    let slot_idx = (seq as usize) % ICX_RING_SLOTS;
                    let slot_offset = (HEADER_BYTES + slot_idx * SLOT_BYTES) as u64;

                    for _attempt in 0..64 {
                        let mut s1_buf = [0u8; 8];
                        if store.read_at(slot_offset, &mut s1_buf).is_err() {
                            break;
                        }
                        let s1 = u64::from_le_bytes(s1_buf);
                        if (s1 & 1) != 0 || s1 == 0 {
                            core::hint::spin_loop();
                            continue;
                        }
                        let committed_seq = s1 >> 1;
                        if committed_seq <= since_seq {
                            break;
                        }

                        if store.read_at(slot_offset, &mut slot_buf).is_err() {
                            break;
                        }
                        let mut s2_buf = [0u8; 8];
                        if store.read_at(slot_offset, &mut s2_buf).is_err() {
                            break;
                        }
                        let s2 = u64::from_le_bytes(s2_buf);
                        let slot_s = u64::from_le_bytes([
                            slot_buf[0],
                            slot_buf[1],
                            slot_buf[2],
                            slot_buf[3],
                            slot_buf[4],
                            slot_buf[5],
                            slot_buf[6],
                            slot_buf[7],
                        ]);
                        if s1 != s2 || slot_s != s1 {
                            core::hint::spin_loop();
                            continue;
                        }

                        let ts = u64::from_le_bytes(slot_buf[8..16].try_into().unwrap_or([0; 8]));
                        let aid = u64::from_le_bytes(slot_buf[16..24].try_into().unwrap_or([0; 8]));
                        let ws_hash =
                            u64::from_le_bytes(slot_buf[24..32].try_into().unwrap_or([0; 8]));
                        let meta =
                            u64::from_le_bytes(slot_buf[32..40].try_into().unwrap_or([0; 8]));
                        let (ag_byte, surp, len, pkt_kind) = unpack_meta(meta);

                        let mut sketch = [0u64; 4];
                        for (i, dst) in sketch.iter_mut().enumerate() {
                            *dst = u64::from_le_bytes(
                                slot_buf[40 + i * 8..48 + i * 8]
                                    .try_into()
                                    .unwrap_or([0; 8]),
                            );
                        }
                        let mut frac_words = [0u64; 4];
                        for (i, dst) in frac_words.iter_mut().enumerate() {
                            *dst = u64::from_le_bytes(
                                slot_buf[72 + i * 8..80 + i * 8]
                                    .try_into()
                                    .unwrap_or([0; 8]),
                            );
                        }
                        let ahtl_packet =
                            String::from_utf8_lossy(&slot_buf[128..128 + len]).into_owned();
                        events.push(TelepathyEvent {
                            seq: committed_seq,
                            timestamp_ms: ts,
                            agent: decode_agent_kind(ag_byte),
                            packet_kind: pkt_kind,
                            artifact_id: ArtifactId(aid),
                            workspace_hash: ws_hash,
                            surprise_q8: surp,
                            hdc_sketch: sketch,
                            fractal_q15: unpack_fractal_q15(frac_words),
                            ahtl_packet,
                        });
                        break;
                    }
                }
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
        let bus = SeqlockTelemetryBus::default();
        assert!(format!("{bus:?}").contains("SeqlockTelemetryBus"));
        assert!(bus.in_process_layout().is_some());
        assert_eq!(FabricHeader::default().version.load(Ordering::Relaxed), 1);
        assert_eq!(SeqlockSlot::default().seqlock.load(Ordering::Relaxed), 0);
        assert_eq!(FabricMemoryLayout::default().slots.len(), ICX_RING_SLOTS);
        bus.set_global_dag_root([0xCD; 32]);
        assert_eq!(bus.global_dag_root(), [0xCD; 32]);

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
        assert_eq!(bus.poll_since(200).len(), 0);
        let events = bus.poll_workspace_since(0, 0xCAFE);
        assert_eq!(events.len(), 200);
        for ev in events {
            assert!(ev.ahtl_packet.starts_with("⟦Θ:worker_"));
            assert!(ev.ahtl_packet.ends_with('⟧'));
            assert_eq!(ev.packet_kind, b'T');
        }
    }

    #[test]
    fn seqlock_8_writers_8_readers_10000_messages_stress_test_and_shared_file_attach() {
        let bus = SeqlockTelemetryBus::new_in_memory();
        let writers_done = Arc::new(AtomicBool::new(false));
        let mut writer_handles = Vec::with_capacity(8);
        let mut reader_handles = Vec::with_capacity(8);

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

        // Verify file-backed shared memory attach, global_dag_root sync, and magic/version validation
        let shm_path =
            std::env::temp_dir().join(format!("icx_shm_test_{}.shm", std::process::id()));
        let _ = std::fs::remove_file(&shm_path);
        let bus_a = SeqlockTelemetryBus::open_shared_file(&shm_path).expect("open bus_a");
        assert!(bus_a.in_process_layout().is_none());
        let bus_b = SeqlockTelemetryBus::open_shared_file(&shm_path).expect("attach bus_b");
        let root: [u8; 32] = core::array::from_fn(|i| (i as u8).wrapping_mul(7).wrapping_add(3));
        bus_a.set_global_dag_root(root);
        assert_eq!(bus_b.global_dag_root(), root);
        bus_a.register_agent(AgentKind::AntigravityCli);
        assert_ne!(bus_b.active_agents_mask(), 0);

        let id = ArtifactId(0x1234_5678_9ABC_DEF0);
        bus_a.publish(
            AgentKind::AntigravityCli,
            id,
            0x7777,
            250,
            [9; 4],
            [3; 16],
            "⟦Θ:agy@shm⊢✓file_sync⟧",
            999,
        );
        assert_eq!(bus_b.write_sequence(), 1);
        let from_b = bus_b.poll_workspace_since(0, 0x7777);
        assert_eq!(from_b.len(), 1);
        assert_eq!(from_b[0].agent, AgentKind::AntigravityCli);
        assert_eq!(from_b[0].artifact_id, id);
        let _ = std::fs::remove_file(&shm_path);

        // Simulate a crashed writer leaving slot[0].seqlock stuck on odd (1) and verify
        // subsequent publishes skip the stalled slot cleanly without hanging or stealing ownership
        let recovery_bus = SeqlockTelemetryBus::new_in_memory();
        if let Some(layout) = recovery_bus.in_process_layout() {
            layout.slots[0].seqlock.store(1, Ordering::Release);
            layout.header.root_seqlock.store(1, Ordering::Release);
        }
        recovery_bus.set_global_dag_root([0x55; 32]);
        assert_eq!(recovery_bus.global_dag_root(), [0u8; 32]);
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

    #[test]
    fn packet_kind_classification_agent_decoding_and_validation_errors() {
        let bus = SeqlockTelemetryBus::new_in_memory();
        let kinds = [
            (AgentKind::ClaudeCode, "⟦Θ:claude⊢Δ:diff⟧", b'D'),
            (AgentKind::Codex, "⟦Θ:codex⊢Ω:fold⟧", b'O'),
            (AgentKind::AntigravityCli, "⟦Θ:agy⊢Φ:epoch⟧", b'F'),
            (AgentKind::Hermes, "⟦Θ:hermes⊢Ψ:ppr⟧", b'P'),
            (AgentKind::GeminiCli, "⟦Θ:gemini_no_turnstile⟧", b'T'),
            (AgentKind::OpenCode, "⟦Δ:direct_diff⟧", b'D'),
            (AgentKind::Cursor, "⟦Ω:direct_fold⟧", b'O'),
            (AgentKind::GenericMcp, "plain_text_packet", b'T'),
        ];
        for (idx, (agent, pkt, expected_kind)) in kinds.iter().enumerate() {
            bus.publish(
                *agent,
                ArtifactId((idx as u64) + 1),
                0xABCD,
                128,
                [0; 4],
                [0; 16],
                pkt,
                idx as u64,
            );
            let evs = bus.poll_workspace_since(idx as u64, 0xABCD);
            assert_eq!(evs.len(), 1);
            assert_eq!(evs[0].agent, *agent);
            assert_eq!(evs[0].packet_kind, *expected_kind);
        }
        assert_eq!(decode_agent_kind(99), AgentKind::GenericMcp);

        // Multi-byte UTF-8 boundary truncation at ICX_AHTL_INLINE_CAP
        let long_utf8 = "⟦".repeat(ICX_AHTL_INLINE_CAP);
        bus.publish(
            AgentKind::ClaudeCode,
            ArtifactId(999),
            0xBEEF,
            255,
            [1; 4],
            [1; 16],
            &long_utf8,
            42,
        );
        let truncated = bus.poll_workspace_since(8, 0xBEEF);
        assert_eq!(truncated.len(), 1);
        assert!(truncated[0].ahtl_packet.len() <= ICX_AHTL_INLINE_CAP);

        // Verify InvalidMagic, UnsupportedVersion, and InsecurePermissions on shared file open
        let bad_path = std::env::temp_dir().join(format!("icx_shm_bad_{}.shm", std::process::id()));
        let _ = std::fs::remove_file(&bad_path);
        {
            let _ok = SeqlockTelemetryBus::open_shared_file(&bad_path).unwrap();
        }
        // Corrupt magic header
        {
            let f = OpenOptions::new().write(true).open(&bad_path).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::FileExt;
                f.write_all_at(&0xDEAD_BEEF_u64.to_le_bytes(), 0).unwrap();
            }
        }
        assert!(matches!(
            SeqlockTelemetryBus::open_shared_file(&bad_path),
            Err(BusError::InvalidMagic)
        ));

        // Restore magic, corrupt version
        {
            let f = OpenOptions::new().write(true).open(&bad_path).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::FileExt;
                f.write_all_at(&ICX_SHM_MAGIC.to_le_bytes(), 0).unwrap();
                f.write_all_at(&99u32.to_le_bytes(), 8).unwrap();
            }
        }
        assert!(matches!(
            SeqlockTelemetryBus::open_shared_file(&bad_path),
            Err(BusError::UnsupportedVersion(99))
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bad_path, std::fs::Permissions::from_mode(0o666)).unwrap();
            assert!(matches!(
                SeqlockTelemetryBus::open_shared_file(&bad_path),
                Err(BusError::InsecurePermissions(_))
            ));
        }
        let _ = std::fs::remove_file(&bad_path);
    }
}
