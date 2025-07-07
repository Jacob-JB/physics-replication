use bevy::prelude::*;
use common::{PingMessage, networking::messages::ReceivedMessages};

pub mod networking;

fn main() {
    let mut app = App::new();

    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::log::LogPlugin {
        level: bevy::log::Level::DEBUG,
        ..default()
    });

    networking::build(&mut app);

    app.add_systems(Update, debug_recv_ping);

    app.run();
}

fn debug_recv_ping(mut connection_q: Query<(Entity, &mut ReceivedMessages<PingMessage>)>) {
    for (connection_entity, mut messages) in connection_q.iter_mut() {
        for PingMessage { message } in messages.drain() {
            info!(
                "Received ping from {} with message: {}",
                connection_entity, message
            );
        }
    }
}
