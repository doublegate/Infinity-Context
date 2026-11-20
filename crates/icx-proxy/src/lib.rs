//! `icx-proxy` — Wire-level Cache-Epoch request transformer for Anthropic, OpenAI, and Gemini APIs.

pub mod wire;

pub use wire::{ProviderFormat, WireProxyTransformer, WorkspaceGate};
