# Public Interfaces & Trait Contracts Specification

**References:** [docs/architecture.md](architecture.md), [docs/data-model.md](data-model.md)  
**Target Crates:** `icx-core`, `icx-codec`, `icx-hdc`, `icx-store`, `icx-bus`, `icx-adapters`, `icx-cli`  
**Applies to:** `icx` v1.1.0

---

## 1. Core Storage & DAG Interfaces (`crates/icx-core`)

```rust
pub trait ContentAddressedStore: Send + Sync {
    /// Ingests raw bytes (after secret redaction), chunks via FastCDC, and returns `(ArtifactId, Blake3Digest)`.
    /// Fails with `IcxError::HashCollision` if a different payload holds the same 64-bit handle
    /// (nothing is overwritten), and rejects payloads above 64 MiB (`ICX_MAX_INGEST_BYTES`).
    fn put_bytes_checked(&mut self, raw: &[u8]) -> Result<(ArtifactId, Blake3Digest), IcxError>;

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
    /// computes the Titans-inspired surprise score, and indexes the artifact.
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

## 5. Telepathy Bus Interface (`crates/icx-bus`)

```rust
impl SeqlockTelemetryBus {
    pub fn new_in_memory() -> Self;
    /// O_NOFOLLOW | O_CLOEXEC, mode 0600; refuses non-regular files, foreign owners, group/world modes.
    pub fn open_shared_file(path: impl AsRef<Path>) -> Result<Self, BusError>;
    /// Err(SlotLapped | SlotContended | Io) when the event was not committed.
    pub fn try_publish(&self, agent: AgentKind, artifact_id: ArtifactId, workspace_hash: u64,
                       surprise_q8: u8, hdc_sketch: [u64; 4], fractal_q15: [i16; 16],
                       ahtl_packet: &str, timestamp_ms: u64) -> Result<u64, BusError>;
    /// Infallible wrapper around `try_publish`; counts drops.
    pub fn publish(/* same arguments */) -> u64;
    pub fn dropped_publishes(&self) -> u64;
    pub fn poll_since(&self, since_seq: u64) -> Vec<TelepathyEvent>;
    /// Events plus the number of sequence numbers after `since_seq` that were not delivered.
    pub fn poll_since_with_loss(&self, since_seq: u64) -> (Vec<TelepathyEvent>, u64);
    pub fn poll_workspace_since(&self, since_seq: u64, workspace_hash: u64) -> Vec<TelepathyEvent>;
}
```

See [docs/wire-protocol-and-shm-bus.md](wire-protocol-and-shm-bus.md) for semantics.

---

## 6. Workspace Resolution & Trust (`crates/icx-adapters/src/workspace.rs`)

```rust
pub struct WorkspaceRoots { pub state_dir: PathBuf, pub config_dir: PathBuf, pub runtime_dir: PathBuf }
impl WorkspaceRoots { pub fn from_env() -> Self; } // absolute paths only; empty root => fail closed

pub fn project_root(cwd: &Path) -> PathBuf;                          // nearest ancestor with `.git`
pub fn default_workspace_dir(cwd: &Path, roots: &WorkspaceRoots) -> PathBuf;
pub fn verify(dir: &Path, roots: &WorkspaceRoots) -> Result<(), TrustError>;  // owner, mode, OWNER seal
pub fn prepare(dir: &Path, roots: &WorkspaceRoots) -> Result<(), TrustError>; // create + seal
pub fn trust(dir: &Path, roots: &WorkspaceRoots) -> Result<(), TrustError>;   // adopt an existing dir
pub fn install_key(roots: &WorkspaceRoots) -> Result<[u8; 32], TrustError>;
pub fn derived_key(roots: &WorkspaceRoots, purpose: &str) -> Result<[u8; 32], TrustError>;
pub fn daemon_socket_path(dir: &Path, roots: &WorkspaceRoots) -> PathBuf;
```

---

## 7. CLI Command Surface (`crates/icx-cli`)

```text
icx <SUBCOMMAND> [OPTIONS]

  init [--dir <PATH>]                 Create and seal a workspace (default: per-project under
                                      $XDG_STATE_HOME/icx/workspaces)
  daemon [--dir <PATH>] [--idle-exit <SECS>]
                                      Keep the workspace resident; serve hooks over a private Unix socket
  trust --dir <PATH>                  Adopt an existing workspace you own (e.g. a v1.0.x `.icx/`)
  deref <HANDLE> [--lines <START:END>] [--dir <PATH>] [--raw]
                                      Dereference a #icx:<16-hex> or #icx:<12-hex> handle; framed as
                                      untrusted data unless --raw
  hook --agent <claude-code|codex|agy|hermes> --event <EVENT> [--dir <PATH>]
                                      Synchronous lifecycle hook (JSON on stdin, JSON on stdout)
  mcp-serve [--dir <PATH>]            Single-tool MCP pager over stdio (JSON-RPC 2.0)
  proxy [--listen <HOST:PORT>] [--provider <anthropic|openai|gemini>] [--upstream <URL>]
        [--dir <PATH>] [--max-requests <N>] [--allow-remote] [--insecure-no-auth]
                                      Token-authenticated HTTP/1.1 & SSE reverse proxy (default 127.0.0.1:49380)
  proxy-transform [--provider ...] [--dir <PATH>]
                                      Transform one outbound request body read from stdin
  encode-ahtl <FILE> [--dir <PATH>]   Store a file and print its AHTL packet + handle
  decode-ahtl <AHTL_STRING>           Parse and inspect an AHTL packet
  status [--dir <PATH>]               MMU status
  doctor [--dir <PATH>]               Inspect agent installations, the bus path and workspace integrity
  benchmark                           Synthetic in-process benchmark suite with SLO gates
```

Environment: `ICX_DIR`, `ICX_STATE_HOME`, `ICX_CONFIG_HOME`, `ICX_SHM_PATH`, `ICX_NO_DAEMON`,
`ICX_DAEMON_AUTOSTART`, `ICX_PROXY_BIND`, `ICX_UPSTREAM_URL`, plus `XDG_STATE_HOME`,
`XDG_CONFIG_HOME` and `XDG_RUNTIME_DIR`.
