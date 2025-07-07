use std::{
    collections::{HashMap, VecDeque},
    marker::PhantomData,
};

use bevy::{ecs::component::Mutable, prelude::*};
use nevy::*;
use serde::{Serialize, de::DeserializeOwned};

use crate::networking::{
    NetworkingSet, StreamHeader,
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
        T: Serialize + DeserializeOwned,
        MessageId<T>: Resource,
        ReceivedMessages<T>: Component<Mutability = Mutable>,
    {
        let mut next_message_id = self.world_mut().get_resource_or_init::<NextMessageId>();

        let message_id = next_message_id.0;
        next_message_id.0 += 1;

        self.insert_resource(MessageId::<T> {
            _p: PhantomData,
            id: message_id,
        });

        self.add_systems(
            PostUpdate,
            (
                insert_received_message_buffers::<T>.in_set(NetworkingSet::InsertComponents),
                deserialize_messages::<T>.in_set(NetworkingSet::DeserializeMessages),
            ),
        );
    }
}

#[derive(Resource)]
pub struct MessageId<T> {
    _p: PhantomData<T>,
    id: u16,
}

impl<T> Clone for MessageId<T> {
    fn clone(&self) -> Self {
        Self {
            _p: PhantomData,
            id: self.id,
        }
    }
}

impl<T> Copy for MessageId<T> {}

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
        buffer: Vec<u8>,
    },
}

/// Component that exists on all connections and holds the received messages of a certain kind.
#[derive(Component)]
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

pub(crate) fn insert_received_message_buffers<T>(
    mut commands: Commands,
    connection_q: Query<Entity, Added<ConnectionOf>>,
) where
    ReceivedMessages<T>: Component,
{
    for connection_entity in &connection_q {
        commands
            .entity(connection_entity)
            .insert(ReceivedMessages::<T> {
                messages: VecDeque::new(),
            });
    }
}

pub(crate) fn take_message_streams(
    mut connection_q: Query<(Entity, &mut RecvStreamHeaders, &mut MessageRecvBuffers)>,
) {
    for (connection_entity, mut headers, mut buffers) in &mut connection_q {
        while let Some((stream_id, dir)) = headers.take_stream(StreamHeader::Messages) {
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
    mut connection_q: Query<(
        Entity,
        &ConnectionOf,
        &QuicConnection,
        &mut MessageRecvBuffers,
    )>,
    mut endpoint_q: Query<&mut QuicEndpoint>,
) -> Result {
    for (connection_entity, connection_of, connection, mut buffers) in &mut connection_q {
        let buffers = buffers.as_mut();

        let mut endpoint = endpoint_q.get_mut(**connection_of)?;

        let connection = endpoint.get_connection(connection)?;

        let mut finished = Vec::new();

        for (&stream_id, buffer_state) in buffers.streams.iter_mut() {
            loop {
                let bytes_needed = match buffer_state {
                    MessageRecvBufferState::ReadingId { buffer } => buffer.bytes_needed(),
                    MessageRecvBufferState::ReadingLength { buffer, .. } => buffer.bytes_needed(),
                    MessageRecvBufferState::ReadingMessage { buffer, length, .. } => {
                        *length as usize - buffer.len()
                    }
                };

                let chunk = match connection.read_recv_stream(stream_id, bytes_needed, true) {
                    Ok(Some(chunk)) => chunk,
                    Ok(None) => {
                        finished.push(stream_id);

                        break;
                    }
                    Err(StreamReadError::Blocked) => break,
                    Err(StreamReadError::Reset(code)) => {
                        warn!(
                            "A stream on connection {} was reset with code {} before sending a header",
                            connection_entity, code,
                        );

                        finished.push(stream_id);

                        break;
                    }
                    Err(err) => {
                        return Err(err.into());
                    }
                };

                match buffer_state {
                    MessageRecvBufferState::ReadingId { buffer } => {
                        buffer.write(&chunk.data);

                        let Some(message_id) = buffer.finish() else {
                            continue;
                        };

                        *buffer_state = MessageRecvBufferState::ReadingLength {
                            message_id,
                            buffer: U16Reader::new(),
                        };
                    }
                    MessageRecvBufferState::ReadingLength { message_id, buffer } => {
                        buffer.write(&chunk.data);

                        let Some(length) = buffer.finish() else {
                            continue;
                        };

                        *buffer_state = MessageRecvBufferState::ReadingMessage {
                            message_id: *message_id,
                            length,
                            buffer: Vec::new(),
                        };
                    }
                    MessageRecvBufferState::ReadingMessage {
                        message_id,
                        length,
                        buffer,
                    } => {
                        buffer.extend(chunk.data);

                        if buffer.len() != *length as usize {
                            continue;
                        }

                        let message = std::mem::take(buffer);

                        buffers
                            .messages
                            .entry(*message_id)
                            .or_default()
                            .push_back(message.into_boxed_slice());
                    }
                }
            }
        }

        for stream_id in finished {
            buffers.streams.remove(&stream_id);
        }
    }

    Ok(())
}

pub(crate) fn deserialize_messages<T>(
    message_id: Res<MessageId<T>>,
    mut connection_q: Query<(&mut MessageRecvBuffers, &mut ReceivedMessages<T>)>,
) where
    T: DeserializeOwned,
    MessageId<T>: Resource,
    ReceivedMessages<T>: Component<Mutability = Mutable>,
{
    for (mut serialized_buffer, mut deserialized_buffer) in connection_q.iter_mut() {
        let Some(buffer) = serialized_buffer.messages.get_mut(&message_id.id) else {
            continue;
        };

        for bytes in buffer.drain(..) {
            match bincode::serde::decode_from_slice(&bytes, bincode_config()) {
                Ok((message, _)) => {
                    deserialized_buffer.messages.push_back(message);
                }
                Err(error) => {
                    error!(
                        "Failed to deserialize \"{}\" message: {}",
                        std::any::type_name::<T>(),
                        error
                    );
                }
            }
        }
    }
}

impl<T> ReceivedMessages<T> {
    pub fn drain(&mut self) -> impl Iterator<Item = T> {
        self.messages.drain(..)
    }

    pub fn next(&mut self) -> Option<T> {
        self.messages.pop_front()
    }
}

/// State machine for a stream that sends messages.
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
    pub fn write<T>(
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
        let message_data = match bincode::serde::encode_to_vec(message, bincode_config()) {
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

fn bincode_config() -> bincode::config::Configuration {
    bincode::config::standard()
}
