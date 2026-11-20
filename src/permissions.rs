//! Atto runtime permission *requests* (not an authorization oracle).
//!
//! Values arrive only from flags2env's argv-only `provided_flags`.
//! This module cannot open files, resolve DNS, perform FFI, or mint a trusted
//! policy manifest. Host/OS, artifact, TenantVM, actor and child ceilings
//! must still be checked at the actual I/O broker (not implemented here).
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum Kind { Read, Write, Net, Env, Run, Sys, Ffi, Import }

impl Kind {
    pub const ALL: [Self; 8] = [
        Self::Read, Self::Write, Self::Net, Self::Env,
        Self::Run, Self::Sys, Self::Ffi, Self::Import,
    ];
    fn key(self) -> &'static str {
        match self {
            Self::Read => "READ", Self::Write => "WRITE",
            Self::Net => "NET", Self::Env => "ENV", Self::Run => "RUN",
            Self::Sys => "SYS", Self::Ffi => "FFI", Self::Import => "IMPORT"
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Scope {
    All,
    Some(BTreeSet<String>),
}

#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct PermissionRequest {
    /// A request inside mandatory non-escalating ceilings, NEVER an entitlement.
    pub allow_all: bool,
    pub allows: BTreeMap<Kind, Scope>,
    pub denies: BTreeMap<Kind, Scope>,
    /// Strict compiler-side checking does not disable dynamic enforcement.
    pub check_at_compile: bool,
}

fn invalid(reason: &'static str) -> String { format!("E_PERMISSION_FLAG: {reason}") }

fn boolean(raw: &str) -> Result<bool, String> {
    match raw {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(invalid("invalid boolean flag value")),
    }
}

impl PermissionRequest {
    /// No ambient environment, dotenv, or schema defaults may provide grants.
    pub fn from_argv_only(flags: &HashMap<String, String>) -> Result<Self, String> {
        let mut out = Self::default();
        if let Some(raw) = flags.get("ATTO_ALLOW_ALL") {
            out.allow_all = boolean(raw)?;
        }
        if let Some(raw) = flags.get("ATTO_PERMISSION_CHECK") {
            match raw.as_str() {
                "compile" => out.check_at_compile = true,
                "runtime" => {},
                _ => return Err(invalid("permission-check must be compile or runtime")),
            }
        }
        if let Some(raw) = flags.get("ATTO_NO_PROMPT") {
            if !boolean(raw)? {
                // No interactive prompt mode exists: explicitly denying
                // --no-prompt must not turn on a prompt-driven self-grant.
                return Err(invalid("interactive permission prompts unsupported"));
            }
        }
        for kind in Kind::ALL {
            for (prefix, map) in [
                ("ATTO_ALLOW_", &mut out.allows),
                ("ATTO_DENY_", &mut out.denies),
            ] {
                let key = format!("{prefix}{}", kind.key());
                if let Some(raw) = flags.get(&key) {
                    map.insert(kind, parse_scope(kind, raw)?);
                }
            }
        }
        Ok(out)
    }

    /// Caller must reject this when target host cannot broker privileged I/O.
    pub fn requests_privileged_io(&self) -> bool {
        self.allow_all || !self.allows.is_empty()
    }

    pub fn is_deny_all_request(&self) -> bool {
        !self.requests_privileged_io()
    }
}

/// Scope validation is intentionally **only syntax/canonical form**.
/// Do NOT use it to authorize actual paths or network destinations.
fn parse_scope(kind: Kind, raw: &str) -> Result<Scope, String> {
    if raw == "*" { return Ok(Scope::All); }
    if raw.is_empty() || raw.len() > 8192 { return Err(invalid("empty or oversized scope")); }
    let mut scopes = BTreeSet::new();
    for token in raw.split(',') {
        if token.is_empty() || token == "*" || token.trim() != token
            || token.chars().any(|c| c.is_control())
        {
            return Err(invalid("malformed comma-separated scope"));
        }
        let normalized = match kind {
            Kind::Net => canonical_net(token)?,
            Kind::Read | Kind::Write | Kind::Run | Kind::Ffi => canonical_path(token)?,
            Kind::Env => canonical_env(token)?,
            Kind::Sys | Kind::Import => canonical_symbol(token)?,
        };
        if !scopes.insert(normalized) {
            return Err(invalid("duplicate scope"));
        }
    }
    if scopes.is_empty() { return Err(invalid("empty scope")); }
    Ok(Scope::Some(scopes))
}

fn canonical_path(raw: &str) -> Result<String, String> {
    // Lexical preflight only: final path identity MUST be checked by a
    // descriptor-relative, tenant-root-locked host FS broker.
    if raw.len() > 4096 || raw.starts_with('~') || raw.contains('\\')
        || raw.contains(':') || raw.starts_with("//") || raw.ends_with('/')
        || raw.split('/').any(|part| part == ".." || part.is_empty())
    {
        return Err(invalid("ambiguous or traversing path"));
    }
    let canonical = raw.strip_prefix("./").unwrap_or(raw);
    if canonical.is_empty() || canonical == "." {
        return Err(invalid("ambiguous path root"));
    }
    // Allow Unix absolute paths as **requests** only; no ambient grant.
    let usable = if raw.starts_with('/') { raw } else { canonical };
    Ok(usable.to_owned())
}

fn canonical_env(raw: &str) -> Result<String, String> {
    let mut iter = raw.bytes();
    if !matches!(iter.next(), Some(b'A'..=b'Z' | b'a'..=b'z' | b'_'))
        || !iter.all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(invalid("invalid environment key"));
    }
    Ok(raw.to_owned())
}

fn canonical_symbol(raw: &str) -> Result<String, String> {
    if raw.len() > 255 || raw.starts_with('-')
        || !raw.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_./".contains(&b))
        || raw.split('/').any(|part| part == "..")
    {
        return Err(invalid("invalid capability name"));
    }
    Ok(raw.to_owned())
}

fn port(raw: &str) -> Result<u16, String> {
    if raw.is_empty() || raw.starts_with('0') || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid("noncanonical network port"));
    }
    let number: u16 = raw.parse().map_err(|_| invalid("network port out of range"))?;
    if number == 0 { return Err(invalid("network port zero")); }
    Ok(number)
}

