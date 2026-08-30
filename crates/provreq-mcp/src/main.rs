//! provreq MCP binary — stdio entry point.
//!
//! Arguments (hand-parsed to avoid a clap dep):
//!
//! - `--url <URL>` (default `http://127.0.0.1:17869`) — the
//!   `provreq server` instance to proxy into.
//! - `--allow-remote` — opt in to non-loopback URLs. Without
//!   this flag, the binary refuses URLs whose host isn't
//!   `localhost` / `127.0.0.0/8` / `::1`. Matches the
//!   LLM-privacyWarning principle applied to the MCP direction
//!   of travel: operators shouldn't accidentally expose their
//!   requirements to a remote server they forgot they were
//!   pointing at.
//! - `--help` / `-h` — usage summary.
//! - `--version` / `-V` — version + protocol version.
//!
//! Logs go to stderr so stdout stays reserved for JSON-RPC
//! traffic.

use std::process::ExitCode;

use provreq_mcp::{ProvreqClient, protocol, transport};
use url::Url;

const DEFAULT_URL: &str = "http://127.0.0.1:17869";

fn main() -> ExitCode {
    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("provreq-mcp: {e}");
            ExitCode::FAILURE
        }
    }
}

fn real_main() -> Result<(), String> {
    init_tracing();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = parse_args(&args)?;
    match parsed {
        ParsedArgs::Help => {
            print_help();
            Ok(())
        }
        ParsedArgs::Version => {
            println!(
                "provreq-mcp {} (MCP protocol {})",
                protocol::SERVER_VERSION,
                protocol::PROTOCOL_VERSION
            );
            Ok(())
        }
        ParsedArgs::Run { url, allow_remote } => run(url, allow_remote),
    }
}

fn init_tracing() {
    // Logs on stderr, stdout reserved for JSON-RPC. Silent by
    // default; set RUST_LOG=provreq_mcp=info to see anything.
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .try_init();
}

fn run(url: Url, allow_remote: bool) -> Result<(), String> {
    if !allow_remote && !is_loopback(&url) {
        return Err(format!(
            "refusing to connect to non-loopback URL {url} (pass --allow-remote to override)"
        ));
    }
    let client = ProvreqClient::new(url);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("build tokio runtime: {e}"))?;
    runtime
        .block_on(transport::run(client))
        .map_err(|e| format!("transport loop: {e}"))
}

#[derive(Debug)]
enum ParsedArgs {
    Help,
    Version,
    Run { url: Url, allow_remote: bool },
}

fn parse_args(args: &[String]) -> Result<ParsedArgs, String> {
    let mut url: Option<Url> = None;
    let mut allow_remote = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--help" | "-h" => return Ok(ParsedArgs::Help),
            "--version" | "-V" => return Ok(ParsedArgs::Version),
            "--allow-remote" => {
                allow_remote = true;
                i += 1;
            }
            "--url" => {
                let v = args
                    .get(i + 1)
                    .ok_or_else(|| "--url requires a value".to_owned())?;
                url = Some(Url::parse(v).map_err(|e| format!("invalid --url '{v}': {e}"))?);
                i += 2;
            }
            other if other.starts_with("--url=") => {
                let v = &other["--url=".len()..];
                url = Some(Url::parse(v).map_err(|e| format!("invalid --url '{v}': {e}"))?);
                i += 1;
            }
            other => {
                return Err(format!("unknown argument '{other}' (see --help)"));
            }
        }
    }
    let url = url.unwrap_or_else(|| Url::parse(DEFAULT_URL).expect("default URL parses"));
    Ok(ParsedArgs::Run { url, allow_remote })
}

fn is_loopback(url: &Url) -> bool {
    let Some(host) = url.host() else {
        return false;
    };
    match host {
        url::Host::Domain(d) => d.eq_ignore_ascii_case("localhost"),
        url::Host::Ipv4(v4) => v4.is_loopback(),
        url::Host::Ipv6(v6) => v6.is_loopback(),
    }
}

fn print_help() {
    let help = "\
provreq-mcp — read-only Model Context Protocol server for provreq

USAGE:
    provreq-mcp [OPTIONS]

Speaks MCP (JSON-RPC 2.0 over stdio) and proxies each request
into a running provreq server. Run via an MCP-aware coding
agent (Claude Code, Cursor, Zed, …); the agent spawns the
binary and writes to its stdin / reads from its stdout.

OPTIONS:
    --url <URL>        provreq server base URL (default: http://127.0.0.1:17869)
    --allow-remote     Permit non-loopback --url values. Without this flag the
                       binary refuses to connect to anything that isn't
                       localhost / 127/8 / [::1]. Off by default.
    -h, --help         Show this help.
    -V, --version      Show the binary + MCP protocol version.
";
    print!("{help}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_args_defaults_to_loopback_url() {
        let parsed = parse_args(&[]).unwrap();
        match parsed {
            ParsedArgs::Run { url, allow_remote } => {
                assert_eq!(url.as_str(), "http://127.0.0.1:17869/");
                assert!(!allow_remote);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_args_accepts_long_url_form_and_allow_remote() {
        let parsed = parse_args(&args(&[
            "--url",
            "https://example.com:9000",
            "--allow-remote",
        ]))
        .unwrap();
        match parsed {
            ParsedArgs::Run { url, allow_remote } => {
                assert_eq!(url.as_str(), "https://example.com:9000/");
                assert!(allow_remote);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_args_accepts_equals_url_form() {
        let parsed = parse_args(&args(&["--url=http://localhost:9999"])).unwrap();
        match parsed {
            ParsedArgs::Run { url, .. } => {
                assert_eq!(url.as_str(), "http://localhost:9999/");
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parse_args_rejects_unknown_flags() {
        let err = parse_args(&args(&["--nope"])).unwrap_err();
        assert!(err.contains("--nope"));
    }

    #[test]
    fn parse_args_help_and_version_short_circuit() {
        assert!(matches!(
            parse_args(&args(&["--help"])),
            Ok(ParsedArgs::Help)
        ));
        assert!(matches!(
            parse_args(&args(&["-V"])),
            Ok(ParsedArgs::Version)
        ));
    }

    #[test]
    fn loopback_check_accepts_localhost_and_127_and_ipv6() {
        assert!(is_loopback(&Url::parse("http://localhost:1234").unwrap()));
        assert!(is_loopback(&Url::parse("http://127.0.0.1:1234").unwrap()));
        assert!(is_loopback(&Url::parse("http://127.5.6.7").unwrap()));
        assert!(is_loopback(&Url::parse("http://[::1]:8080").unwrap()));
    }

    #[test]
    fn loopback_check_rejects_public_internet() {
        assert!(!is_loopback(
            &Url::parse("https://api.example.com").unwrap()
        ));
        assert!(!is_loopback(&Url::parse("http://8.8.8.8").unwrap()));
        // RFC 1918 private ranges are NOT loopback — they're
        // still "remote" for the purpose of the ack gate.
        assert!(!is_loopback(&Url::parse("http://192.168.1.1").unwrap()));
    }
}
