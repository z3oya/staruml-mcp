mod staruml;
mod tools;

use std::process::ExitCode;

use rmcp::transport::stdio;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

pub(crate) const SERVER_NAME: &str = "staruml-mcp-server";
pub(crate) const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_API_PORT: u16 = 58321;

#[tokio::main]
async fn main() -> ExitCode {
    init_logging();
    match parse_args(std::env::args().skip(1)) {
        Err(err) => {
            eprintln!("[{SERVER_NAME}] {err}");
            ExitCode::FAILURE
        }
        Ok(None) => ExitCode::SUCCESS,
        Ok(Some(api_port)) => run(api_port).await,
    }
}

/// rmcp reports protocol diagnostics via `tracing`; the subscriber writes to
/// stderr because stdout carries the protocol. `RUST_LOG` overrides the level.
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if let Err(err) = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .try_init()
    {
        eprintln!("[{SERVER_NAME}] failed to initialise logging: {err}");
    }
}

async fn run(api_port: u16) -> ExitCode {
    tracing::info!(api_port, "listening on stdio; StarUML API at http://localhost:{api_port}");
    let service = tools::StarUmlTools::new(staruml::Client::new(api_port));
    let server = match service.serve(stdio()).await {
        Ok(server) => server,
        Err(err) => {
            tracing::error!("failed to start: {err}");
            return ExitCode::FAILURE;
        }
    };
    match server.waiting().await {
        Ok(reason) => {
            tracing::debug!("server quit: {reason:?}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            tracing::error!("server exited with error: {err}");
            ExitCode::FAILURE
        }
    }
}

/// `Ok(Some(port))` runs the server; `Ok(None)` means `--help`/`--version`
/// already printed and the process should exit successfully.
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Option<u16>, String> {
    let mut api_port = None;
    for arg in args {
        if let Some(raw) = arg.strip_prefix("--api-port=") {
            match raw.parse::<u16>() {
                Ok(0) => return Err(format!("invalid port number: {raw} (0 is reserved)")),
                Ok(port) => api_port = Some(port),
                Err(_) => return Err(format!("invalid port number: {raw}")),
            }
        } else {
            match arg.as_str() {
                "--help" | "-h" => {
                    println!("{SERVER_NAME} v{SERVER_VERSION}");
                    println!("Usage: staruml-mcp [--api-port=<port>]  (default: {DEFAULT_API_PORT})");
                    return Ok(None);
                }
                "--version" | "-V" => {
                    println!("{SERVER_NAME} v{SERVER_VERSION}");
                    return Ok(None);
                }
                other => return Err(format!("unknown argument: {other}")),
            }
        }
    }
    Ok(Some(api_port.unwrap_or(DEFAULT_API_PORT)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn defaults_to_builtin_api_port() {
        assert_eq!(parse_args(args(&[])), Ok(Some(DEFAULT_API_PORT)));
    }

    #[test]
    fn parses_api_port_override() {
        assert_eq!(parse_args(args(&["--api-port=58999"])), Ok(Some(58999)));
    }

    #[test]
    fn rejects_reserved_and_unparsable_ports() {
        assert!(parse_args(args(&["--api-port=0"])).is_err());
        assert!(parse_args(args(&["--api-port=x"])).is_err());
    }

    #[test]
    fn unknown_argument_is_an_error() {
        assert!(parse_args(args(&["--wat"])).is_err());
    }
}
