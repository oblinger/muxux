use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MuxSettings {
    pub project_root: String,
}

impl Default for MuxSettings {
    fn default() -> Self {
        MuxSettings {
            project_root: String::new(),
        }
    }
}
