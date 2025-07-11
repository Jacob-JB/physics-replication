use std::time::Duration;

use bevy::prelude::*;
use common::networking::{PingMessage, StreamHeader};
use nevy::*;

pub mod networking;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(bevy::log::LogPlugin {
        level: bevy::log::Level::DEBUG,
        filter: bevy::log::DEFAULT_FILTER.to_string()
            + ",bevy_render=info,bevy_app=info,offset_allocator=info,bevy_asset=info,gilrs=info,bevy_winit=info",
        ..default()
    }));

    networking::build(&mut app);

    app.add_systems(PostStartup, debug_connect_to_server);
    app.add_systems(Update, debug_send_ping);

    app.run();
}

#[derive(Component)]
pub struct ClientConnection;

fn debug_connect_to_server(
    mut commands: Commands,
    endpoint_q: Query<Entity, With<networking::ClientEndpoint>>,
) -> Result {
    let endpoint_entity = endpoint_q.single()?;

    commands.spawn((
        ClientConnection,
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
    connection_q: Query<(Entity, &ConnectionStatus), With<ClientConnection>>,
    time: Res<Time>,
    mut last_ping: Local<Duration>,
    message_id: Res<MessageId<PingMessage>>,
    mut sender: MessageSender,
) -> Result {
    sender.flush()?;

    if time.elapsed() - *last_ping < Duration::from_millis(1000) {
        return Ok(());
    }

    *last_ping = time.elapsed();

    for (connection_entity, status) in &connection_q {
        let ConnectionStatus::Established = status else {
            continue;
        };

        sender.write(
            StreamHeader::Messages,
            connection_entity,
            *message_id,
            &PingMessage {
                message: "Hello Server!".into(),
            },
            true,
        )?;
    }

    Ok(())
}
