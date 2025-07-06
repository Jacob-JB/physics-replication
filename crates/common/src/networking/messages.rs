use std::{
    collections::{HashMap, VecDeque},
    marker::PhantomData,
};

use bevy::{ecs::component::Mutable, prelude::*};
use nevy::*;
use serde::{Serialize, de::DeserializeOwned};

use crate::networking::{
    StreamHeader,
    stream_headers::{HeaderedStreamState, RecvStreamHeaders},
    u16_reader::U16Reader,
};

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

#[derive(Resource, Clone, Copy)]
pub struct MessageId<T> {
    _p: PhantomData<T>,
    id: u16,
}

#[derive(Component, Default)]
pub(crate) struct MessageRecvBuffers {
    streams: HashMap<StreamId, MessageRecvBufferState>,
    messages: HashMap<u16, VecDeque<Box<[u8]>>>,
}

enum MessageRecvBufferState {
    ReadingId {
        buffer: U16Reader,
    },
    ReadingLength {
        message_id: u16,
        buffer: U16Reader,
    },
    ReadingMessage {
        message_id: u16,
        length: u16,
        buffer: U16Reader,
    },
}

#[derive(Component, Default)]
pub struct ReceivedMessages<T> {
    messages: VecDeque<T>,
}

pub(crate) fn insert_recv_stream_buffers(
    mut commands: Commands,
    connection_q: Query<Entity, Added<ConnectionOf>>,
) {
    for connection_entity in &connection_q {
        commands
            .entity(connection_entity)
            .insert(MessageRecvBuffers::default());
    }
}

pub(crate) fn take_message_streams(
    mut connection_q: Query<(Entity, &mut RecvStreamHeaders, &mut MessageRecvBuffers)>,
) {
    for (connection_entity, mut headers, mut buffers) in &mut connection_q {
        while let Some((stream_id, dir)) = headers.take_stream(StreamHeader::Messages) {
            debug!("Took message stream");

            if let Dir::Bi = dir {
                warn!(
                    "Connection {} opened a bidirectional message stream which should only ever be unidirectional.",
                    connection_entity
                );
            }

            buffers.streams.insert(
                stream_id,
                MessageRecvBufferState::ReadingId {
                    buffer: U16Reader::new(),
                },
            );
        }
    }
}

pub(crate) fn read_message_streams(
    mut connection_q: Query<(&ConnectionOf, &QuicConnection, &mut MessageRecvBuffers)>,
    mut endpoint_q: Query<&mut QuicEndpoint>,
) -> Result {
    for (connection_of, connection, mut buffers) in &mut connection_q {
        let mut endpoint = endpoint_q.get_mut(**connection_of)?;

        let connection = endpoint.get_connection(connection)?;

        for (&stream_id, buffer_state) in buffers.streams.iter_mut() {
            match buffer_state {
                MessageRecvBufferState::ReadingId { buffer } => todo!(),
                MessageRecvBufferState::ReadingLength { message_id, buffer } => todo!(),
                MessageRecvBufferState::ReadingMessage {
                    message_id,
                    length,
                    buffer,
                } => todo!(),
            }
        }
    }

    Ok(())
}

pub(crate) fn deserialize_messages<T>(
    message_id: Res<MessageId<T>>,
    mut connection_q: Query<(&mut MessageRecvBuffers, &mut ReceivedMessages<T>)>,
) where
    MessageId<T>: Resource,
    ReceivedMessages<T>: Component<Mutability = Mutable>,
{
}

pub struct MessageSendStreamState {
    stream: HeaderedStreamState,
    buffer: VecDeque<u8>,
}

impl MessageSendStreamState {
    pub fn new(stream_id: StreamId) -> Self {
        Self {
            stream: HeaderedStreamState::new(stream_id, StreamHeader::Messages),
            buffer: VecDeque::new(),
        }
    }

    /// Gets the stream id.
    pub fn stream_id(&self) -> StreamId {
        self.stream.stream_id()
    }

    /// Returns true if the internal buffer has been entirely written to the connection.
    pub fn uncongested(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Writes as much of the internal buffer as possible to the connection.
    pub fn flush(&mut self, connection: &mut ConnectionState) -> Result<(), StreamWriteError> {
        let written_bytes = self
            .stream
            .write(connection, self.buffer.make_contiguous())?;

        self.buffer.drain(..written_bytes);

        Ok(())
    }

    /// Attempts to send a message
    ///
    /// If `queue` is true the message will always be written and `Ok(true)` will be returned.
    /// This will cause the internal buffer to grow without limit if the stream is congested.
    /// See `Self::uncongested`.
    ///
    /// If `queue` is false and the stream is congested the message will not be written and `Ok(false)` will be returned.
    pub fn send<T>(
        &mut self,
        message_id: MessageId<T>,
        connection: &mut ConnectionState,
        message: &T,
        queue: bool,
    ) -> Result<bool, StreamWriteError>
    where
        T: Serialize,
    {
        // only attempt to write data if queueing or uncongested
        if !(queue || self.uncongested()) {
            return Ok(false);
        }

        // serialize
        let message_data = match bincode::serde::encode_to_vec(message, bincode::config::standard())
        {
            Ok(data) => data,
            Err(err) => panic!("Failed to serialize message: {}", err),
        };

        // write the message id
        self.buffer.extend(message_id.id.to_be_bytes());

        // write the message length
        let message_length: u16 = message_data.len().try_into().expect("Message was too long");
        self.buffer.extend(message_length.to_be_bytes());

        // write the message
        self.buffer.extend(message_data);

        self.flush(connection)?;

        Ok(true)
    }
}
