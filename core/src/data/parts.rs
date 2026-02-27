//! Parts registry — parse and resolve parts.md into Tiles.
//!
//! Parts are reusable layout building blocks: agents (single pane), compositions
//! (multi-pane layouts), and sessions (complete templates referencing other parts).
//!
//! Format: markdown with `##` headings for names.
//!   - `role: <name>` → Agent
//!   - `ROW(...)` / `COL(...)` → Composition or Session (auto-classified)

use crate::data::layout_expr::parse_layout_expr;
use crate::types::session::LayoutNode;
use crate::types::tiles::{Tile, TileKind};

/// A registry of parsed parts, grouped by kind.
#[derive(Debug, Clone, Default)]
pub struct PartRegistry {
    pub parts: Vec<Tile>,
}

impl PartRegistry {
    /// Parse parts from markdown text (the contents of parts.md).
    pub fn from_markdown(input: &str) -> PartRegistry {
        let mut parts = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_body = String::new();

        for line in input.lines() {
            if let Some(heading) = line.strip_prefix("## ") {
                // Flush previous part
                if let Some(name) = current_name.take() {
                    if let Some(tile) = parse_part_body(&name, &current_body) {
                        parts.push(tile);
                    }
                }
                current_name = Some(heading.trim().to_string());
                current_body.clear();
            } else if line.starts_with("# ") || line.starts_with("### ") {
                // Skip H1 and H3+ headings
            } else if current_name.is_some() {
                current_body.push_str(line);
                current_body.push('\n');
            }
        }

        // Flush last part
        if let Some(name) = current_name.take() {
            if let Some(tile) = parse_part_body(&name, &current_body) {
                parts.push(tile);
            }
        }

        // Classify: distinguish compositions from sessions.
        // A part whose layout leaves are all agent names is a composition.
        // A part whose layout references other parts is a session.
        let agent_names: Vec<String> = parts
            .iter()
            .filter(|t| t.kind == TileKind::Agent)
            .map(|t| t.name.clone())
            .collect();
        let all_names: Vec<String> = parts.iter().map(|t| t.name.clone()).collect();

        for tile in &mut parts {
            if tile.kind == TileKind::Composition {
                if let Some(ref layout) = tile.layout {
                    if layout_references_parts(layout, &agent_names, &all_names) {
                        tile.kind = TileKind::Session;
                    }
                }
            }
        }

        PartRegistry { parts }
    }

    /// Load parts from a file path. Returns empty registry if file doesn't exist.
    pub fn from_file(path: &std::path::Path) -> PartRegistry {
        match std::fs::read_to_string(path) {
            Ok(content) => Self::from_markdown(&content),
            Err(_) => PartRegistry::default(),
        }
    }

    /// Load from the default location: `~/.config/skd/skd-library/parts.md`.
    pub fn from_default_path() -> PartRegistry {
        if let Some(home) = dirs_next_home() {
            let path = home.join(".config/skd/skd-library/parts.md");
            Self::from_file(&path)
        } else {
            PartRegistry::default()
        }
    }

    /// Find a part by name.
    pub fn get(&self, name: &str) -> Option<&Tile> {
        self.parts.iter().find(|t| t.name == name)
    }

    /// Get all parts of a given kind.
    pub fn by_kind(&self, kind: TileKind) -> Vec<&Tile> {
        self.parts.iter().filter(|t| t.kind == kind).collect()
    }

