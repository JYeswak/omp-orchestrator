//! Where the daemon is, and how the bearer token is found without ever being
//! written into this repository.
//!
//! Measured 2026-09-02: the running daemon is
//! `am serve-http --host 127.0.0.1 --port 8765 --no-tui` — note the absence of
//! `--no-auth`. Consequently every route except `/health` and `/healthz`
//! answers 401 to an unauthenticated caller, and the 401 is returned BEFORE
//! routing, so the MCP path cannot even be discovered without a credential.
//! The live MCP endpoint is `http://127.0.0.1:8765/mcp/` (trailing slash).
//!
//! The token is a secret. It is discovered at runtime from the environment or
//! from the MCP client configuration this machine already keeps, and it is
//! never a literal in this crate, never logged, and redacted in `Debug`.

use std::fmt;
use std::path::PathBuf;

/// The default MCP endpoint, matching the measured live daemon.
pub const DEFAULT_MCP_URL: &str = "http://127.0.0.1:8765/mcp/";

/// The unauthenticated liveness route, which answers without a token.
pub const HEALTH_PATH: &str = "/health";

/// Environment variable the Agent Mail server itself uses for its bearer
/// token, per strings extracted from the shipped `am` binary.
pub const ENV_PRIMARY_TOKEN: &str = "HTTP_BEARER_TOKEN";

/// Alternate, more explicit variable name accepted for callers that do not
/// want to export a generic one.
pub const ENV_ALT_TOKEN: &str = "AGENT_MAIL_BEARER_TOKEN";

/// Environment variable naming the MCP endpoint.
pub const ENV_URL: &str = "AM_MCP_URL";

/// Where a discovered token came from. Names the source, never the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    /// Read from the named environment variable.
    Environment(String),
    /// Parsed out of an MCP client configuration file.
    ClientConfig(PathBuf),
    /// Supplied directly by the caller.
    Explicit,
}

impl fmt::Display for TokenSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Environment(name) => write!(formatter, "env:{name}"),
            Self::ClientConfig(path) => write!(formatter, "client-config:{}", path.display()),
            Self::Explicit => formatter.write_str("explicit"),
        }
    }
}

/// A resolved daemon endpoint plus the credential to reach it.
#[derive(Clone)]
pub struct Endpoint {
    url: String,
    token: Option<String>,
    source: Option<TokenSource>,
}

/// `Debug` deliberately redacts the token.
///
/// A binding that prints its own bearer token into a test log or an error
/// message has leaked it into the terminal scrollback, the CI artifact, and
/// every agent transcript that quotes the failure.
impl fmt::Debug for Endpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Endpoint")
            .field("url", &self.url)
            .field(
                "token",
                &match &self.token {
                    Some(_) => "<redacted>",
                    None => "<none>",
                },
            )
            .field("source", &self.source)
            .finish()
    }
}

impl Endpoint {
    /// Build an endpoint from an explicit URL and token.
    #[must_use]
    pub fn new(url: impl Into<String>, token: Option<String>) -> Self {
        let source = token.as_ref().map(|_| TokenSource::Explicit);
        Self {
            url: url.into(),
            token,
            source,
        }
    }

    /// The MCP URL.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The bearer token, if one was discovered.
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Where the token came from, for evidence without disclosure.
    #[must_use]
    pub fn token_source(&self) -> Option<&TokenSource> {
        self.source.as_ref()
    }

    /// The unauthenticated health URL derived from the MCP URL's origin.
    ///
    /// `/health` is the one route that answers without a credential, which
    /// makes it the only honest liveness probe: a 401 from `/mcp/` proves the
    /// daemon is UP, but a caller with no token cannot tell 401-because-no-token
    /// from 401-because-wrong-token without it.
    #[must_use]
    pub fn health_url(&self) -> String {
        match origin_of(&self.url) {
            Some(origin) => format!("{origin}{HEALTH_PATH}"),
            None => format!("{}{HEALTH_PATH}", self.url.trim_end_matches('/')),
        }
    }

    /// Discover the endpoint from the environment, then from this machine's
    /// MCP client configuration.
    ///
    /// Order is deliberate: an explicitly exported variable must win over a
    /// file, so an operator can point the binding at a different daemon
    /// without editing anyone's editor config.
    ///
    /// Returns an endpoint with `token: None` when nothing was found; the
    /// caller turns that into
    /// [`MissingCredential`](crate::MailError::MissingCredential) rather than
    /// sending an empty `Authorization` header, because an empty credential
    /// produces a 401 that looks like a WRONG token instead of a missing one.
    #[must_use]
    pub fn discover() -> Self {
        let url = std::env::var(ENV_URL)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MCP_URL.to_owned());

