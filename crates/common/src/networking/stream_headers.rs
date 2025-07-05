use bevy::{platform::collections::HashMap, prelude::*};
use nevy::*;

#[derive(Component, Default)]
pub struct StreamHeaders {
    buffers: HashMap<StreamId, StreamHeaderState>,
}

enum StreamHeaderState {
    Reading { dir: Dir, buffer: Vec<u8> },
    HeaderReceived { dir: Dir, header: u16 },
}

pub(crate) fn insert_stream_header_buffers(
    mut commands: Commands,
    connection_q: Query<Entity, Added<ConnectionOf>>,
) {
    for connection_entity in &connection_q {
        commands
            .entity(connection_entity)
            .insert(StreamHeaders::default());
    }
}

pub(crate) fn read_stream_headers(
    mut connection_q: Query<(Entity, &ConnectionOf, &QuicConnection, &mut StreamHeaders)>,
    mut endpoint_q: Query<&mut QuicEndpoint>,
) -> Result {
    for (connection_entity, connection_of, quic_connection, mut buffers) in &mut connection_q {
        let mut endpoint = endpoint_q.get_mut(**connection_of)?;

        let connection = endpoint.get_connection(quic_connection)?;

        for dir in [Dir::Uni, Dir::Bi] {
            while let Some(stream_id) = connection.accept_stream(dir) {
                buffers.buffers.insert(
                    stream_id,
                    StreamHeaderState::Reading {
                        dir,
                        buffer: Vec::new(),
                    },
                );
            }
        }

        let mut failed_streams = Vec::new();

        for (&stream_id, state) in buffers.buffers.iter_mut() {
            let StreamHeaderState::Reading { dir, buffer } = state else {
                continue;
            };

            loop {
                match connection.read_recv_stream(stream_id, 2 - buffer.len(), true) {
                    Ok(Some(chunk)) => {
                        buffer.extend(chunk.data);

                        debug_assert!(buffer.len() <= 2, "should never read more than two bytes");

                        if buffer.len() != 2 {
                            continue;
                        }

                        let Ok(&buffer) = buffer.as_slice().try_into() else {
                            continue;
                        };

                        let header = u16::from_be_bytes(buffer);
                        let dir = *dir;

                        *state = StreamHeaderState::HeaderReceived { dir, header };

                        break;
                    }
                    Ok(None) => {
                        warn!(
                            "A stream on connection {} finished a stream before sending a header",
                            connection_entity
                        );

                        failed_streams.push(stream_id);
                    }
                    Err(StreamReadError::Blocked) => break,
                    Err(StreamReadError::Reset(code)) => {
                        warn!(
                            "A stream on connection {} was reset with code {} before sending a header",
                            connection_entity, code,
                        );

                        failed_streams.push(stream_id);
                    }
                    Err(err) => {
                        return Err(err.into());
                    }
                }
            }
        }
    }

    Ok(())
}

impl StreamHeaders {
    pub fn take_stream(&mut self, header: u16) -> Option<(StreamId, Dir)> {
        if let Some((stream_id, dir)) = self.buffers.iter().find_map(|(&stream_id, state)| {
            let StreamHeaderState::HeaderReceived {
                dir,
                header: compare_header,
            } = state
            else {
                return None;
            };

            if *compare_header != header {
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
