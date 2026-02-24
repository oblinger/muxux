//! Layout snapshot — reconstruct `LayoutNode` tree from tmux pane geometry.
//!
//! When tmux reports pane positions and sizes, this module groups them into
//! rows (same `top`) and columns (same `left`) and builds a recursive
//! `LayoutNode` tree. This lets CMX compare the actual layout against the
//! desired layout and detect drift.

use crate::types::session::{LayoutEntry, LayoutNode, TmuxPane};

/// Reconstruct a `LayoutNode` tree from a flat list of pane geometries.
///
/// # Algorithm
///
/// 1. If there is only one pane, return a `Pane` node.
/// 2. Group panes by `top` coordinate. If there are multiple groups, this
///    is a column layout (panes stacked vertically).
/// 3. Within each row-group, if there are multiple panes, they form a row
///    layout (panes side by side horizontally).
/// 4. Recurse to handle nested splits.
/// 5. Compute percentage from pixel dimensions relative to the total.
pub fn from_panes(panes: &[TmuxPane]) -> LayoutNode {
    if panes.is_empty() {
        return LayoutNode::Pane {
            agent: String::new(),
        };
    }
    if panes.len() == 1 {
        return LayoutNode::Pane {
            agent: panes[0].agent.clone().unwrap_or_default(),
        };
    }

    // Check if panes can be grouped into vertical bands (same top = row members).
    let mut top_groups = group_by_top(panes);

    if top_groups.len() > 1 {
        // Multiple rows stacked vertically -> Col layout.
        top_groups.sort_by_key(|g| g[0].top);
        let total_height = compute_total_height(&top_groups);
        let children: Vec<LayoutEntry> = top_groups
            .iter()
            .map(|group| {
                let node = from_panes(group);
                let group_height = group_height(group);
                let percent = if total_height > 0 {
                    ((group_height as u64 * 100) / total_height as u64) as u32
                } else {
                    0
                };
                LayoutEntry {
                    node,
                    percent: Some(percent),
                }
            })
            .collect();
        return LayoutNode::Col { children };
    }

    // All panes share the same top -> they are side by side horizontally.
    let mut sorted = panes.to_vec();
    sorted.sort_by_key(|p| p.left);

    // Check if we can split by left coordinate into separate columns.
    let left_groups = group_by_left(&sorted);

    if left_groups.len() > 1 {
        let total_width = compute_total_width(&sorted);
        let children: Vec<LayoutEntry> = left_groups
            .iter()
            .map(|group| {
                let node = from_panes(group);
                let group_width = group_width(group);
                let percent = if total_width > 0 {
                    ((group_width as u64 * 100) / total_width as u64) as u32
                } else {
                    0
                };
                LayoutEntry {
                    node,
                    percent: Some(percent),
                }
            })
            .collect();
        return LayoutNode::Row { children };
    }

    // Fallback: treat each pane as a leaf in a row.
    let total_width = compute_total_width(&sorted);
    let children: Vec<LayoutEntry> = sorted
        .iter()
        .map(|p| {
            let percent = if total_width > 0 {
                ((p.width as u64 * 100) / total_width as u64) as u32
            } else {
                0
            };
            LayoutEntry {
                node: LayoutNode::Pane {
                    agent: p.agent.clone().unwrap_or_default(),
                },
                percent: Some(percent),
            }
        })
        .collect();
    LayoutNode::Row { children }
}

