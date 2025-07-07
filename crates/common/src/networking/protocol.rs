use crate::networking::messages::AddMessage;
use bevy::prelude::*;

pub fn build_protocol(app: &mut App) {
    app.add_message::<crate::PingMessage>();
}
