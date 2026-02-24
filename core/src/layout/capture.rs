//! Layout capture pipeline — get panes, reconstruct tree, diff, persist.
//!
//! Wires together the tmux pane parser, the snapshot reconstruction algorithm,
//! and the layout expression serializer to produce a `CaptureResult` that
//! indicates whether the layout has changed since the last capture.

use std::collections::HashMap;

use crate::data::layout_expr;
use crate::infrastructure::tmux;
use crate::layout::snapshot;
use crate::types::session::LayoutNode;


/// Result of a layout capture attempt.
#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub session: String,
    pub layout: LayoutNode,
    pub layout_expr: String,
    pub changed: bool,
    pub timestamp_ms: u64,
}


/// Capture the current tmux layout for a session.
///
/// Parses pane geometry from `pane_output`, reconstructs the ROW/COL tree,
/// serializes it to a layout expression string, and compares against the
/// previous known layout expression.
pub fn capture_session(
    session: &str,
    pane_output: &str,
    previous_expr: Option<&str>,
    now_ms: u64,
) -> Result<CaptureResult, String> {
    // 1. Parse tmux list-panes output into TmuxPane structs
    let panes = tmux::parse_list_panes(pane_output);
    if panes.is_empty() {
        return Err(format!("No panes found for session '{}'", session));
    }
    // 2. Reconstruct layout tree
    let layout = snapshot::from_panes(&panes);
    // 3. Serialize to expression string
    let layout_expr_str = layout_expr::serialize_layout_expr(&layout);
    // 4. Compare against previous
    let changed = match previous_expr {
        Some(prev) => prev != layout_expr_str,
        None => true,
    };
    Ok(CaptureResult {
        session: session.to_string(),
        layout,
        layout_expr: layout_expr_str,
        changed,
        timestamp_ms: now_ms,
    })
}


/// Capture all sessions and return results.
///
/// Iterates over the given session names, looks up their pane output and
/// previous layout expression, and runs `capture_session` for each.
/// Sessions without pane output are silently skipped.
pub fn capture_all_sessions(
    sessions: &[String],
    pane_outputs: &HashMap<String, String>,
    previous_layouts: &HashMap<String, String>,
    now_ms: u64,
) -> Vec<CaptureResult> {
    sessions
        .iter()
        .filter_map(|s| {
            let output = pane_outputs.get(s)?;
            let prev = previous_layouts.get(s).map(|s| s.as_str());
            capture_session(s, output, prev, now_ms).ok()
        })
        .collect()
}


// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::session::LayoutNode;

    // Helper: build a pane line in the format parse_list_panes expects
    // Format: %id:index:width:height:top:left
    fn pane_line(id: &str, index: u32, w: u32, h: u32, top: u32, left: u32) -> String {
        format!("{}:{}:{}:{}:{}:{}", id, index, w, h, top, left)
    }

    #[test]
    fn single_pane_session_produces_leaf() {
        let output = pane_line("%0", 0, 120, 40, 0, 0);
        let result = capture_session("test", &output, None, 1000).unwrap();
        assert!(result.changed);
        assert_eq!(result.session, "test");
        assert_eq!(result.timestamp_ms, 1000);
        match &result.layout {
            LayoutNode::Pane { agent } => assert_eq!(agent, ""),
            other => panic!("expected Pane, got {:?}", other),
        }
    }

    #[test]
    fn two_horizontal_panes_produce_row() {
        let output = format!(
            "{}\n{}",
            pane_line("%0", 0, 60, 40, 0, 0),
            pane_line("%1", 1, 60, 40, 0, 60),
        );
        let result = capture_session("test", &output, None, 1000).unwrap();
        assert!(result.changed);
        assert!(result.layout_expr.contains("ROW"));
        match &result.layout {
            LayoutNode::Row { children } => assert_eq!(children.len(), 2),
            other => panic!("expected Row, got {:?}", other),
        }
    }

    #[test]
    fn two_vertical_panes_produce_col() {
        let output = format!(
            "{}\n{}",
            pane_line("%0", 0, 120, 20, 0, 0),
            pane_line("%1", 1, 120, 20, 20, 0),
        );
        let result = capture_session("test", &output, None, 1000).unwrap();
        assert!(result.changed);
        assert!(result.layout_expr.contains("COL"));
        match &result.layout {
            LayoutNode::Col { children } => assert_eq!(children.len(), 2),
            other => panic!("expected Col, got {:?}", other),
        }
    }

    #[test]
    fn nested_layout_two_top_one_bottom() {
        // Two panes on top (side by side), one on bottom spanning full width
        let output = format!(
            "{}\n{}\n{}",
            pane_line("%0", 0, 60, 20, 0, 0),
            pane_line("%1", 1, 60, 20, 0, 60),
            pane_line("%2", 2, 120, 20, 20, 0),
        );
        let result = capture_session("test", &output, None, 1000).unwrap();
        assert!(result.changed);
        match &result.layout {
            LayoutNode::Col { children } => {
                assert_eq!(children.len(), 2);
                match &children[0].node {
                    LayoutNode::Row { children: row_kids } => {
                        assert_eq!(row_kids.len(), 2);
                    }
                    other => panic!("expected Row for top group, got {:?}", other),
                }
                match &children[1].node {
                    LayoutNode::Pane { .. } => {}
                    other => panic!("expected Pane for bottom, got {:?}", other),
                }
            }
            other => panic!("expected Col, got {:?}", other),
        }
    }

    #[test]
    fn no_panes_returns_error() {
        let result = capture_session("test", "", None, 1000);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No panes"));
    }

    #[test]
    fn same_layout_changed_is_false() {
        let output = pane_line("%0", 0, 120, 40, 0, 0);
        let first = capture_session("test", &output, None, 1000).unwrap();
        let second =
            capture_session("test", &output, Some(&first.layout_expr), 2000).unwrap();
        assert!(!second.changed);
    }

    #[test]
    fn different_layout_changed_is_true() {
        let output1 = pane_line("%0", 0, 120, 40, 0, 0);
        let output2 = format!(
            "{}\n{}",
            pane_line("%0", 0, 60, 40, 0, 0),
            pane_line("%1", 1, 60, 40, 0, 60),
        );
        let first = capture_session("test", &output1, None, 1000).unwrap();
        let second =
            capture_session("test", &output2, Some(&first.layout_expr), 2000).unwrap();
        assert!(second.changed);
    }

    #[test]
    fn no_previous_layout_changed_is_true() {
        let output = pane_line("%0", 0, 120, 40, 0, 0);
        let result = capture_session("test", &output, None, 1000).unwrap();
        assert!(result.changed);
    }

    #[test]
    fn capture_all_sessions_collects_results() {
        let sessions = vec!["s1".to_string(), "s2".to_string(), "s3".to_string()];
        let mut pane_outputs = HashMap::new();
        pane_outputs.insert("s1".to_string(), pane_line("%0", 0, 120, 40, 0, 0));
        pane_outputs.insert("s2".to_string(), format!(
            "{}\n{}",
            pane_line("%0", 0, 60, 40, 0, 0),
            pane_line("%1", 1, 60, 40, 0, 60),
        ));
        // s3 has no pane output — should be skipped
        let previous_layouts = HashMap::new();

        let results = capture_all_sessions(&sessions, &pane_outputs, &previous_layouts, 5000);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].session, "s1");
        assert_eq!(results[1].session, "s2");
    }

    #[test]
    fn capture_session_produces_valid_expr() {
        // Single pane capture produces a layout expression string.
        // Since tmux panes have no agent name, the expression is an
        // empty string for a single pane — which is the expected
        // serialization of Pane { agent: "" }.
        let output = pane_line("%0", 0, 120, 40, 0, 0);
        let result = capture_session("test", &output, None, 1000).unwrap();
        // The layout itself should be correct
        assert_eq!(
            result.layout,
            LayoutNode::Pane { agent: String::new() }
        );
        // The expression string is the serialized form
        assert_eq!(result.layout_expr, "");
    }

    #[test]
    fn layout_expr_is_parseable() {
        // Multi-pane layout expressions can be parsed (structural test).
        // Agent names from tmux are empty, so exact round-trip equality
        // isn't expected for entries with percents, but the expression is
        // always syntactically valid.
        let output = format!(
            "{}\n{}",
            pane_line("%0", 0, 60, 40, 0, 0),
            pane_line("%1", 1, 60, 40, 0, 60),
        );
        let result = capture_session("test", &output, None, 1000).unwrap();
        let reparsed =
            crate::data::layout_expr::parse_layout_expr(&result.layout_expr);
        assert!(reparsed.is_ok(), "layout expr should parse: {}", result.layout_expr);
        // Verify it's still a Row at the top level
        match reparsed.unwrap() {
            LayoutNode::Row { children } => assert_eq!(children.len(), 2),
            other => panic!("expected Row, got {:?}", other),
        }
    }
}
