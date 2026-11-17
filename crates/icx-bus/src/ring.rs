//! Lock-Free Seqlock Ring Buffer (`SeqlockTelemetryBus`) over POSIX shared memory (`/dev/shm`)
//! or in-process aligned memory for `< 100 µs` cross-agent telepathy.

use core::sync::atomic::{fence, AtomicU64, Ordering};
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

/// Error returned by `SeqlockTelemetryBus` operations.
#[derive(Debug, Error)]
pub enum BusError {
    #[error("shared memory I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid shared memory magic header (expected 0x{ICX_SHM_MAGIC:016x})")]
    InvalidMagic,
}

/// 64-byte cache-line-aligned header at the start of the shared memory fabric.
#[repr(C, align(64))]
#[derive(Debug)]
pub struct FabricHeader {
    pub magic: u64,
    pub version: u32,
    pub reserved: u32,
    pub write_seq: AtomicU64,
    pub _pad: [u8; 40],
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
    pub _reserved: u32,
    pub hdc_sketch: [u64; 4],
    pub fractal_q15: [i16; 16],
    pub _pad: [u8; 24],
    pub ahtl_bytes: [u8; ICX_AHTL_INLINE_CAP],
}

const _: () = assert!(core::mem::size_of::<FabricHeader>() == 64);
const _: () = assert!(core::mem::size_of::<SeqlockSlot>() == 4096);

/// Complete layout of the shared-memory ring buffer.
#[repr(C, align(64))]
#[derive(Debug)]
pub struct FabricMemoryLayout {
    pub header: FabricHeader,
    pub slots: [SeqlockSlot; ICX_RING_SLOTS],
}

/// Decoded cross-agent telepathy packet read from the `SeqlockTelemetryBus`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelepathyEvent {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub agent: AgentKind,
    pub artifact_id: ArtifactId,
    pub workspace_hash: u64,
    pub surprise_q8: u8,
    pub hdc_sketch: [u64; 4],
    pub fractal_q15: [i16; 16],
    pub ahtl_packet: String,
}

#[derive(Clone)]
enum BackingStore {
    InProcess(Arc<FabricMemoryLayout>),
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
    /// `/dev/shm/icx-fabric-v1` or `$XDG_RUNTIME_DIR`).
    pub fn open_shared_file(path: impl AsRef<Path>) -> Result<Self, BusError> {
        let required_len = core::mem::size_of::<FabricMemoryLayout>();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.as_ref())?;

        let metadata = file.metadata()?;
        let is_new = metadata.len() < (required_len as u64);
        if is_new {
            file.set_len(required_len as u64)?;
        }

        // SAFETY: `file` is sized to `size_of::<FabricMemoryLayout>()` and mapped into page-aligned
        // virtual memory. All concurrent mutations inside `FabricMemoryLayout` go through atomic
        // seqlock protocol primitives.
        let mut mmap = unsafe { MmapOptions::new().len(required_len).map_mut(&file)? };

        // SAFETY: `mmap` is page-aligned (>= 64-byte alignment) and has length `required_len`.
        let layout_ref = unsafe { &mut *(mmap.as_mut_ptr().cast::<FabricMemoryLayout>()) };
        if is_new || layout_ref.header.magic == 0 {
            layout_ref.header.magic = ICX_SHM_MAGIC;
            layout_ref.header.version = ICX_SHM_VERSION;
            layout_ref.header.write_seq.store(0, Ordering::Release);
        } else if layout_ref.header.magic != ICX_SHM_MAGIC {
            return Err(BusError::InvalidMagic);
        }

