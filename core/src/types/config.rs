use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MuxSettings {
    pub project_root: String,
    /// Maximum width (in px) for overlay zone containers. Default: 160.
    #[serde(default = "default_zone_max_width")]
    pub zone_max_width: u32,
    /// Maximum rows in the Spotlight-style search dropdown. Default: 10.
    #[serde(default = "default_search_max_rows")]
    pub search_max_rows: u32,
}

fn default_zone_max_width() -> u32 {
    160
}

fn default_search_max_rows() -> u32 {
    10
}

impl Default for MuxSettings {
    fn default() -> Self {
        MuxSettings {
            project_root: String::new(),
            zone_max_width: default_zone_max_width(),
            search_max_rows: default_search_max_rows(),
        }
    }
}
