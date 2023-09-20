use std::prelude::v1::*;

use std::io::{self, Error, ErrorKind, Read, Result};
use std::net::{Shutdown, SocketAddr, SocketAddrV4, SocketAddrV6, TcpStream};

use ppp::{
    v1::Addresses as V1Addr, v1::PROTOCOL_PREFIX as V1ProtocolPrefix, v2::Addresses as V2Addr,
    v2::ParseError, v2::PROTOCOL_PREFIX as v2ProtocolPrefix, HeaderResult, PartialResult,
};

use crate::StreamTrait;

pub struct ProxyTcpStream {
    pub stream: TcpStream,
    pub ty: ProxyStreamType,

    pub proxy_protocol_v1_address: V1Addr,
    pub proxy_protocol_v2_address: V2Addr,
}

pub enum ProxyStreamType {
    Unknown,
    ProxyV1,
    ProxyV2,
    Normal,
}

impl ProxyTcpStream {
    pub fn new(s: TcpStream) -> Self {
        Self {
            stream: s,
            ty: ProxyStreamType::Unknown,
            proxy_protocol_v1_address: V1Addr::Unknown,
            proxy_protocol_v2_address: V2Addr::Unspecified,
        }
    }

    pub fn set_nonblocking(&self, is_nonblocking: bool) -> io::Result<()> {
        self.stream.set_nonblocking(is_nonblocking)
    }

    fn unwrap(&mut self) -> io::Result<()> {
        let v1_prefix = V1ProtocolPrefix.as_bytes();
        match self.ty {
            ProxyStreamType::Unknown => {
                let mut buffer = [0; 512];
                let header = {
                    match self.stream.peek(&mut buffer) {
                        Ok(len) => {
                            // check v1 prefix
                            let mut is_v1_prefix = true;
                            if len < v1_prefix.len() {
                                if !v1_prefix.starts_with(&buffer[..len]) {
                                    is_v1_prefix = false;
                                }
                            } else {
                                if &buffer[..v1_prefix.len()] != v1_prefix {
                                    is_v1_prefix = false;
                                }
                            }
                            // check v2 prefix
                            let mut is_v2_prefix = true;
                            if len < v2ProtocolPrefix.len() {
                                if !v2ProtocolPrefix.starts_with(&buffer[..len]) {
                                    is_v2_prefix = false;
                                }
                            } else {
                                if &buffer[..v2ProtocolPrefix.len()] != v2ProtocolPrefix {
                                    is_v2_prefix = false;
                                }
                            }
                            if !is_v1_prefix && !is_v2_prefix {
                                // fallback to normal stream
                                HeaderResult::V2(Err(ParseError::Prefix))
                            } else {
                                let header = HeaderResult::parse(&buffer[..len]);
                                if header.is_complete() {
                                    header
                                } else {
                                    if is_v1_prefix && len > 108 {
                                        HeaderResult::V2(Err(ParseError::Prefix))
                                    } else if is_v2_prefix && len > 16 + 216 {
                                        HeaderResult::V2(Err(ParseError::Prefix))
                                    } else {
                                        return Err(Error::new(ErrorKind::WouldBlock, ""));
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            return Err(err);
                        }
                    }
                };
                match header {
                    HeaderResult::V1(Ok(hdr)) => {
                        let mut read_buffer = [0; 512];
                        self.stream.read(&mut read_buffer[..hdr.header.len()])?;
                        self.ty = ProxyStreamType::ProxyV1;
                        self.proxy_protocol_v1_address = hdr.addresses;
                    }
                    HeaderResult::V2(Ok(hdr)) => {
                        let mut read_buffer = [0; 512];
                        self.stream.read(&mut read_buffer[..hdr.header.len()])?;
                        self.ty = ProxyStreamType::ProxyV2;
                        self.proxy_protocol_v2_address = hdr.addresses;
                    }
                    HeaderResult::V1(Err(_)) => {
                        self.ty = ProxyStreamType::Normal;
                    }
                    HeaderResult::V2(Err(_)) => {
                        self.ty = ProxyStreamType::Normal;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match self.ty {
            ProxyStreamType::Unknown => {
                Err(Error::new(ErrorKind::InvalidInput, "Need to unwrap first"))
            }
            ProxyStreamType::ProxyV1 => match self.proxy_protocol_v1_address {
                V1Addr::Tcp4(ipv4) => {
                    Ok(SocketAddrV4::new(ipv4.source_address, ipv4.source_port).into())
                }
                V1Addr::Tcp6(ipv6) => {
                    Ok(SocketAddrV6::new(ipv6.source_address, ipv6.source_port, 0, 0).into())
                }
                _ => Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Invalid v1 address type",
                )),
            },
            ProxyStreamType::ProxyV2 => match self.proxy_protocol_v2_address {
                V2Addr::IPv4(ipv4) => Ok(SocketAddr::V4(SocketAddrV4::new(
                    ipv4.source_address,
                    ipv4.source_port,
                ))),
                V2Addr::IPv6(ipv6) => Ok(SocketAddr::V6(SocketAddrV6::new(
                    ipv6.source_address,
                    ipv6.source_port,
                    0,
                    0,
                ))),
                _ => Err(Error::new(
                    ErrorKind::InvalidInput,
                    "Invalid v2 address type",
                )),
            },
            ProxyStreamType::Normal => self.stream.peer_addr(),
        }
    }
}

impl std::io::Read for ProxyTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.ty {
            ProxyStreamType::Unknown => match self.unwrap() {
                Ok(()) => {}
                Err(err) => return Err(err),
            },
            _ => {}
        }
        self.stream.read(buf)
    }
}

impl std::io::Write for ProxyTcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl StreamTrait for ProxyTcpStream {
    fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        ProxyTcpStream::set_nonblocking(self, nonblocking)
    }
    fn peer_addr(&self) -> Result<SocketAddr> {
        ProxyTcpStream::peer_addr(&self)
    }
    fn shutdown(&self, how: Shutdown) -> Result<()> {
        self.stream.shutdown(how)
    }
    fn set_read_timeout(&self, dur: Option<core::time::Duration>) -> Result<()> {
        self.stream.set_read_timeout(dur)
    }
}