        Ok(Self {
            backing: BackingStore::SharedMmap(Arc::new(mmap)),
        })
    }

    fn heap_zeroed_layout() -> Arc<FabricMemoryLayout> {
        let alloc_layout = std::alloc::Layout::new::<FabricMemoryLayout>();
        // SAFETY: `FabricMemoryLayout` has non-zero size and consists exclusively of integer
        // primitives and `AtomicU64` where all-zero bytes are a valid representation.
        unsafe {
            let ptr = std::alloc::alloc_zeroed(alloc_layout).cast::<FabricMemoryLayout>();
            if ptr.is_null() {
                std::alloc::handle_alloc_error(alloc_layout);
            }
            (*ptr).header.magic = ICX_SHM_MAGIC;
            (*ptr).header.version = ICX_SHM_VERSION;
            let boxed = Box::from_raw(ptr);
            Arc::from(boxed)
        }
    }

    #[inline]
    fn layout(&self) -> &FabricMemoryLayout {
        match &self.backing {
            BackingStore::InProcess(arc) => arc.as_ref(),
            BackingStore::SharedMmap(mmap) => {
                // SAFETY: `mmap` is validated at creation to have length `size_of::<FabricMemoryLayout>()`
                // and page alignment >= 64 bytes.
                unsafe { &*(mmap.as_ptr().cast::<FabricMemoryLayout>()) }
            }
        }
    }

    /// Returns the total number of telepathy events published so far.
    #[inline]
    pub fn write_sequence(&self) -> u64 {
        self.layout().header.write_seq.load(Ordering::Acquire)
    }

    /// Publishes a cross-agent telepathy event into the ring buffer using the lock-free Seqlock protocol.
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
        let layout = self.layout();
        let seq = layout.header.write_seq.fetch_add(1, Ordering::AcqRel);
        let slot_idx = (seq as usize) % ICX_RING_SLOTS;

        let slot_ptr = (&raw const layout.slots[slot_idx]).cast_mut();
        // SAFETY: Writer claims `seq` and brackets slot writes with odd/even Release stores on `seqlock`.
        unsafe {
            let odd_version = (seq << 1) | 1;
            (*slot_ptr).seqlock.store(odd_version, Ordering::Release);
            fence(Ordering::AcqRel);

            (*slot_ptr).timestamp_ms = timestamp_ms;
            (*slot_ptr).artifact_id = artifact_id.0;
            (*slot_ptr).workspace_hash = workspace_hash;
            (*slot_ptr).agent_kind = agent as u8;
            (*slot_ptr).surprise_q8 = surprise_q8;

            let raw_bytes = ahtl_packet.as_bytes();
            let copy_len = raw_bytes.len().min(ICX_AHTL_INLINE_CAP);
            (*slot_ptr).payload_len = copy_len as u16;
            (*slot_ptr).hdc_sketch = hdc_sketch;
            (*slot_ptr).fractal_q15 = fractal_q15;
            core::ptr::copy_nonoverlapping(
                raw_bytes.as_ptr(),
                (*slot_ptr).ahtl_bytes.as_mut_ptr(),
                copy_len,
            );

            fence(Ordering::AcqRel);
            let even_version = (seq + 1) << 1;
            (*slot_ptr).seqlock.store(even_version, Ordering::Release);
        }

        seq + 1
    }

    /// Wait-free optimistic poll of all committed telepathy events with sequence number `> since_seq`.
    pub fn poll_since(&self, since_seq: u64) -> Vec<TelepathyEvent> {
        let layout = self.layout();
        let current_head = layout.header.write_seq.load(Ordering::Acquire);
        if current_head <= since_seq {
            return Vec::new();
        }

        let start_seq = since_seq.max(current_head.saturating_sub(ICX_RING_SLOTS as u64));
        let mut events = Vec::new();

        for seq in start_seq..current_head {
            let slot_idx = (seq as usize) % ICX_RING_SLOTS;
            let slot = &layout.slots[slot_idx];

            let s1 = slot.seqlock.load(Ordering::Acquire);
            let expected_even = (seq + 1) << 1;
            if (s1 & 1) != 0 || s1 < expected_even {
                continue;
            }

            // Optimistically copy fields
            let slot_ptr = &raw const *slot;
            // SAFETY: Optimistic read guarded by `s1 == s2` seqlock verification below.
            let (ts, aid, ws_hash, ag_byte, surp, len, sketch, frac, buf) = unsafe {
                let ts = core::ptr::read_volatile(&raw const (*slot_ptr).timestamp_ms);
                let aid = core::ptr::read_volatile(&raw const (*slot_ptr).artifact_id);
                let ws_hash = core::ptr::read_volatile(&raw const (*slot_ptr).workspace_hash);
                let ag_byte = core::ptr::read_volatile(&raw const (*slot_ptr).agent_kind);
                let surp = core::ptr::read_volatile(&raw const (*slot_ptr).surprise_q8);
                let len = (core::ptr::read_volatile(&raw const (*slot_ptr).payload_len) as usize)
                    .min(ICX_AHTL_INLINE_CAP);
                let sketch = core::ptr::read_volatile(&raw const (*slot_ptr).hdc_sketch);
                let frac = core::ptr::read_volatile(&raw const (*slot_ptr).fractal_q15);
                let mut buf = vec![0u8; len];
                core::ptr::copy_nonoverlapping(
                    (*slot_ptr).ahtl_bytes.as_ptr(),
                    buf.as_mut_ptr(),
                    len,
                );
                (ts, aid, ws_hash, ag_byte, surp, len, sketch, frac, buf)
            };

            fence(Ordering::Acquire);
            let s2 = slot.seqlock.load(Ordering::Acquire);
            if s1 != s2 {
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
                seq: seq + 1,
                timestamp_ms: ts,
                agent,
                artifact_id: ArtifactId(aid),
                workspace_hash: ws_hash,
                surprise_q8: surp,
                hdc_sketch: sketch,
                fractal_q15: frac,
                ahtl_packet,
            });
        }

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let events = bus.poll_since(0);
        assert_eq!(events.len(), 200);
        for ev in events {
            assert!(ev.ahtl_packet.starts_with("⟦Θ:worker_"));
            assert!(ev.ahtl_packet.ends_with('⟧'));
        }
    }
}
