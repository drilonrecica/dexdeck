use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogPriority {
    Verbose,
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogMarkerKind {
    JavaCrash,
    NativeCrash,
    Anr,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogRecord {
    pub timestamp: String,
    pub process_id: u32,
    pub thread_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<u32>,
    pub priority: LogPriority,
    pub tag: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    pub continuation: bool,
    pub crash_boundary: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<LogMarkerKind>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}
