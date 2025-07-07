use bevy::{platform::collections::HashMap, prelude::*};
use nevy::*;

use crate::networking::u16_reader::U16Reader;

#[derive(Component, Default)]
pub struct RecvStreamHeaders {
    buffers: HashMap<StreamId, RecvStreamHeaderState>,
}

enum RecvStreamHeaderState {
    Reading { dir: Dir, buffer: U16Reader },
    HeaderReceived { dir: Dir, header: u16 },
}

pub(crate) fn insert_stream_header_buffers(
    mut commands: Commands,
    connection_q: Query<Entity, Added<ConnectionOf>>,
) {
    for connection_entity in &connection_q {
        commands
            .entity(connection_entity)
            .insert(RecvStreamHeaders::default());
    }
}

pub(crate) fn read_stream_headers(
    mut connection_q: Query<(
        Entity,
        &ConnectionOf,
        &QuicConnection,
        &mut RecvStreamHeaders,
    )>,
    mut endpoint_q: Query<&mut QuicEndpoint>,
) -> Result {
    for (connection_entity, connection_of, quic_connection, mut buffers) in &mut connection_q {
        let mut endpoint = endpoint_q.get_mut(**connection_of)?;

        let connection = endpoint.get_connection(quic_connection)?;

        for dir in [Dir::Uni, Dir::Bi] {
            while let Some(stream_id) = connection.accept_stream(dir) {
                buffers.buffers.insert(
                    stream_id,
                    RecvStreamHeaderState::Reading {
                        dir,
                        buffer: U16Reader::new(),
                    },
                );
            }
        }

        let mut finished_streams = Vec::new();

        for (&stream_id, state) in buffers.buffers.iter_mut() {
            let RecvStreamHeaderState::Reading { dir, buffer } = state else {
                continue;
            };

            loop {
                match connection.read_recv_stream(stream_id, buffer.bytes_needed(), true) {
                    Ok(Some(chunk)) => {
                        buffer.write(&chunk.data);

                        let Some(header) = buffer.finish() else {
                            continue;
                        };

                        let dir = *dir;

                        *state = RecvStreamHeaderState::HeaderReceived { dir, header };

                        break;
                    }
                    Ok(None) => {
                        warn!(
                            "A stream on connection {} finished a stream before sending a header",
                            connection_entity
                        );

                        finished_streams.push(stream_id);
                    }
                    Err(StreamReadError::Blocked) => break,
                    Err(StreamReadError::Reset(code)) => {
                        warn!(
                            "A stream on connection {} was reset with code {} before sending a header",
                            connection_entity, code,
                        );

                        finished_streams.push(stream_id);
                    }
                    Err(err) => {
                        return Err(err.into());
                    }
                }
            }
        }

        for stream_id in finished_streams {
            buffers.buffers.remove(&stream_id);
        }
    }

    Ok(())
}

impl RecvStreamHeaders {
    pub fn take_stream(&mut self, header: impl Into<u16>) -> Option<(StreamId, Dir)> {
        let target_header = header.into();

        if let Some((stream_id, dir)) = self.buffers.iter().find_map(|(&stream_id, state)| {
            let RecvStreamHeaderState::HeaderReceived { dir, header } = state else {
                return None;
            };

            if *header != target_header {
                return None;
            }

            Some((stream_id, *dir))
        }) {
            self.buffers.remove(&stream_id);
            Some((stream_id, dir))
        } else {
            None
        }
    }
}

/// Used for writing a header to a stream before writing data.
pub struct HeaderedStreamState {
    stream_id: StreamId,
    header_buffer: Option<Vec<u8>>,
}

impl HeaderedStreamState {
    pub fn new(stream_id: StreamId, header: impl Into<u16>) -> Self {
        Self {
            stream_id,
            header_buffer: Some(header.into().to_be_bytes().into()),
        }
    }

    /// Gets the stream id.
    pub fn stream_id(&self) -> StreamId {
        self.stream_id
    }

    /// Attempts to write some data to the stream.
    ///
    /// Will only write data once the header has been written.
    pub fn write(
        &mut self,
        connection: &mut ConnectionState,
        data: &[u8],
    ) -> Result<usize, StreamWriteError> {
        if let Some(buffer) = &mut self.header_buffer {
            let bytes_written = connection.write_send_stream(self.stream_id, buffer)?;

            buffer.drain(..bytes_written);

            if buffer.is_empty() {
                self.header_buffer = None;
            } else {
                return Ok(0);
            }
        }

        connection.write_send_stream(self.stream_id, data)
    }
}