        for name in [ENV_PRIMARY_TOKEN, ENV_ALT_TOKEN] {
            if let Ok(value) = std::env::var(name) {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Self {
                        url,
                        token: Some(trimmed.to_owned()),
                        source: Some(TokenSource::Environment(name.to_owned())),
                    };
                }
            }
        }

        for path in client_config_candidates() {
            if let Some(token) = token_from_client_config(&path) {
                return Self {
                    url,
                    token: Some(token),
                    source: Some(TokenSource::ClientConfig(path)),
                };
            }
        }

        Self {
            url,
            token: None,
            source: None,
        }
    }

    /// The ordered list of places [`Endpoint::discover`] looks, for a
    /// `MissingCredential` report that tells the operator where to put it.
    #[must_use]
    pub fn discovery_sources() -> Vec<String> {
        let mut sources = vec![
            format!("env:{ENV_PRIMARY_TOKEN}"),
            format!("env:{ENV_ALT_TOKEN}"),
        ];
        sources.extend(
            client_config_candidates()
                .into_iter()
                .map(|path| format!("client-config:{}", path.display())),
        );
        sources
    }
}

/// Return the `scheme://host:port` prefix of a URL.
fn origin_of(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split('/').next().unwrap_or(rest);
    if authority.is_empty() {
        return None;
    }
    Some(format!("{scheme}://{authority}"))
}

/// MCP client configuration files this machine is known to keep a token in.
fn client_config_candidates() -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    vec![
        home.join(".claude").join("settings.json"),
        home.join(".codex").join("config.toml"),
    ]
}

/// `$HOME` as a path, without pulling in a dependency for it.
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Extract a bearer token from a client config file.
///
/// Two shapes are recognised, both measured on this machine:
/// - JSON: `"Authorization": "Bearer <token>"`
/// - TOML: `HTTP_BEARER_TOKEN = "<token>"`
///
/// A deliberately narrow scan rather than a full JSON/TOML parse: the goal is
/// to find one credential in a file this crate does not own, and a narrow
/// matcher cannot be confused by unrelated structure elsewhere in a large
/// editor settings file.
fn token_from_client_config(path: &PathBuf) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        if let Some(token) = bearer_after(line, "Bearer ") {
            return Some(token);
        }
        if let Some(token) = assigned_string(line, ENV_PRIMARY_TOKEN) {
            return Some(token);
        }
    }
    None
}

/// Pull the quoted token that follows a `Bearer ` marker on one line.
fn bearer_after(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    let token = rest[..end].trim();
    (!token.is_empty()).then(|| token.to_owned())
}

/// Pull the quoted value of `KEY = "value"` on one line.
fn assigned_string(line: &str, key: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let rest = trimmed.strip_prefix(key)?;
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    let token = rest[..end].trim();
    (!token.is_empty()).then(|| token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_token() {
        let endpoint = Endpoint::new(DEFAULT_MCP_URL, Some("s3cr3t-value-abc123".to_owned()));
        let rendered = format!("{endpoint:?}");
        assert!(
            !rendered.contains("s3cr3t-value-abc123"),
            "Debug leaked the bearer token: {rendered}"
        );
        assert!(rendered.contains("<redacted>"), "got {rendered}");
    }

    #[test]
    fn debug_distinguishes_absent_from_redacted() {
        let rendered = format!("{:?}", Endpoint::new(DEFAULT_MCP_URL, None));
        assert!(rendered.contains("<none>"), "got {rendered}");
    }

    #[test]
    fn health_url_is_derived_from_the_origin_not_appended_to_the_path() {
        // The trap: naive concatenation yields
        // `http://127.0.0.1:8765/mcp/health`, which is NOT the health route
        // and answers 401 like every other unknown path.
        let endpoint = Endpoint::new(DEFAULT_MCP_URL, None);
        assert_eq!(endpoint.health_url(), "http://127.0.0.1:8765/health");
    }

    #[test]
    fn bearer_token_is_parsed_from_a_json_authorization_line() {
        let line = r#"        "Authorization": "Bearer d93d128e9c7c325a" "#;
        assert_eq!(
            bearer_after(line, "Bearer "),
            Some("d93d128e9c7c325a".to_owned())
        );
    }

    #[test]
    fn bearer_token_is_parsed_from_a_toml_assignment() {
        let line = r#"HTTP_BEARER_TOKEN = "abc123def" "#;
        assert_eq!(
            assigned_string(line, ENV_PRIMARY_TOKEN),
            Some("abc123def".to_owned())
        );
    }

    #[test]
    fn unrelated_lines_yield_no_token() {
        assert_eq!(bearer_after("no credential here", "Bearer "), None);
        assert_eq!(assigned_string("OTHER_KEY = \"x\"", ENV_PRIMARY_TOKEN), None);
        // An empty quoted value must not be accepted as a token: it would
        // produce an `Authorization: Bearer ` header and a misleading 401.
        assert_eq!(assigned_string(r#"HTTP_BEARER_TOKEN = """#, ENV_PRIMARY_TOKEN), None);
    }

    #[test]
    fn discovery_sources_are_named_and_ordered() {
        let sources = Endpoint::discovery_sources();
        assert!(
            sources.len() >= 2,
            "discovery must name where to put a token: {sources:?}"
        );
        assert_eq!(sources[0], format!("env:{ENV_PRIMARY_TOKEN}"));
        assert_eq!(sources[1], format!("env:{ENV_ALT_TOKEN}"));
    }

    #[test]
    fn origin_parsing_handles_paths_and_rejects_garbage() {
        assert_eq!(
            origin_of("http://127.0.0.1:8765/mcp/"),
            Some("http://127.0.0.1:8765".to_owned())
        );
        assert_eq!(origin_of("not-a-url"), None);
    }
}
