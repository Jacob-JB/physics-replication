use std::{collections::HashMap, marker::PhantomData};

use bevy::prelude::*;
use nevy::*;
use serde::{Serialize, de::DeserializeOwned};

#[derive(Resource, Default)]
struct NextMessageId(u16);

pub trait AddMessage {
    fn add_message<T>(&mut self)
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'static;
}

impl AddMessage for App {
    fn add_message<T>(&mut self)
    where
        T: Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        let mut next_message_id = self.world_mut().get_resource_or_init::<NextMessageId>();

        let message_id = next_message_id.0;
        next_message_id.0 += 1;

        self.insert_resource(MessageId::<T> {
            _p: PhantomData,
            id: message_id,
        });
    }
}

#[derive(Resource)]
struct MessageId<T> {
    _p: PhantomData<T>,
    id: u16,
}

#[derive(Component)]
struct MessageStreams {
    streams: HashMap<StreamId, MessageStreamState>,
    messages: Vec<(u16, Box<[u8]>)>,
}

enum MessageStreamState {
    ReadingId {
        buffer: Vec<u8>,
    },
    ReadingLength {
        message_id: u16,
        buffer: Vec<u8>,
    },
    ReadingMessage {
        message_id: u16,
        length: u16,
        buffer: Vec<u8>,
    },
}
