use bevy::prelude::*;
use nevy::*;

pub mod messages;
pub mod stream_headers;

pub struct NetworkingPlugin;

impl Plugin for NetworkingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(NevyPlugin::default());

        app.add_systems(
            PostUpdate,
            (
                stream_headers::insert_stream_header_buffers,
                stream_headers::read_stream_headers,
            )
                .after(UpdateEndpoints),
        );

        app.add_systems(Update, log_connections);
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

fn log_connections(
    connection_q: Query<(Entity, &ConnectionOf, &QuicConnection), Added<ConnectionOf>>,
    mut endpoint_q: Query<&mut QuicEndpoint>,
) -> Result {
    for (connection_entity, connection_of, connection) in &connection_q {
        let mut endpoint = endpoint_q.get_mut(**connection_of)?;

        let connection = endpoint.get_connection(connection)?;

        let address = connection.get_remote_address();

        info!("New connection {} {}", connection_entity, address);
    }

    Ok(())
}
