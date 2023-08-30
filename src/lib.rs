#![cfg_attr(feature = "tstd", no_std)]

#[cfg(feature = "tstd")]
#[macro_use]
extern crate sgxlib as std;

mod any_stream;
pub use any_stream::*;
mod dns;
pub use dns::*;
pub mod tcp;