    /// Serialize the registry to JSON (for IPC to frontend).
    pub fn to_json(&self) -> String {
        serde_json::json!({
            "agents": self.by_kind(TileKind::Agent)
                .iter()
                .map(|t| serde_json::json!({ "name": t.name, "role": t.role }))
                .collect::<Vec<_>>(),
            "compositions": self.by_kind(TileKind::Composition)
                .iter()
                .map(|t| serde_json::json!({ "name": t.name }))
                .collect::<Vec<_>>(),
            "sessions": self.by_kind(TileKind::Session)
                .iter()
                .map(|t| serde_json::json!({ "name": t.name }))
                .collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// Recursively expand a part name into a fully resolved LayoutNode.
    ///
    /// Agent names become Pane nodes; compositions and sessions have their
    /// layout expressions expanded recursively.
    pub fn expand(&self, name: &str) -> Option<LayoutNode> {
        let tile = self.get(name)?;
        match tile.kind {
            TileKind::Agent => Some(LayoutNode::Pane {
                agent: name.to_string(),
            }),
            TileKind::Composition | TileKind::Session => {
                let layout = tile.layout.as_ref()?;
                Some(self.expand_node(layout))
            }
        }
    }

    /// Recursively expand layout references in a LayoutNode.
    fn expand_node(&self, node: &LayoutNode) -> LayoutNode {
        match node {
            LayoutNode::Pane { agent } => {
                // If this pane name is a known composition/session, expand it
                if let Some(tile) = self.get(agent) {
                    if tile.layout.is_some() {
                        if let Some(expanded) = self.expand(agent) {
                            return expanded;
                        }
                    }
                }
                // Otherwise keep as pane (it's a role name)
                LayoutNode::Pane {
                    agent: agent.clone(),
                }
            }
            LayoutNode::Row { children } => LayoutNode::Row {
                children: children
                    .iter()
                    .map(|e| crate::types::session::LayoutEntry {
                        node: self.expand_node(&e.node),
                        percent: e.percent,
                    })
                    .collect(),
            },
            LayoutNode::Col { children } => LayoutNode::Col {
                children: children
                    .iter()
                    .map(|e| crate::types::session::LayoutEntry {
                        node: self.expand_node(&e.node),
                        percent: e.percent,
                    })
                    .collect(),
            },
        }
    }
}

/// Parse the body lines of a single part.
fn parse_part_body(name: &str, body: &str) -> Option<Tile> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Check for `role: <name>` → Agent
    if let Some(role_line) = trimmed.lines().find(|l| l.trim().starts_with("role:")) {
        let role = role_line.trim().strip_prefix("role:")?.trim().to_string();
        return Some(Tile {
            name: name.to_string(),
            kind: TileKind::Agent,
            role: Some(role),
            layout: None,
        });
    }

    // Try parsing as layout expression — must start with ROW( or COL(
    let first_non_empty = trimmed.lines().find(|l| !l.trim().is_empty())?;
    let upper = first_non_empty.trim().to_uppercase();
    if !upper.starts_with("ROW(") && !upper.starts_with("COL(") {
        return None; // not a valid part definition
    }
    match parse_layout_expr(first_non_empty.trim()) {
        Ok(layout) => Some(Tile {
            name: name.to_string(),
            kind: TileKind::Composition, // may be reclassified to Session later
            role: None,
            layout: Some(layout),
        }),
        Err(_) => None, // unparseable body — skip
    }
}

/// Check if a layout's leaf names reference known parts (not just agents).
fn layout_references_parts(
    node: &LayoutNode,
    agent_names: &[String],
    all_names: &[String],
) -> bool {
    match node {
        LayoutNode::Pane { agent } => {
            // If the leaf is a known part name but NOT an agent, it's a part reference
            all_names.contains(agent) && !agent_names.contains(agent)
        }
        LayoutNode::Row { children } | LayoutNode::Col { children } => children
            .iter()
            .any(|e| layout_references_parts(&e.node, agent_names, all_names)),
    }
}

/// Get home directory (pure function to avoid platform dependency).
fn dirs_next_home() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(std::path::PathBuf::from)
}


#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PARTS: &str = r#"# Parts Library

## pm
role: pm

## worker
role: worker

## curator
role: curator

## remote
role: remote

## rig
COL(remote 70%, worker 30%)

## dev-pair
ROW(worker, worker)

## dev-station
COL(pm 30%, dev-pair 70%)

## gpu-station
COL(rig 80%, curator 20%)
"#;

