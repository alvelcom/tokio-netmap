extern crate futures;
extern crate tokio_core;
extern crate tokio_netmap;

use futures::Stream;
use futures::Future;
use tokio_netmap::RingId;

fn main() {
    let mut core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let handle2 = handle.clone();

    let netmap = tokio_netmap::Netmap::new("em1", &handle2).unwrap();
    let rx_ring = netmap.open(RingId::Rx(0)).unwrap();
    let tx_ring = netmap.open(RingId::Tx(0)).unwrap();
    let server = rx_ring.for_each(|rx_slot| {
        println!("got a packet");
        let tx_ring2 = tx_ring.clone();
        let answer = tx_ring2.take(1).into_future().then(move |tx_slot| {
            println!("send a packet");
            tx_slot.unwrap().0.unwrap().get_mut().copy_from_slice(rx_slot.get());
            Ok(())
        });
        handle.spawn(answer);
        Ok(())
    });

    core.run(server).unwrap();
}

