//! provreq MCP server — the Model Context Protocol adapter for
//! `provreq server` (arc-2 slice 8, #376).
//!
//! The `provreq-mcp` binary talks JSON-RPC 2.0 on stdio to any
//! MCP-aware AI coding agent (Claude Code, Cursor, Zed, GitHub
//! Copilot) and converts each request into one or more REST
//! calls against a running `provreq server`. The library side
//! exposes the transport + tool-dispatch surface so integration
//! tests can drive the loop without a child process. The
//! management surface is read-only; the proof surface adds two
//! operator write actions (verify, triage).
//!
//! Implements: REQ081

pub mod client;
pub mod error;
pub mod prompts;
pub mod protocol;
pub mod resources;
pub mod tools;
pub mod transport;

pub use client::ProvreqClient;
