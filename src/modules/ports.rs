// SPDX-License-Identifier: MIT OR Apache-2.0
//! The `ports` module (feature `ports`): `provider = "ports"` on an `int` (a port) or a `string` (a
//! `host:port` address). Rules P1 to P4 of `docs/MODULES.md` section 2; they are pure functions of the text
//! (no socket, no lookup), so a load stays deterministic.
//!
//! A refusal names the rule and says what is wrong, but never quotes the value: an address string can hold
//! a credential by mistake (`user:pw@host:80`), and the loader already replaces the whole reason of a
//! `secret` key with `<redacted>`.

use std::net::{Ipv4Addr, Ipv6Addr};

use crate::module::{Choice, Module, Needs};
use crate::schema::ValueDef;
use crate::value::Value;

const FIRST_UNPRIVILEGED: u32 = 1024;
const MAX_LABEL: usize = 63;
const MAX_NAME: usize = 253;

/// The module a schema's `provider = "ports"` names. Options belong to the application, not the schema
/// (the one schema-side switch is the key's `privileged = "<reason>"`).
#[derive(Debug, Clone, Default)]
pub struct Ports {
    allow_zero: bool,
    suggested: Vec<u16>,
}

impl Ports {
    pub fn new() -> Self {
        Self::default()
    }

    /// Accept port `0` ("let the system choose"), which P1 otherwise refuses.
    pub fn allow_zero(mut self) -> Self {
        self.allow_zero = true;
        self
    }

    /// Ports `list_choices` offers (it never probes). Without this it offers none.
    pub fn suggest(mut self, ports: &[u16]) -> Self {
        self.suggested = ports.to_vec();
        self
    }

    fn check_port(&self, port: u32, privileged: Option<&str>, key: &str) -> Result<(), String> {
        if port == 0 {
            return if self.allow_zero {
                Ok(())
            } else {
                Err("ports P1: port 0 is reserved (\"any port\"); use 1 to 65535".into())
            };
        }
        if port > 65535 {
            return Err("ports P1: a port is 1 to 65535".into());
        }
        if port < FIRST_UNPRIVILEGED && privileged.is_none() {
            return Err(format!(
                "ports P3: a port below {FIRST_UNPRIVILEGED} needs the privileges of a service; \
                 declare `privileged = \"<reason>\"` on `{key}` in the schema to allow it"
            ));
        }
        Ok(())
    }
}

impl Module for Ports {
    fn name(&self) -> &str {
        "ports"
    }

    fn interface_version(&self) -> u32 {
        2 // implements `validate_all`
    }

    fn list_choices(&self, _needs: &Needs) -> Vec<Choice> {
        self.suggested
            .iter()
            .map(|p| Choice::new(&p.to_string(), "suggested by the application"))
            .collect()
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        // Without the key's declaration no port below 1024 is allowed.
        check(self, value, None, "this key")
    }

    fn validate_for(&self, def: &ValueDef, value: &Value) -> Result<(), String> {
        check(self, value, def.privileged.as_deref(), &def.key)
    }

    /// P5: two keys that bind the same port on overlapping hosts. Port 0 (the system chooses) never collides.
    /// Each key at fault gets a message naming the other key and never a value.
    fn validate_all(&self, values: &[(&str, &Value)]) -> Vec<(String, String)> {
        let ends: Vec<(&str, u32, Option<String>)> = values
            .iter()
            .filter_map(|(k, v)| endpoint(v).map(|(p, h)| (*k, p, h)))
            .filter(|(_, p, _)| *p != 0)
            .collect();
        let mut out = Vec::new();
        for (i, (key, port, host)) in ends.iter().enumerate() {
            let other = ends
                .iter()
                .enumerate()
                .find(|(j, (_, p, h))| *j != i && p == port && hosts_overlap(host, h));
            if let Some((_, (other_key, _, _))) = other {
                out.push((
                    (*key).to_owned(),
                    format!(
                        "ports P5: this key binds the same port as `{other_key}`; give each listener its own port"
                    ),
                ));
            }
        }
        out
    }
}

