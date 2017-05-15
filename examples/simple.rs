extern crate tokio_netmap;

fn main() {
    let netmap = match tokio_netmap::sys::Instance::new("em1") {
        Ok(v) => {
            println!("{:?}", v);
            println!("{:?}", v.iface());
            println!("TX {:?}", v.iface().tx_ring(0));
            println!("RX {:?}", v.iface().rx_ring(0));
            v
        },
        Err(reason) => panic!("Error {}", reason)
    };
    loop {
        netmap.rx_sync();
        let mut ring = netmap.iface().rx_ring(0);
        match ring.next() {
            Some(index) => println!("{:?}", ring.buffer_from_slot(index)),
            None => println!("Nope"),
        }
        ring.reclaim();
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
