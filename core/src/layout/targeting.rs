//! Target resolver — translates agent names and P-notation into tmux pane IDs.
//!
//! Supports two addressing schemes:
//!
//! - **P-notation:** `P0` (first pane), `P0.1` (second pane of first window),
//!   `P2.0` (first pane of third window). Maps to `session:window.pane`.
//! - **Agent name:** looks up the agent's assigned session and pane from the
//!   agent list.

use crate::types::agent::Agent;

/// Resolve a target string to a tmux pane identifier.
///
/// # Target formats
///
/// - `P<window>` — shorthand for pane 0 of the given window, e.g. `P0` -> `:0.0`
/// - `P<window>.<pane>` — specific pane in a window, e.g. `P1.2` -> `:1.2`
/// - Any other string — treated as an agent name, looked up in the agents list.
///
/// # Returns
///
/// A tmux-compatible target string (e.g. `"session:0.1"`) or an error message.
pub fn resolve(target: &str, agents: &[Agent]) -> Result<String, String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err("empty target string".to_string());
    }

    // P-notation: must start with P/p followed by a digit.
    if is_p_notation(trimmed) {
        return resolve_p_notation(trimmed);
    }

    // Agent name lookup
    resolve_agent_name(trimmed, agents)
}

/// Check if a target string looks like P-notation (P/p followed by a digit).
fn is_p_notation(s: &str) -> bool {
    (s.starts_with('P') || s.starts_with('p'))
        && s.len() > 1
        && s.as_bytes()[1].is_ascii_digit()
}

/// Parse P-notation target (e.g. `P0`, `P1.2`).
fn resolve_p_notation(target: &str) -> Result<String, String> {
    let body = &target[1..]; // strip the 'P' or 'p'

    if body.is_empty() {
        return Err("P-notation requires a window number (e.g. P0, P1.2)".to_string());
    }

    if body.contains('.') {
        // P<window>.<pane>
        let parts: Vec<&str> = body.splitn(2, '.').collect();
        let window = parts[0]
            .parse::<u32>()
            .map_err(|_| format!("invalid window number in P-notation: '{}'", parts[0]))?;
        let pane = parts[1]
            .parse::<u32>()
            .map_err(|_| format!("invalid pane number in P-notation: '{}'", parts[1]))?;
        Ok(format!(":{}.{}", window, pane))
    } else {
        // P<window> — defaults to pane 0
        let window = body
            .parse::<u32>()
            .map_err(|_| format!("invalid window number in P-notation: '{}'", body))?;
        Ok(format!(":{}.0", window))
    }
}

/// Resolve an agent name to a tmux target string.
fn resolve_agent_name(name: &str, agents: &[Agent]) -> Result<String, String> {
    let agent = agents
        .iter()
        .find(|a| a.name == name)
        .ok_or_else(|| format!("unknown agent: '{}'", name))?;

    let session = agent
        .session
        .as_ref()
        .ok_or_else(|| format!("agent '{}' has no session assigned", name))?;

    // Return the session name as the target. The pane within the session
    // is managed by the placement system; here we return the session-level
    // target which tmux will resolve to the active pane.
    Ok(session.clone())
}

/// Validate that a target string looks syntactically correct without resolving
/// it against live state.
pub fn validate_format(target: &str) -> Result<(), String> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return Err("empty target".to_string());
    }
    // Bare "P" or "p" is ambiguous — reject it.
    if trimmed == "P" || trimmed == "p" {
        return Err("bare 'P' is ambiguous; use P0, P1.2, etc.".to_string());
    }
    if is_p_notation(trimmed) {
        // Validate P-notation structure.
        let body = &trimmed[1..];
        if body.is_empty() {
            return Err("P-notation requires a window number".to_string());
        }
        let parts: Vec<&str> = body.split('.').collect();
        if parts.len() > 2 {
            return Err(format!(
                "P-notation has too many components: '{}'",
                trimmed
            ));
        }
        for part in parts {
            if part.parse::<u32>().is_err() {
                return Err(format!("non-numeric component in P-notation: '{}'", part));
            }
        }
        return Ok(());
    }
    // Agent names: must be non-empty and contain only valid identifier chars.
    if trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        Ok(())
    } else {
        Err(format!(
            "invalid characters in target name: '{}'",
            trimmed
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::agent::{AgentStatus, AgentType, HealthState};

    fn make_agent(name: &str, session: Option<&str>) -> Agent {
        Agent {
            name: name.into(),
            role: "worker".into(),
            agent_type: AgentType::Claude,
            task: None,
            path: "/tmp".into(),
            status: AgentStatus::Idle,
            status_notes: String::new(),
            health: HealthState::Healthy,
            last_heartbeat_ms: None,
            session: session.map(|s| s.into()),
        }
    }

    #[test]
    fn p_notation_window_only() {
        let agents: Vec<Agent> = vec![];
        assert_eq!(resolve("P0", &agents).unwrap(), ":0.0");
        assert_eq!(resolve("P3", &agents).unwrap(), ":3.0");
    }

    #[test]
    fn p_notation_window_and_pane() {
        let agents: Vec<Agent> = vec![];
        assert_eq!(resolve("P0.1", &agents).unwrap(), ":0.1");
        assert_eq!(resolve("P2.3", &agents).unwrap(), ":2.3");
    }

    #[test]
    fn p_notation_lowercase() {
        let agents: Vec<Agent> = vec![];
        assert_eq!(resolve("p1.2", &agents).unwrap(), ":1.2");
    }

    #[test]
    fn p_notation_invalid_window() {
        let agents: Vec<Agent> = vec![];
        assert!(resolve("Pabc", &agents).is_err());
    }

    #[test]
    fn p_notation_empty() {
        let agents: Vec<Agent> = vec![];
        assert!(resolve("P", &agents).is_err());
    }

    #[test]
    fn agent_name_found() {
        let agents = vec![make_agent("worker-1", Some("cmx-main"))];
        let result = resolve("worker-1", &agents).unwrap();
        assert_eq!(result, "cmx-main");
    }

    #[test]
    fn agent_name_not_found() {
        let agents = vec![make_agent("worker-1", Some("cmx-main"))];
        assert!(resolve("nonexistent", &agents).is_err());
    }

    #[test]
    fn agent_no_session() {
        let agents = vec![make_agent("worker-1", None)];
        assert!(resolve("worker-1", &agents).is_err());
    }

    #[test]
    fn empty_target_error() {
        let agents: Vec<Agent> = vec![];
        assert!(resolve("", &agents).is_err());
    }

    #[test]
    fn validate_p_notation_ok() {
        assert!(validate_format("P0").is_ok());
        assert!(validate_format("P1.2").is_ok());
        assert!(validate_format("p3").is_ok());
    }

    #[test]
    fn validate_p_notation_bad() {
        assert!(validate_format("P").is_err());
        assert!(validate_format("P1.2.3").is_err());
    }

    #[test]
    fn p_prefix_non_digit_is_agent_name() {
        // "Pabc" starts with P but is followed by letters, not digits.
        // It is treated as a valid agent name, not P-notation.
        assert!(validate_format("Pabc").is_ok());
    }

    #[test]
    fn validate_agent_name_ok() {
        assert!(validate_format("worker-1").is_ok());
        assert!(validate_format("pm_agent").is_ok());
    }

    #[test]
    fn validate_agent_name_bad() {
        assert!(validate_format("has space").is_err());
        assert!(validate_format("").is_err());
    }
}
