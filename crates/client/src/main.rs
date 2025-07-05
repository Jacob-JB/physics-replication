use bevy::prelude::*;

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
