use std::prelude::v1::*;

use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::{Error, ErrorKind, Result};
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::sync::RwLock;
use std::time::{Duration, Instant};

lazy_static! {
    static ref GLOBAL_CACHE: DnsCache = DnsCache::new();
}

pub fn reslove<A>(addr: A) -> Result<SocketAddr>
where
    A: ToSocketAddrs + Debug,
{
    GLOBAL_CACHE.reslove(addr)
}

pub struct DnsCache {
    cache: RwLock<BTreeMap<String, (SocketAddr, Instant)>>,
}

impl DnsCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(BTreeMap::new()),
        }
    }

    pub fn reslove<A>(&self, addr: A) -> Result<SocketAddr>
    where
        A: ToSocketAddrs + Debug,
    {
        let key = format!("{:?}", addr);
        match self.cache.read().unwrap().get(&key).cloned() {
            Some((addr, instant)) => {
                if instant.elapsed() < Duration::from_secs(100) {
                    return Ok(addr);
                }
            }
            None => {}
        }
        let start = Instant::now();
        for socket_addr in addr.to_socket_addrs()? {
            if socket_addr.is_ipv4() {
                glog::debug!("query dns for {}: {:?}", key, start.elapsed());
                let mut guard = self.cache.write().unwrap();
                guard.insert(key, (socket_addr.clone(), Instant::now()));
                return Ok(socket_addr);
            }
        }

        return Err(Error::new(ErrorKind::Other, "SocketAddr not found"));
    }
}
