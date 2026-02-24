//! Snapshot timer — tracks when each session was last captured and determines
//! which sessions are due for a new layout snapshot.

use std::collections::HashMap;


/// Tracks per-session capture timestamps and determines which sessions
/// are due for a new layout snapshot based on a configurable interval.
pub struct SnapshotTimer {
    interval_ms: u64,
    last_capture: HashMap<String, u64>,
}


impl SnapshotTimer {
    /// Create a new timer with the given interval in milliseconds.
    pub fn new(interval_ms: u64) -> Self {
        SnapshotTimer {
            interval_ms,
            last_capture: HashMap::new(),
        }
    }

    /// Returns sessions that are due for a snapshot.
    ///
    /// A session is due if it has never been captured or if the time since
    /// its last capture exceeds the configured interval.
    pub fn sessions_due(&self, sessions: &[String], now_ms: u64) -> Vec<String> {
        sessions
            .iter()
            .filter(|s| {
                match self.last_capture.get(s.as_str()) {
                    Some(&last) => now_ms.saturating_sub(last) >= self.interval_ms,
                    None => true, // never captured — immediately due
                }
            })
            .cloned()
            .collect()
    }

    /// Record that a session was captured at the given timestamp.
    pub fn record_capture(&mut self, session: &str, now_ms: u64) {
        self.last_capture.insert(session.to_string(), now_ms);
    }

    /// Remove a session from tracking (e.g. when a session is destroyed).
    pub fn remove_session(&mut self, session: &str) {
        self.last_capture.remove(session);
    }

    /// Return the configured interval.
    pub fn interval_ms(&self) -> u64 {
        self.interval_ms
    }
}


// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_is_immediately_due() {
        let timer = SnapshotTimer::new(5000);
        let sessions = vec!["s1".to_string()];
        let due = timer.sessions_due(&sessions, 10000);
        assert_eq!(due, vec!["s1"]);
    }

    #[test]
    fn recently_captured_session_is_not_due() {
        let mut timer = SnapshotTimer::new(5000);
        timer.record_capture("s1", 10000);
        let sessions = vec!["s1".to_string()];
        // Only 1000ms since capture, interval is 5000ms
        let due = timer.sessions_due(&sessions, 11000);
        assert!(due.is_empty());
    }

    #[test]
    fn session_past_interval_is_due() {
        let mut timer = SnapshotTimer::new(5000);
        timer.record_capture("s1", 10000);
        let sessions = vec!["s1".to_string()];
        // 6000ms since capture, interval is 5000ms
        let due = timer.sessions_due(&sessions, 16000);
        assert_eq!(due, vec!["s1"]);
    }

    #[test]
    fn removed_session_is_no_longer_tracked() {
        let mut timer = SnapshotTimer::new(5000);
        timer.record_capture("s1", 10000);
        timer.remove_session("s1");
        // After removal, the session has no last_capture entry, so it's
        // immediately due again if it reappears in the session list
        let sessions = vec!["s1".to_string()];
        let due = timer.sessions_due(&sessions, 10001);
        assert_eq!(due, vec!["s1"]);
    }

    #[test]
    fn mixed_due_and_not_due() {
        let mut timer = SnapshotTimer::new(5000);
        timer.record_capture("s1", 10000);
        timer.record_capture("s2", 5000);
        // s3 has never been captured
        let sessions = vec!["s1".to_string(), "s2".to_string(), "s3".to_string()];
        // At time 12000:
        //   s1: 2000ms ago -> not due
        //   s2: 7000ms ago -> due
        //   s3: never captured -> due
        let due = timer.sessions_due(&sessions, 12000);
        assert_eq!(due, vec!["s2", "s3"]);
    }

    #[test]
    fn exact_interval_boundary_is_due() {
        let mut timer = SnapshotTimer::new(5000);
        timer.record_capture("s1", 10000);
        let sessions = vec!["s1".to_string()];
        // Exactly 5000ms since capture
        let due = timer.sessions_due(&sessions, 15000);
        assert_eq!(due, vec!["s1"]);
    }

    #[test]
    fn interval_accessor() {
        let timer = SnapshotTimer::new(3000);
        assert_eq!(timer.interval_ms(), 3000);
    }
}
