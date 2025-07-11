use bevy::prelude::*;
use nevy::*;
use serde::{Deserialize, Serialize};

pub fn add_protocol(app: &mut App) {
    app.add_message::<PingMessage>();
}

pub enum StreamHeader {
    Messages,
}

impl From<StreamHeader> for u16 {
    fn from(value: StreamHeader) -> Self {
        value as u16
    }
}

pub fn log_connection_status(
    connection_q: Query<
        (Entity, &ConnectionOf, &QuicConnection, &ConnectionStatus),
        Changed<ConnectionStatus>,
    >,
    mut endpoint_q: Query<&mut QuicEndpoint>,
) -> Result {
    for (connection_entity, connection_of, connection, status) in &connection_q {
        let mut endpoint = endpoint_q.get_mut(**connection_of)?;

        let address = endpoint
            .get_connection(connection)
            .map(|connection| connection.get_remote_address());

        match status {
            ConnectionStatus::Connecting => {
                info!("New connection {} addr {:?}", connection_entity, address)
            }
            ConnectionStatus::Established => info!(
                "Connection {} addr {:?} established",
                connection_entity, address
            ),
            ConnectionStatus::Closed { reason } => info!(
                "Connection {} addr {:?} closed: {:?}",
                connection_entity, address, reason
            ),
            ConnectionStatus::Failed { error } => info!(
                "Connection {} addr {:?} failed: {}",
                connection_entity, address, error
            ),
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct PingMessage {
    pub message: String,
}