fn canonical_net(raw: &str) -> Result<String, String> {
    // This validates the requested endpoint only. The host must recheck DNS
    // answers and concrete connection IPs on each attempt and redirect.
    if let Some(without_open) = raw.strip_prefix('[') {
        let (ip, remainder) = without_open.split_once(']')
            .ok_or_else(|| invalid("unclosed IPv6 address"))?;
        let ip = Ipv6Addr::from_str(ip).map_err(|_| invalid("invalid IPv6 literal"))?;
        let suffix = if remainder.is_empty() {
            String::new()
        } else {
            let number = port(remainder.strip_prefix(':')
                .ok_or_else(|| invalid("invalid IPv6 endpoint suffix"))?)?;
            format!(":{number}")
        };
        return Ok(format!("[{ip}]{suffix}"));
    }
    let mut parts = raw.split(':');
    let host = parts.next().unwrap_or_default();
    let port_part = parts.next();
    if parts.next().is_some() || host.is_empty() || host.contains('[') || host.contains(']') {
        return Err(invalid("invalid network authority"));
    }
    let suffix = port_part.map(port).transpose()?.map_or_else(String::new, |n| format!(":{n}"));
    let wildcard = host.starts_with("*.");
    let host = host.strip_prefix("*.").unwrap_or(host);
    let canonical = host.trim_end_matches('.').to_ascii_lowercase();
    if canonical.is_empty() || canonical.len() > 253
        || (wildcard && !canonical.contains('.'))
    {
        return Err(invalid("invalid network hostname"));
    }
    let dns_valid = canonical.split('.').all(|part| {
        !part.is_empty() && part.len() <= 63
            && !part.starts_with('-') && !part.ends_with('-')
            && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    });
    if !dns_valid { return Err(invalid("invalid DNS hostname")); }
    // For IPv4, reject noncanonical dotted-quad spellings to prevent aliasing.
    if canonical.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && Ipv4Addr::from_str(&canonical).is_err()
    {
        return Err(invalid("noncanonical IPv4 address"));
    }
    Ok(format!("{}{}{}", if wildcard { "*." } else { "" }, canonical, suffix))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parsed(values: &[(&str, &str)]) -> PermissionRequest {
        let flags: HashMap<String, String> = values.iter()
            .map(|(k, v)| (k.to_string(), v.to_string())).collect();
        PermissionRequest::from_argv_only(&flags).unwrap()
    }

    #[test]
    fn empty_or_denial_only_is_never_a_grant() {
        assert!(parsed(&[]).is_deny_all_request());
        assert!(parsed(&[("ATTO_DENY_READ", "*")]).is_deny_all_request());
        assert!(parsed(&[("ATTO_ALLOW_ALL", "false")]).is_deny_all_request());
    }
    #[test]
    fn argv_only_request_does_not_mint_artifact_or_tenant_capability() {
        let req = parsed(&[("ATTO_ALLOW_READ", "./data"), ("ATTO_DENY_READ", "./data/secret")]);
        assert!(!req.allow_all);
        assert!(req.requests_privileged_io());
        assert_eq!(req.allows.get(&Kind::Read),
            Some(&Scope::Some(BTreeSet::from(["data".to_string()]))));
        assert_eq!(req.denies.get(&Kind::Read),
            Some(&Scope::Some(BTreeSet::from(["data/secret".to_string()]))));
        assert!(!req.allows.contains_key(&Kind::Write));
    }
    #[test]
    fn canonical_net_distinguishes_ports_and_wildcards() {
        assert_eq!(canonical_net("EXAMPLE.com.:443").unwrap(), "example.com:443");
        assert_eq!(canonical_net("*.EXAMPLE.com:443").unwrap(), "*.example.com:443");
        assert_eq!(canonical_net("[2001:0db8::1]:443").unwrap(), "[2001:db8::1]:443");
        for invalid_net in ["example.com:0", "example.com:65536",
          "example.com:0443", "*.com", "example..com", "127.1", "[fe80::1", "a:80:80",
          "example.com,"] {
          assert!(parse_scope(Kind::Net, invalid_net).is_err(), "{invalid_net}");
        }
    }
    #[test]
    fn bad_scopes_are_rejected_not_ignored() {
        for s in ["", "a,", ",a", "a,,b", "a, b", "a,\nb", "a,a", "a,*"] {
            assert!(parse_scope(Kind::Read, s).is_err(), "{s}");
        }
        for s in ["../data", "./../data", "/tmp/../secret", "C:\\secret", "~/secret"] {
            assert!(parse_scope(Kind::Write, s).is_err(), "{s}");
        }
        for s in ["SECRET=oops", "ABC-WRONG", "1FIRST"] {
            assert!(parse_scope(Kind::Env, s).is_err(), "{s}");
        }
    }
    #[test]
    fn permission_check_compile_is_not_a_runtime_bypass() {
        let request = parsed(&[("ATTO_PERMISSION_CHECK", "compile"),
            ("ATTO_ALLOW_NET", "example.com")]);
        assert!(request.check_at_compile);
        assert!(request.requests_privileged_io());
        assert!(PermissionRequest::from_argv_only(&HashMap::from([
            ("ATTO_PERMISSION_CHECK".to_string(), "off".to_string())
        ])).is_err());
    }
    #[test]
    fn deny_all_override_remains_visible() {
        let request = parsed(&[("ATTO_ALLOW_ALL", "true"), ("ATTO_DENY_NET", "*")]);
        assert!(request.allow_all);
        assert_eq!(request.denies.get(&Kind::Net), Some(&Scope::All));
    }
    #[test]
    fn false_no_prompt_is_not_authorization_to_prompt() {
        assert!(PermissionRequest::from_argv_only(&HashMap::from([
            ("ATTO_NO_PROMPT".to_string(), "false".to_string())
        ])).is_err());
    }
}
