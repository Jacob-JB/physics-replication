use bevy::prelude::*;
use common::networking::{StreamHeader, add_protocol};
use nevy::*;

pub fn build(app: &mut App) {
    app.add_plugins((
        NevyPlugin::default(),
        NevyHeaderPlugin::default(),
        NevyMessagesPlugin::default(),
    ));

    app.insert_resource(MessageStreamHeader::new(StreamHeader::Messages));

    add_protocol(app);

    app.add_systems(Startup, spawn_endpoint);
    app.add_systems(Update, common::networking::log_connection_status);
}

#[derive(Component)]
pub struct ServerEndpoint;

fn spawn_endpoint(mut commands: Commands) -> Result {
    commands.spawn((
        ServerEndpoint,
        EndpointWithHeaderedConnections,
        EndpointWithMessageConnections,
        QuicEndpoint::new(
            "0.0.0.0:27518",
            quinn_proto::EndpointConfig::default(),
            Some(create_server_endpoint_config()),
            AlwaysAcceptIncoming::new(),
        )?,
    ));

    Ok(())
}

fn create_server_endpoint_config() -> nevy::quinn_proto::ServerConfig {
    let cert = rcgen::generate_simple_self_signed(vec!["dev.nevy".to_string()]).unwrap();
    let key = rustls::pki_types::PrivateKeyDer::try_from(cert.key_pair.serialize_der()).unwrap();
    let chain = cert.cert.der().clone();

    let mut tls_config = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(vec![chain], key)
    .unwrap();

    tls_config.max_early_data_size = u32::MAX;
    tls_config.alpn_protocols = vec![b"h3".to_vec()]; // this one is important

    let quic_tls_config =
        nevy::quinn_proto::crypto::rustls::QuicServerConfig::try_from(tls_config).unwrap();

    let mut server_config =
        nevy::quinn_proto::ServerConfig::with_crypto(std::sync::Arc::new(quic_tls_config));

    let mut transport_config = nevy::quinn_proto::TransportConfig::default();
    transport_config.max_idle_timeout(Some(std::time::Duration::from_secs(10).try_into().unwrap()));
    transport_config.keep_alive_interval(Some(std::time::Duration::from_millis(200)));

    server_config.transport = std::sync::Arc::new(transport_config);

    server_config
}
