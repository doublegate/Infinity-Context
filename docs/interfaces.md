# Public Interfaces & Trait Contracts Specification

**References:** [docs/architecture.md](architecture.md), [docs/data-model.md](data-model.md)  
**Target Crates:** `icx-core`, `icx-codec`, `icx-hdc`, `icx-store`, `icx-bus`, `icx-adapters`

---

## 1. Core Storage & DAG Interfaces (`crates/icx-core`)

```rust
pub trait ContentAddressedStore: Send + Sync {
    /// Ingests raw bytes (after secret redaction), chunks via FastCDC, compresses, and returns the ArtifactId.
    fn put_bytes(&mut self, raw: &[u8]) -> Result<(ArtifactId, Blake3Digest), IcxError>;

    /// Reconstructs the exact uncompressed byte stream for `id` and verifies its BLAKE3 digest.
    fn deref_bytes(&self, id: ArtifactId) -> Result<Vec<u8>, IcxError>;

    /// Reconstructs the exact 1-based inclusive line range `[start_line, end_line]`.
    fn deref_lines(&self, id: ArtifactId, start_line: usize, end_line: usize) -> Result<String, IcxError>;

    /// Searches cold storage for `pattern` and returns matching lines with line numbers.
    fn grep_cold(&self, id: Option<ArtifactId>, pattern: &str, max_matches: usize) -> Result<Vec<ColdMatch>, IcxError>;
}
```

---

## 2. Codec & Skeletonizer Interfaces (`crates/icx-codec`)

```rust
pub trait ContextCodec: Send + Sync {
    /// Redacts secrets, skeletonizes `raw_text`, and compiles an AHTL Holographic Packet.
    fn compress_to_packet(
        &self,
        kind: PacketKind,
        subject: &str,
        raw_text: &str,
        deref_id: Option<ArtifactId>,
        hologram: [u8; 8],
    ) -> AhtlPacket;

    /// Renders an AhtlPacket into its canonical non-human BPE-aligned string representation.
    fn render_packet(&self, packet: &AhtlPacket) -> String;

    /// Parses a rendered AHTL packet back into its structured representation.
    fn parse_packet(&self, encoded: &str) -> Result<AhtlPacket, CodecError>;
}
```

---

## 3. Hyperdimensional & Fractal Index Interfaces (`crates/icx-hdc`)

```rust
pub trait AssociativeMemoryIndex: Send + Sync {
    /// Encodes text and metadata into a 10,240-bit Hypervector, updates the Fractal IFS trajectory,
    /// computes the Titans Surprise score, and indexes the artifact.
    fn observe_event(
        &mut self,
        artifact_id: ArtifactId,
        subject: &str,
        content: &str,
        is_anomaly: bool,
    ) -> ObservationReceipt;

    /// Queries the associative index using combined HDC Hamming similarity and Fractal suffix matching.
    fn query_associative(&self, query_text: &str, top_k: usize) -> Vec<AssociativeHit>;
}
```

---

## 4. Agent Lifecycle Hook Adapter Interface (`crates/icx-adapters`)

```rust
pub trait AgentHookAdapter: Send + Sync {
    /// Identifies the target agent harness.
    fn agent_kind(&self) -> AgentKind;

    /// Processes a raw JSON hook payload from `stdin` and produces the exact wire JSON response for `stdout`.
    fn handle_hook_event(
        &self,
        event_name: &str,
        stdin_json: &str,
        mmu: &mut InfinityContextMmu,
    ) -> Result<String, AdapterError>;
}
```

---

## 5. CLI Command Surface (`crates/icx-cli`)

```text
icx <SUBCOMMAND>

Subcommands:
  init            Initialize Infinity-Context CAS, Bi-Temporal Graph, and /dev/shm fabric
  hook            Execute a synchronous <500µs lifecycle hook for an agent (claude-code, codex, agy, hermes)
  mcp-serve       Run the Single-Tool Polymorphic MCP Pager over stdio
  proxy           Run the Cache-Epoch-Pinned HTTP/2 & SSE reverse proxy (127.0.0.1:49380)
  encode-ahtl     Compress a file, log, or diff into an AHTL Holographic Packet + CAS handle
  decode-ahtl     Parse an AHTL packet and optionally dereference its #icx:<hash> handle
  deref           Losslessly reconstruct full bytes or line slices for an #icx:<hash> handle
  status          Display Merkle-DAG stats, Cache-Epoch metrics, HDC index size, and /dev/shm ring health
  doctor          Verify agent hook registrations, CAS integrity, and prefix-cache alignment
  benchmark       Run the built-in compression ratio, HDC latency, and lossless round-trip benchmark suite
```