    #[test]
    fn parse_agents() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let agents = reg.by_kind(TileKind::Agent);
        assert_eq!(agents.len(), 4);
        assert!(agents.iter().any(|t| t.name == "pm"));
        assert!(agents.iter().any(|t| t.name == "worker"));
        assert!(agents.iter().any(|t| t.name == "curator"));
        assert!(agents.iter().any(|t| t.name == "remote"));
    }

    #[test]
    fn agent_has_role() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let pm = reg.get("pm").unwrap();
        assert_eq!(pm.role.as_deref(), Some("pm"));
        assert_eq!(pm.kind, TileKind::Agent);
    }

    #[test]
    fn parse_compositions() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let comps = reg.by_kind(TileKind::Composition);
        assert_eq!(comps.len(), 2); // rig, dev-pair
        assert!(comps.iter().any(|t| t.name == "rig"));
        assert!(comps.iter().any(|t| t.name == "dev-pair"));
    }

    #[test]
    fn composition_has_layout() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let rig = reg.get("rig").unwrap();
        assert!(rig.layout.is_some());
        match rig.layout.as_ref().unwrap() {
            LayoutNode::Col { children } => {
                assert_eq!(children.len(), 2);
                assert_eq!(children[0].percent, Some(70));
            }
            _ => panic!("rig should be COL"),
        }
    }

    #[test]
    fn parse_sessions() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let sessions = reg.by_kind(TileKind::Session);
        assert_eq!(sessions.len(), 2); // dev-station, gpu-station
        assert!(sessions.iter().any(|t| t.name == "dev-station"));
        assert!(sessions.iter().any(|t| t.name == "gpu-station"));
    }

    #[test]
    fn session_references_composition() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let ds = reg.get("dev-station").unwrap();
        assert_eq!(ds.kind, TileKind::Session);
        // Layout should have dev-pair as a leaf reference
        match ds.layout.as_ref().unwrap() {
            LayoutNode::Col { children } => {
                assert_eq!(children.len(), 2);
                // Second child should be a Pane referencing dev-pair
                match &children[1].node {
                    LayoutNode::Pane { agent } => assert_eq!(agent, "dev-pair"),
                    _ => panic!("second child should be a Pane reference"),
                }
            }
            _ => panic!("dev-station should be COL"),
        }
    }

    #[test]
    fn to_json_groups() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let json: serde_json::Value = serde_json::from_str(&reg.to_json()).unwrap();
        assert_eq!(json["agents"].as_array().unwrap().len(), 4);
        assert_eq!(json["compositions"].as_array().unwrap().len(), 2);
        assert_eq!(json["sessions"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn expand_agent() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let node = reg.expand("pm").unwrap();
        match node {
            LayoutNode::Pane { agent } => assert_eq!(agent, "pm"),
            _ => panic!("agent should expand to Pane"),
        }
    }

    #[test]
    fn expand_composition() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let node = reg.expand("rig").unwrap();
        match node {
            LayoutNode::Col { children } => {
                assert_eq!(children.len(), 2);
                // Leaves should still be agent names (remote, worker)
                match &children[0].node {
                    LayoutNode::Pane { agent } => assert_eq!(agent, "remote"),
                    _ => panic!("expected Pane"),
                }
            }
            _ => panic!("rig should expand to Col"),
        }
    }

    #[test]
    fn expand_session_recursive() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        let node = reg.expand("dev-station").unwrap();
        // dev-station = COL(pm 30%, dev-pair 70%)
        // dev-pair = ROW(worker, worker)
        // So expanded: COL(pm 30%, ROW(worker, worker) 70%)
        match node {
            LayoutNode::Col { children } => {
                assert_eq!(children.len(), 2);
                match &children[0].node {
                    LayoutNode::Pane { agent } => assert_eq!(agent, "pm"),
                    _ => panic!("first child should be pm pane"),
                }
                match &children[1].node {
                    LayoutNode::Row { children } => {
                        assert_eq!(children.len(), 2);
                    }
                    _ => panic!("second child should be Row (expanded dev-pair)"),
                }
            }
            _ => panic!("dev-station should expand to Col"),
        }
    }

    #[test]
    fn get_nonexistent() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        assert!(reg.get("nonexistent").is_none());
        assert!(reg.expand("nonexistent").is_none());
    }

    #[test]
    fn empty_input() {
        let reg = PartRegistry::from_markdown("");
        assert!(reg.parts.is_empty());
    }

    #[test]
    fn missing_file() {
        let reg = PartRegistry::from_file(std::path::Path::new("/nonexistent/parts.md"));
        assert!(reg.parts.is_empty());
    }

    #[test]
    fn malformed_body_skipped() {
        let input = "## bad-part\nthis is not a valid layout or role\n\n## good-pm\nrole: pm\n";
        let reg = PartRegistry::from_markdown(input);
        assert_eq!(reg.parts.len(), 1);
        assert_eq!(reg.parts[0].name, "good-pm");
    }

    #[test]
    fn total_parts_count() {
        let reg = PartRegistry::from_markdown(SAMPLE_PARTS);
        assert_eq!(reg.parts.len(), 8); // 4 agents + 2 compositions + 2 sessions
    }
}
