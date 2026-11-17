//! `icx-bus` — Lock-free POSIX `/dev/shm` Seqlock Ring Buffer (`SeqlockTelemetryBus`) for
//! sub-100µs cross-agent telepathy.

pub mod ring;

pub use ring::{
    BusError, FabricHeader, FabricMemoryLayout, ICX_RING_SLOTS, ICX_SHM_MAGIC, SeqlockSlot,
    SeqlockTelemetryBus, TelepathyEvent,
};
