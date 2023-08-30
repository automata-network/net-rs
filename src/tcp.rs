use std::prelude::v1::*;

use std::io::Result;
use std::net::TcpStream;
use std::net::ToSocketAddrs;
use std::os::unix::io::AsRawFd;

use libc;
use net2;

pub fn set_nonblocking(fd: libc::c_int, nonblocking: bool) -> Result<()> {
    let mut nonblocking = nonblocking as libc::c_int;
    #[cfg(feature = "tstd")]
    cvt(unsafe { libc::ocall::ioctl_arg1(fd, libc::FIONBIO, &mut nonblocking) })?;
    #[cfg(not(feature = "tstd"))]
    cvt(unsafe { libc::ioctl(fd, libc::FIONBIO, &mut nonblocking) })?;

    Ok(())
}

pub fn connect<A>(addr: A) -> Result<TcpStream>
where
    A: ToSocketAddrs + std::fmt::Debug,
{
    let addr = super::dns::reslove(addr)?;

    let builder = net2::TcpBuilder::new_v4()?;
    set_nonblocking(builder.as_raw_fd(), true)?;
    // TODO: close socket when error?

    let stream = {
        match builder.connect(&addr) {
            Ok(stream) => stream,
            Err(err) => match err.raw_os_error() {
                Some(115) | Some(36) => {
                    // glog::info!("in progress: addr={}, {:?}", addr_name, err);
                    builder.to_tcp_stream()?
                }
                _ => return Err(err),
            },
        }
    };
    Ok(stream)
}

fn cvt(t: libc::c_int) -> std::io::Result<libc::c_int> {
    if t == -1 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(t)
    }
}
