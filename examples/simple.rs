extern crate tokio_netmap;

fn main() {
    let netmap = tokio_netmap::sys::Instance::new("em1");
    match netmap {
        Ok(netmap) => println!("Got {:?}", netmap.iface()),
        Err(reason) => panic!("Error {}", reason)
    }
}
