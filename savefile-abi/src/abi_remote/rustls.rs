use std::fs::File;
use std::io::{BufReader, Error, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use rustls::{ClientConfig, RootCertStore, ServerConfig, ServerConnection, StreamOwned};
use rustls::pki_types::ServerName;
use savefile::{Deserializer, SavefileError, Serializer};
use crate::abi_remote::{ClientConnection, ConnectionStrategy, Server};
use crate::serialize_helpers::SerializerAndDeserializer;


/// A strategy, usable with ServerBuilder::finish, for enabling
/// TLS on the server end.
/// If this is used for the server, TlsClientStrategy must be used
/// for the client.
pub struct TlsServerStrategy {
    config: Arc<ServerConfig>,
}

/// A strategy, usable with ClientConnection, for enabling
/// TLS on the client end.
/// If this is used for the client, TlsServerStrategy must be used
/// for the server.
pub struct TlsClientStrategy {
    config: Arc<ClientConfig>,
    server_name: String,
}

impl TlsClientStrategy {
    fn construct_config(server_pem_cert_file: &str, server_name: &str) -> Result<TlsClientStrategy, Box<dyn std::error::Error>> {

        let certs = rustls_pemfile::certs(&mut BufReader::new(&mut File::open(server_pem_cert_file)?))
            .collect::<Result<Vec<_>, _>>()?;
        let mut store : RootCertStore = RootCertStore::empty();

        for cert in certs {
            store.add(cert)?;
        }

        Ok(TlsClientStrategy {
            server_name: server_name.to_string(),
            config: Arc::new(ClientConfig::builder().with_root_certificates(store).with_no_client_auth())
        })
    }
    /// Configure TLS. The given certificate file must contain the certificate used by the server.
    /// The server certificate must be valid for the given name.
    pub fn new(server_pem_cert_file: &str, server_name: &str) -> Result<TlsClientStrategy, SavefileError> {
        Ok(Self::construct_config(server_pem_cert_file, server_name)?)
    }
    fn connect(&mut self, stream: TcpStream) -> Result<StreamOwned<rustls::ClientConnection, TcpStream>, Box<dyn std::error::Error>> {
        let server_name:ServerName<'_> = self.server_name.as_str().try_into()?;
        let conn = rustls::ClientConnection::new(Arc::clone(&self.config), server_name.to_owned())?;
        Ok(StreamOwned::new(conn, stream))
    }
}

impl SerializerAndDeserializer for StreamOwned<rustls::ServerConnection, TcpStream> {
    type TR = StreamOwned<rustls::ServerConnection, TcpStream>;
    type TW = StreamOwned<rustls::ServerConnection, TcpStream>;

    fn get_serializer(&mut self) -> Serializer<Self::TW> {
        Serializer {
            writer: self,
            file_version: 0,
        }
    }

    fn get_deserializer(&mut self) -> Deserializer<Self::TR> {
        Deserializer {
            reader: self,
            file_version: 0,
            ephemeral_state: Default::default(),
        }
    }

    fn raw_writer(&mut self) -> &mut Self::TW {
        self
    }

    fn raw_reader(&mut self) -> &mut Self::TR {
        self
    }

    fn notify_close(&mut self) {
        println!("Sending close_notify");
        self.conn.send_close_notify()
    }
}

impl SerializerAndDeserializer for StreamOwned<rustls::ClientConnection, TcpStream> {
    type TR = StreamOwned<rustls::ClientConnection, TcpStream>;
    type TW = StreamOwned<rustls::ClientConnection, TcpStream>;

    fn get_serializer(&mut self) -> Serializer<Self::TW> {
        Serializer {
            writer: self,
            file_version: 0,
        }
    }

    fn get_deserializer(&mut self) -> Deserializer<Self::TR> {
        Deserializer {
            reader: self,
            file_version: 0,
            ephemeral_state: Default::default()
        }
    }

    fn raw_writer(&mut self) -> &mut Self::TW {
        self
    }

    fn raw_reader(&mut self) -> &mut Self::TR {
        self
    }

    fn notify_close(&mut self) {
        println!("Sending close_notify");
        self.conn.send_close_notify()
    }
}

impl ConnectionStrategy for TlsClientStrategy {
    type TS = StreamOwned<rustls::ClientConnection, TcpStream>;

    fn create(&mut self, stream: TcpStream) -> Result<Self::TS, SavefileError> {
        Ok(self.connect(stream)?)
    }
}

impl TlsServerStrategy {
    fn construct_config(pem_cert_file: &str, pem_privkey: &str) -> Result<ServerConfig, Box<dyn std::error::Error>> {
        let mut certfile = match File::open(pem_cert_file) {
            Ok(f) => f,
            Err(err) => {
                return Err(format!("Failed to open file '{}' (current dir: {}), because: {}",
                    pem_cert_file, std::env::current_dir().map(|x|x.to_string_lossy().to_string()).unwrap_or("?".to_string()), err).into());
            }
        };
        let certs = rustls_pemfile::certs(&mut BufReader::new(&mut certfile))
            .collect::<Result<Vec<_>, _>>()?;
        let private_key =
            rustls_pemfile::private_key(&mut BufReader::new(&mut File::open(pem_privkey)?))?
                .unwrap();
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, private_key)?;
        Ok(config)
    }
    /// Create a new TLS configuration.
    /// The cert file must contain a cert usable for the address the server is hosted on.
    /// The privkey must contain the private key for that cert.
    pub fn new(pem_cert_file: &str, pem_privkey: &str) -> Result<TlsServerStrategy, SavefileError> {
        let config = Self::construct_config(pem_cert_file, pem_privkey)?;

        Ok(TlsServerStrategy{
            config: Arc::new(config),
        })
    }
    fn connect(&mut self, mut stream: TcpStream) -> Result<StreamOwned<ServerConnection, TcpStream>, Box<dyn std::error::Error>> {
        let mut conn = rustls::ServerConnection::new(Arc::clone(&self.config))?;
        conn.complete_io(&mut stream)?;
        let mut owned = StreamOwned::new(conn, stream);
        Ok(owned)
    }
}

impl ConnectionStrategy for TlsServerStrategy {
    type TS = StreamOwned<ServerConnection, TcpStream>;

    fn create(&mut self, mut stream: TcpStream) -> Result<Self::TS, SavefileError> {
       Ok(self.connect(stream)?)
    }
}