fn check(ports: &Ports, value: &Value, privileged: Option<&str>, key: &str) -> Result<(), String> {
    match value {
        Value::Int(n) => {
            let port =
                u32::try_from(*n).map_err(|_| "ports P1: a port is 1 to 65535".to_owned())?;
            ports.check_port(port, privileged, key)
        }
        Value::String(text) => {
            let (port, _) = split_address(text)?;
            ports.check_port(port, privileged, key)
        }
        other => Err(format!(
            "ports P2: expected an int port or a `host:port` string, found {}",
            other.type_of().name()
        )),
    }
}

/// Check `host:port` and return the port and the host (lower case, without brackets).
fn split_address(text: &str) -> Result<(u32, String), String> {
    let bad = |why: &str| format!("ports P2: not a `host:port` address: {why}");
    let (host_text, port) = if let Some(rest) = text.strip_prefix('[') {
        let (inside, after) = rest
            .split_once(']')
            .ok_or_else(|| bad("an IPv6 host needs a closing `]`"))?;
        let port = after
            .strip_prefix(':')
            .ok_or_else(|| bad("a `:port` must follow the `]`"))?;
        if inside.parse::<Ipv6Addr>().is_err() {
            return Err(bad("the text in `[...]` is not an IPv6 address"));
        }
        (inside.to_ascii_lowercase(), port)
    } else {
        let (host, port) = text
            .rsplit_once(':')
            .ok_or_else(|| bad("there is no `:port`"))?;
        if host.contains(':') {
            return Err(bad("an IPv6 host must be written in `[...]`"));
        }
        if host.is_empty() {
            return Err(bad("the host is empty"));
        }
        check_host(host).map_err(bad)?;
        (host.to_ascii_lowercase(), port)
    };
    Ok((parse_port(port).map_err(bad)?, host_text))
}

/// Where a value listens, for P5: its port and its host (`None` for a bare int port, which means every host).
fn endpoint(value: &Value) -> Option<(u32, Option<String>)> {
    match value {
        Value::Int(n) => u32::try_from(*n).ok().map(|p| (p, None)),
        Value::String(text) => split_address(text).ok().map(|(p, h)| (p, Some(h))),
        _ => None,
    }
}

fn is_wildcard(host: &Option<String>) -> bool {
    host.as_deref()
        .is_none_or(|h| h == "0.0.0.0" || h.parse::<Ipv6Addr>().is_ok_and(|a| a.is_unspecified()))
}

/// Can two listeners on the same port bind at once? Not if the hosts are the same, or either is a wildcard.
fn hosts_overlap(a: &Option<String>, b: &Option<String>) -> bool {
    a == b || is_wildcard(a) || is_wildcard(b)
}

fn parse_port(text: &str) -> Result<u32, &'static str> {
    if text.is_empty() {
        return Err("the port is empty");
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err("the port is not a number");
    }
    // More than five digits is out of range whatever they are; 0 and 65536+ are judged by `check_port`.
    if text.len() > 5 {
        return Err("a port is 1 to 65535");
    }
    text.parse().map_err(|_| "the port is not a number")
}

/// An IPv4 literal or a DNS name (labels of 1 to 63 letters, digits and `-`, none starting or ending with
/// `-`, 253 in all). A name made only of digits and dots must be a valid IPv4 address.
fn check_host(host: &str) -> Result<(), &'static str> {
    if host.parse::<Ipv4Addr>().is_ok() {
        return Ok(());
    }
    if host.len() > MAX_NAME {
        return Err("the host name is longer than 253 characters");
    }
    if host.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return Err("the host looks like an IPv4 address but is not one");
    }
    for label in host.split('.') {
        if label.is_empty() || label.len() > MAX_LABEL {
            return Err("a host name label is empty or longer than 63 characters");
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("a host name label may not start or end with `-`");
        }
        if !label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err("a host name holds only letters, digits, `-` and `.`");
        }
    }
    Ok(())
}
