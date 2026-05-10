use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub name: String,
    pub meeting_dir: PathBuf,
    pub audio_path: PathBuf,
    pub transcript_path: Option<PathBuf>,
    pub protocol_path: Option<PathBuf>,
    pub transcript_text: Option<String>,
    pub protocol_text: Option<String>,
    pub created_at: i64,
}

impl Meeting {
    pub fn new(name: String, audio_path: PathBuf) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            meeting_dir: PathBuf::new(),
            audio_path,
            transcript_path: None,
            protocol_path: None,
            transcript_text: None,
            protocol_text: None,
            created_at: now_unix(),
        }
    }
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock went before epoch")
        .as_secs() as i64
}
