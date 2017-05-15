extern crate libc;

use std;
use std::io;
use std::os::unix::io::AsRawFd;

#[derive(Debug)]
pub struct Instance {
    file: std::fs::File,
    pub fd: std::os::unix::io::RawFd,

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
            ring_id: 0x4000, // Only HW rings
            cmd: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
            flags: 0,
            _spare: Nop([0; 1])
        };
        req.name.0[0..interface_name.len()].copy_from_slice(interface_name.as_bytes());

        let result = unsafe { libc::ioctl(fd, ioctls::NIOCREGIF, &req as *const Request) };
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

    pub fn iface(&self) -> &Iface {
        unsafe {
            let base = self.region as *mut u8;
            let iface = base.offset(self.request.offset as isize) as *const Iface;
            &*iface
        }
    }

    pub fn tx_sync(&self) {
        unsafe { libc::ioctl(self.fd, ioctls::NIOCTXSYNC, &self.request as *const Request) };
    }
    pub fn rx_sync(&self) {
        unsafe { libc::ioctl(self.fd, ioctls::NIOCRXSYNC, &self.request as *const Request) };
    }
}

const NETMAP_API: u32 = 11;

#[cfg(target_os = "linux")]
mod ioctls {
    pub const NIOCREGIF: libc::c_ulong = 3225184658;
    pub const NIOCTXSYNC: libc::c_ulong = 27028;
    pub const NIOCRXSYNC: libc::c_ulong = 27029;
    /*
    pub const NIOCGINFO: libc::c_ulong = 3225184657;
    pub const NIOCCONFIG: libc::c_ulong = 3239078294;
    */
}

#[cfg(target_os = "freebsd")]
mod ioctls {
    extern crate libc;
    pub const NIOCREGIF: libc::c_ulong = 3225184658;
    pub const NIOCTXSYNC: libc::c_ulong = 536897940;
    pub const NIOCRXSYNC: libc::c_ulong = 536897941;
    /*
    pub const NIOCGINFO: libc::c_ulong = 3225184657;
    pub const NIOCCONFIG: libc::c_ulong = 3239078294;
    */
}

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

impl Iface {
    #[inline]
    fn ring(&self, index: isize) -> &mut Ring {
        let iface = self as *const Iface as *mut u8;
        let ring_offsets = &self.ring_offsets as *const libc::ssize_t;
        unsafe {
            let ring_offset = *ring_offsets.offset(index);
            let ring = iface.offset(ring_offset as isize) as *mut Ring;
            &mut *ring
        }
    }

    pub fn tx_ring(&self, index: u32) -> &mut Ring {
        if index >= self.tx_rings {
            panic!("Can't get TX ring #{} for {:?}", index, self.name)
        }
        self.ring(index as isize)
    }

    pub fn rx_ring(&self, index: u32) -> &mut Ring {
        if index >= self.rx_rings {
            panic!("Can't get RX ring #{} for {:?}", index, self.name)
        }
        self.ring(1 + index as isize + self.tx_rings as isize)
    }
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

impl Ring {
    pub fn slot(&self, index: u32) -> &mut Slot {
        if index > self.num_slots {
            panic!("")
        }
        let slots = &self.slots as *const [Slot; 0] as *mut Slot;
        unsafe { &mut *slots.offset(index as isize) }
    }

    fn buffer(&self, index: u32) -> &mut [u8] {
        let offset = self.buffer_offset + (index * self.buffer_size) as i64;
        let ring = self as *const Ring as *mut u8;
        unsafe {
            let buffer = ring.offset(offset as isize);
            std::slice::from_raw_parts_mut(buffer, self.buffer_size as usize)
        }
    }

    pub fn buffer_from_slot(&self, index: u32) -> &mut [u8] {
        self.buffer(self.slot(index).buffer_index)
    }

    pub fn has_next(&mut self) -> bool {
        self.cur != self.tail
    }

    pub fn next(&mut self) -> Option<u32> {
        if self.cur == self.tail {
            None
        } else {
            let prev = self.cur;
            self.cur = (self.cur + 1) % self.num_slots;
            Some(prev)
        }
    }

    pub fn reclaim(&mut self) {
        if self.head != self.cur {
            self.head = self.cur;
        }
    }
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
