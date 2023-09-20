use core::time::Duration;
use std::prelude::v1::*;

use rustls::{ClientConfig, ClientSession, ServerConfig, ServerSession, StreamOwned};
use std::io::Result;
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::sync::Arc;

use crate::ProxyTcpStream;

pub struct AnyStream(Box<dyn StreamTrait>);

impl std::fmt::Debug for AnyStream {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AnyStream")
    }
}

pub trait StreamTrait: std::io::Read + std::io::Write + Send + Sync {
    fn set_nonblocking(&self, nonblocking: bool) -> Result<()>;
    fn peer_addr(&self) -> Result<SocketAddr>;
    fn set_read_timeout(&self, dur: Option<Duration>) -> Result<()>;

    fn shutdown(&self, how: Shutdown) -> Result<()>;
    fn shutdown_read(&self) -> Result<()> {
        self.shutdown(Shutdown::Read)
    }
    fn shutdown_both(&self) -> Result<()> {
        self.shutdown(Shutdown::Both)
    }
    fn shutdown_write(&self) -> Result<()> {
        self.shutdown(Shutdown::Write)
    }
}

impl StreamTrait for AnyStream {
    fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.0.peer_addr()
    }
    fn shutdown(&self, how: Shutdown) -> Result<()> {
        self.0.shutdown(how)
    }
    fn set_read_timeout(&self, dur: Option<Duration>) -> Result<()> {
        self.0.set_read_timeout(dur)
    }
}

impl std::io::Read for AnyStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.read(buf)
    }
}

impl std::io::Write for AnyStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> Result<()> {
        self.0.flush()
    }
}

impl AnyStream {
    pub fn new<T: StreamTrait + 'static>(t: T) -> Self {
        Self(Box::new(t))
    }
}

pub struct AnyStreamBuilder {
    pub stream: TcpStream,
    pub proxy_protocol: bool,
    pub tls_client: Option<String>,
    pub tls_server: Option<Arc<ServerConfig>>,
}

impl AnyStreamBuilder {
    pub fn build(self) -> Result<AnyStream> {
        let mut stream = match self.proxy_protocol {
            false => AnyStream::new(ProxyTcpStream::new(self.stream)),
            true => AnyStream::new(self.stream),
        };
        stream = if let Some(hostname) = self.tls_client {
            let mut config = ClientConfig::new();
            config
                .root_store
                .add_server_trust_anchors(&webpki::roots::TLS_SERVER_ROOTS);
            let config = Arc::new(config);
            let session = ClientSession::new(
                &config,
                webpki::DNSNameRef::try_from_ascii_str(hostname.as_ref()).map_err(|err| {
                    std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", err))
                })?,
            );
            let tls_stream = StreamOwned::new(session, stream);
            AnyStream(Box::new(tls_stream))
        } else if let Some(cfg) = self.tls_server {
            let tls_session = ServerSession::new(&cfg);
            let tls_stream = StreamOwned::new(tls_session, stream);
            AnyStream(Box::new(tls_stream))
        } else {
            stream
        };
        Ok(stream)
    }
}

impl StreamTrait for TcpStream {
    fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        TcpStream::set_nonblocking(self, nonblocking)
    }
    fn peer_addr(&self) -> Result<SocketAddr> {
        TcpStream::peer_addr(&self)
    }
    fn shutdown(&self, how: Shutdown) -> Result<()> {
        TcpStream::shutdown(&self, how)
    }
    fn set_read_timeout(&self, dur: Option<Duration>) -> Result<()> {
        TcpStream::set_read_timeout(&self, dur)
    }
}

impl<S: rustls::Session + Sized, T: StreamTrait> StreamTrait for StreamOwned<S, T> {
    fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        self.sock.set_nonblocking(nonblocking)
    }
    fn peer_addr(&self) -> Result<SocketAddr> {
        self.sock.peer_addr()
    }
    fn shutdown(&self, how: Shutdown) -> Result<()> {
        self.sock.shutdown(how)
    }
    fn set_read_timeout(&self, dur: Option<Duration>) -> Result<()> {
        self.sock.set_read_timeout(dur)
    }
}
