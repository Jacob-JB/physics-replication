use std::time::Duration;

use bevy::prelude::*;
use common::networking::messages::MessageSendStreamState;
use nevy::*;

pub mod networking;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins);

    networking::build(&mut app);

    app.add_systems(PostStartup, debug_connect_to_server);

    app.run();
}

fn debug_connect_to_server(
    mut commands: Commands,
    endpoint_q: Query<Entity, With<networking::ClientEndpoint>>,
) -> Result {
    let endpoint_entity = endpoint_q.single()?;

    commands.spawn((
        nevy::ConnectionOf(endpoint_entity),
        nevy::QuicConnectionConfig {
            client_config: networking::create_connection_config(),
            address: "127.0.0.1:27518".parse().unwrap(),
            server_name: "example.server".to_string(),
        },
    ));

    Ok(())
}

fn debug_send_ping(
    mut commands: Commands,
    connection_q: Query<(&ConnectionOf, &QuicConnection, &ConnectionStatus)>,
    mut endpoint_q: Query<&mut QuicEndpoint>,
    time: Res<Time>,
    mut last_ping: Local<Duration>,
) -> Result {
    if time.elapsed() - *last_ping < Duration::from_millis(1000) {
        return Ok(());
    }

    *last_ping = time.elapsed();

    for (connection_of, connection, status) in &connection_q {
        let ConnectionStatus::Established = status else {
            continue;
        };

        let mut endpoint = endpoint_q.get_mut(**connection_of)?;

        let connection = endpoint.get_connection(connection)?;

        let stream_id = connection.open_stream(Dir::Uni)?;

        let mut stream_state = MessageSendStreamState::new(stream_id);
    }

    Ok(())
}
