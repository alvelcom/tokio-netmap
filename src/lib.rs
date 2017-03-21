extern crate mio;
extern crate futures;
extern crate tokio_core;
extern crate libc;

use tokio_core::reactor::{PollEvented};

pub struct Netmap<'a> {
    pub io: PollEvented<mio::unix::EventedFd<'a>>,
}

pub mod sys {
    use std;
    use std::io;
    use std::os::unix::io::AsRawFd;
    use libc;

    #[derive(Debug)]
    pub struct Instance {
        file: std::fs::File,
        fd: std::os::unix::io::RawFd,

        request: Request,

        region: *mut libc::c_void,
    }
    
    impl Instance {
        pub fn new(interface_name: &str) -> io::Result<Instance> {
            let file = std::fs::OpenOptions::new()
                       .create(false).write(true).read(true)
                       .open(std::path::Path::new("/dev/netmap"))?;
            let fd = file.as_raw_fd();

            let mut req = Request {
                name: IfaceName([0; 16]),
                version: NETMAP_API,
                offset: 0,
                memory_size: 0,
                tx_slots: 0,
                rx_slots: 0,
                tx_rings: 0,
                rx_rings: 0,
                ring_id: 0x4000,
                cmd: 0,
                arg1: 0,
                arg2: 0,
                arg3: 0,
                flags: 0,
                _spare: Nop([0; 1])
            };
            req.name.0[0..interface_name.len()].copy_from_slice(interface_name.as_bytes());

            let result = unsafe { libc::ioctl(fd, NIOCREGIF, &req as *const Request) };
            if result != 0 {
                return Err(io::Error::last_os_error())
            }
            
            let region = unsafe { libc::mmap(std::mem::transmute(0 as usize),
                                             req.memory_size as usize,
                                             libc::PROT_READ | libc::PROT_WRITE,
                                             libc::MAP_SHARED, fd, 0) };
            if region == libc::MAP_FAILED {
                return Err(io::Error::last_os_error())
            }

            Ok(Instance {
                file: file,
                fd: fd,
                request: req,
                region: region,
            })
        }

        pub fn iface(&self) -> &mut Iface {
            unsafe {
                let base = self.region as *mut u8;
                let iface = base.offset(self.request.offset as isize) as *mut Iface;
                &mut *iface
            }
        }
    }

    const NETMAP_API: u32 = 11;
    #[cfg(target_os = "linux")]
    pub const NIOCGINFO: libc::c_ulong = 3225184657;
    #[cfg(target_os = "linux")]
    pub const NIOCREGIF: libc::c_ulong = 3225184658;
    #[cfg(target_os = "linux")]
    pub const NIOCTXSYNC: libc::c_uint = 27028;
    #[cfg(target_os = "linux")]
    pub const NIOCRXSYNC: libc::c_uint = 27029;
    #[cfg(target_os = "linux")]
    pub const NIOCCONFIG: libc::c_ulong = 3239078294;

    #[cfg(target_os = "freebsd")]
    pub const NIOCGINFO: libc::c_ulong = 3225184657;
    #[cfg(target_os = "freebsd")]
    pub const NIOCREGIF: libc::c_ulong = 3225184658;
    #[cfg(target_os = "freebsd")]
    pub const NIOCTXSYNC: libc::c_uint = 536897940;
    #[cfg(target_os = "freebsd")]
    pub const NIOCRXSYNC: libc::c_uint = 536897941;
    #[cfg(target_os = "freebsd")]
    pub const NIOCCONFIG: libc::c_ulong = 3239078294;
 
    #[derive(Debug)]
    #[repr(C)]
    pub struct Request {
        name: IfaceName,
        version: u32, // API Version
        offset: u32, // nifp offset in shared region
        memory_size: u32, // size of shared memory
        tx_slots: u32, // slots in tx rings
        rx_slots: u32, // slots in rx rings
        tx_rings: u16, // number of tx rings
        rx_rings: u16, // number of rx rings

        ring_id: u16, // ring(s) we care about

        cmd: u16, 
        arg1: u16,
        arg2: u16,
        arg3: u32,

        flags: u32,

        _spare: Nop<[u32; 1]>,
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct Iface {
        name: IfaceName,
        version: u32,
        flags: u32,

        tx_rings: u32,
        rx_rings: u32,

        extra_buffers_head: u32,
        spare: Nop<[u32; 5]>,

        ring_offsets: [libc::ssize_t; 0],
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct Ring {
       buffer_offset: i64,
       num_slots: u32,
       buffer_size: u32,
       ringid: u16,
       direction: u16,

       head: u32,
       cur: u32,
       tail: u32,

       flags: u32,

       ts: Nop<libc::timeval>,

       _padding: Nop<[u8; 72]>,
       sem: Nop<[u8; 128]>,

       slots: [Slot; 0],
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct Slot {
        buffer_index: u32,
        length: u16,
        flags: u16,
        ptr: u64,
    }

    pub struct IfaceName(pub [u8; libc::IF_NAMESIZE]);

    impl std::fmt::Debug for IfaceName {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            let str = unsafe {
                let ptr = &std::mem::transmute::<[u8; libc::IF_NAMESIZE], [i8; libc::IF_NAMESIZE]>(self.0);
                std::ffi::CStr::from_ptr(ptr as *const libc::c_char)
            };
            write!(f, "{:?}", str)
        }
    }

    pub struct Nop<T>(pub T);
    impl <T> std::fmt::Debug for Nop<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "NOP")
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
