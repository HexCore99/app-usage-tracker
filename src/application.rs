use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct Application {
    pub(crate) title: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) display_name: String,
    pub(crate) executable: String,
    pub(crate) pid: u32,
    pub(crate) usage: AppUsage,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct AppUsage {
    pub(crate) total_time: Duration,
}
