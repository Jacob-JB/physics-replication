use bevy::prelude::*;
use nevy::*;

pub mod messages;
pub mod stream_headers;
pub mod u16_reader;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(NevyPlugin::default());

        app.add_systems(
            PostUpdate,
            (
                stream_headers::insert_stream_header_buffers,
                stream_headers::read_stream_headers,
                messages::take_message_streams,
            )
                .after(UpdateEndpoints),
        );

        app.add_systems(Update, log_connection_status);
    }
}

pub enum StreamHeader {
    Messages,
}

impl From<StreamHeader> for u16 {
    fn from(value: StreamHeader) -> Self {
        value as u16
    }
}

fn log_connection_status(
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
