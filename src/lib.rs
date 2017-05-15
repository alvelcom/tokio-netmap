extern crate mio;
extern crate futures;
extern crate tokio_core;

use std::io;
use tokio_core::reactor;

use std::rc::Rc;

pub mod sys; 

pub struct Netmap<'a> {
    inner: Rc<Inner>,
    handle: &'a reactor::Handle,
}

#[derive(Clone, Debug)]
pub struct Ring {
    ring: RingId,
    inner: Rc<Inner>,
}

#[derive(Debug)]
struct Inner {
    sys: sys::Instance,
    io: reactor::PollEvented<NetmapFd>,
}


#[derive(Clone, Copy, Debug)]
pub enum RingId {
    Tx(u32),
    Rx(u32),
}


#[derive(Clone, Copy, Debug)]
pub struct NetmapFd(pub std::os::unix::io::RawFd);

impl<'a> Netmap<'a> {
    pub fn new(interface_name: &str, handle: &'a reactor::Handle) -> io::Result<Self> {
		let inner = sys::Instance::new(interface_name)?;
        let io = reactor::PollEvented::new(NetmapFd(inner.fd), handle)?;
        Ok(Netmap {
            inner: Rc::new(Inner{
                sys: inner,
                io: io,
            }),
            handle: handle,
        })
    }

    pub fn open(&self, ring: RingId) -> io::Result<Ring> {
        Ok(Ring {
            ring: ring,
            inner: self.inner.clone(),
        })
    }
}

impl futures::Stream for Ring {
	type Item = Slot;
	type Error = io::Error;

	fn poll(&mut self) -> futures::Poll<Option<Self::Item>, Self::Error> {
        let ring = match self.ring {
            RingId::Tx(index) => self.inner.sys.iface().tx_ring(index),
            RingId::Rx(index) => self.inner.sys.iface().rx_ring(index),
        };
        if !ring.has_next() {
            self.inner.sys.rx_sync();
        }

        match ring.next() {
            Some(index) => Ok(futures::Async::Ready(Some(Slot {
                    index: index,
                    ring: self.ring,
                    inner: self.inner.clone(),
               }))),
            None => {
                self.inner.io.need_read();
                Ok(futures::Async::NotReady)
            }
        }
    }
}

#[derive(Debug)]
pub struct Slot {
    index: u32,
    ring: RingId,
    inner: Rc<Inner>,
}

impl Slot {
    pub fn get(&self) -> &[u8] {
        let ring = match self.ring {
            RingId::Tx(index) => self.inner.sys.iface().tx_ring(index),
            RingId::Rx(index) => self.inner.sys.iface().rx_ring(index),
        };
        ring.buffer_from_slot(self.index)
    }

    pub fn get_mut(&self) -> &mut [u8] {
        let ring = match self.ring {
            RingId::Tx(index) => self.inner.sys.iface().tx_ring(index),
            RingId::Rx(index) => self.inner.sys.iface().rx_ring(index),
        };
        ring.buffer_from_slot(self.index)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let ring = self.inner.sys.iface().rx_ring(0);
        ring.reclaim();
    }
}


impl mio::Evented for NetmapFd {
    fn register(&self, poll: &mio::Poll, token: mio::Token,
                interest: mio::Ready, opts: mio::PollOpt) -> io::Result<()> {
        mio::unix::EventedFd(&self.0).register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &mio::Poll, token: mio::Token,
                  interest: mio::Ready, opts: mio::PollOpt) -> io::Result<()> {
        mio::unix::EventedFd(&self.0).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        mio::unix::EventedFd(&self.0).deregister(poll)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
