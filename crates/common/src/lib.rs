use serde::{Deserialize, Serialize};

pub mod networking;

#[derive(Serialize, Deserialize)]
pub struct PingMessage {
    pub message: String,
}
