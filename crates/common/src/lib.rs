use serde::{Deserialize, Serialize};

pub mod networking;

#[derive(Serialize, Deserialize)]
pub struct Ping {
    pub message: String,
}
