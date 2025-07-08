use serde::{Deserialize, Serialize};

pub mod networking;
pub mod simulation;

#[derive(Serialize, Deserialize)]
pub struct PingMessage {
    pub message: String,
}