/// Compare two `LayoutNode` trees structurally. Returns `true` if they differ.
pub fn diff(a: &LayoutNode, b: &LayoutNode) -> bool {
    a != b
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Group panes by their `top` coordinate.
fn group_by_top(panes: &[TmuxPane]) -> Vec<Vec<TmuxPane>> {
    let mut map: Vec<(u32, Vec<TmuxPane>)> = Vec::new();
    for p in panes {
        if let Some(entry) = map.iter_mut().find(|(t, _)| *t == p.top) {
            entry.1.push(p.clone());
        } else {
            map.push((p.top, vec![p.clone()]));
        }
    }
    map.into_iter().map(|(_, v)| v).collect()
}

/// Group panes by their `left` coordinate.
fn group_by_left(panes: &[TmuxPane]) -> Vec<Vec<TmuxPane>> {
    let mut map: Vec<(u32, Vec<TmuxPane>)> = Vec::new();
    for p in panes {
        if let Some(entry) = map.iter_mut().find(|(l, _)| *l == p.left) {
            entry.1.push(p.clone());
        } else {
            map.push((p.left, vec![p.clone()]));
        }
    }
    map.sort_by_key(|(l, _)| *l);
    map.into_iter().map(|(_, v)| v).collect()
}

/// Compute the total width from a sorted list of panes.
fn compute_total_width(panes: &[TmuxPane]) -> u32 {
    if panes.is_empty() {
        return 0;
    }
    // Total = rightmost pane's (left + width) - leftmost pane's left.
    let min_left = panes.iter().map(|p| p.left).min().unwrap_or(0);
    let max_right = panes.iter().map(|p| p.left + p.width).max().unwrap_or(0);
    max_right - min_left
}

/// Compute the total height from grouped rows.
fn compute_total_height(groups: &[Vec<TmuxPane>]) -> u32 {
    if groups.is_empty() {
        return 0;
    }
    let min_top = groups
        .iter()
        .map(|g| g.iter().map(|p| p.top).min().unwrap_or(0))
        .min()
        .unwrap_or(0);
    let max_bottom = groups
        .iter()
        .map(|g| g.iter().map(|p| p.top + p.height).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    max_bottom - min_top
}

/// Height of a group of panes (they share the same top).
fn group_height(group: &[TmuxPane]) -> u32 {
    group.iter().map(|p| p.height).max().unwrap_or(0)
}

/// Width of a group of panes (they share the same left).
fn group_width(group: &[TmuxPane]) -> u32 {
    group.iter().map(|p| p.width).max().unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: &str, w: u32, h: u32, top: u32, left: u32, agent: Option<&str>) -> TmuxPane {
        TmuxPane {
            id: id.into(),
            index: 0,
            width: w,
            height: h,
            top,
            left,
            agent: agent.map(|a| a.into()),
        }
    }

    #[test]
    fn single_pane_becomes_leaf() {
        let panes = vec![pane("%0", 120, 40, 0, 0, Some("pilot"))];
        let layout = from_panes(&panes);
        assert_eq!(
            layout,
            LayoutNode::Pane {
                agent: "pilot".into()
            }
        );
    }

    #[test]
    fn two_side_by_side_panes_become_row() {
        let panes = vec![
            pane("%0", 60, 40, 0, 0, Some("left")),
            pane("%1", 60, 40, 0, 60, Some("right")),
        ];
        let layout = from_panes(&panes);
        match &layout {
            LayoutNode::Row { children } => {
                assert_eq!(children.len(), 2);
            }
            other => panic!("expected Row, got {:?}", other),
        }
    }

    #[test]
    fn two_stacked_panes_become_col() {
        let panes = vec![
            pane("%0", 120, 20, 0, 0, Some("top")),
            pane("%1", 120, 20, 20, 0, Some("bottom")),
        ];
        let layout = from_panes(&panes);
        match &layout {
            LayoutNode::Col { children } => {
                assert_eq!(children.len(), 2);
            }
            other => panic!("expected Col, got {:?}", other),
        }
    }

    #[test]
    fn empty_panes_returns_empty_leaf() {
        let layout = from_panes(&[]);
        assert_eq!(
            layout,
            LayoutNode::Pane {
                agent: String::new()
            }
        );
    }

    #[test]
    fn percentages_computed() {
        let panes = vec![
            pane("%0", 30, 40, 0, 0, Some("left")),
            pane("%1", 90, 40, 0, 30, Some("right")),
        ];
        let layout = from_panes(&panes);
        match &layout {
            LayoutNode::Row { children } => {
                assert_eq!(children.len(), 2);
                assert_eq!(children[0].percent, Some(25)); // 30/120 * 100 = 25
                assert_eq!(children[1].percent, Some(75)); // 90/120 * 100 = 75
            }
            other => panic!("expected Row, got {:?}", other),
        }
    }

    #[test]
    fn diff_same_layout_no_difference() {
        let a = LayoutNode::Pane {
            agent: "pilot".into(),
        };
        let b = LayoutNode::Pane {
            agent: "pilot".into(),
        };
        assert!(!diff(&a, &b));
    }

    #[test]
    fn diff_different_layouts() {
        let a = LayoutNode::Pane {
            agent: "pilot".into(),
        };
        let b = LayoutNode::Pane {
            agent: "worker".into(),
        };
        assert!(diff(&a, &b));
    }

    #[test]
    fn diff_row_vs_col() {
        let a = LayoutNode::Row {
            children: vec![LayoutEntry {
                node: LayoutNode::Pane {
                    agent: "x".into(),
                },
                percent: Some(100),
            }],
        };
        let b = LayoutNode::Col {
            children: vec![LayoutEntry {
                node: LayoutNode::Pane {
                    agent: "x".into(),
                },
                percent: Some(100),
            }],
        };
        assert!(diff(&a, &b));
    }

    #[test]
    fn agent_none_uses_empty_string() {
        let panes = vec![pane("%0", 120, 40, 0, 0, None)];
        let layout = from_panes(&panes);
        assert_eq!(
            layout,
            LayoutNode::Pane {
                agent: String::new()
            }
        );
    }

    #[test]
    fn nested_layout_reconstruction() {
        // Three panes: two on top row (side by side), one on bottom spanning full width.
        let panes = vec![
            pane("%0", 60, 20, 0, 0, Some("tl")),
            pane("%1", 60, 20, 0, 60, Some("tr")),
            pane("%2", 120, 20, 20, 0, Some("bottom")),
        ];
        let layout = from_panes(&panes);
        match &layout {
            LayoutNode::Col { children } => {
                assert_eq!(children.len(), 2);
                // Top row should be a Row with 2 children.
                match &children[0].node {
                    LayoutNode::Row { children: row_kids } => {
                        assert_eq!(row_kids.len(), 2);
                    }
                    other => panic!("expected Row for top group, got {:?}", other),
                }
                // Bottom should be a single pane.
                match &children[1].node {
                    LayoutNode::Pane { agent } => {
                        assert_eq!(agent, "bottom");
                    }
                    other => panic!("expected Pane for bottom, got {:?}", other),
                }
            }
            other => panic!("expected Col, got {:?}", other),
        }
    }
}
