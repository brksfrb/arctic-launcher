//! Real overlapped ConnectEx transfer under the process-local loopback guard.
use hudhook::windows::{
    Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    core::{GUID, s, w},
};
use std::{
    ffi::c_void,
    io::{Read, Write},
    os::windows::io::FromRawSocket,
};
type Connect = unsafe extern "system" fn(
    usize,
    *const u8,
    i32,
    *const c_void,
    u32,
    *mut u32,
    *mut Overlapped,
) -> i32;
type Close = unsafe extern "system" fn(usize) -> i32;
struct Socket(usize, Close);
impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            (self.1)(self.0);
        }
    }
}
#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offsets: u64,
    event: usize,
}
pub fn discover() -> Result<usize, Box<dyn std::error::Error>> {
    use std::os::windows::io::AsRawSocket;
    type Query = unsafe extern "system" fn(
        usize,
        u32,
        *const c_void,
        u32,
        *mut c_void,
        u32,
        *mut u32,
        *mut c_void,
        *mut c_void,
    ) -> i32;
    let socket = std::net::TcpListener::bind("127.0.0.1:0")?;
    unsafe {
        let module = LoadLibraryW(w!("ws2_32.dll"))?;
        let query: Query = std::mem::transmute(
            GetProcAddress(module, s!("WSAIoctl")).ok_or("extension query missing")?,
        );
        let id = GUID::from_u128(0x25a207b9_ddf3_4660_8ee9_76e58c74063e);
        let mut function = 0usize;
        let mut used = 0;
        if query(
            socket.as_raw_socket() as usize,
            0xc8000006,
            (&id as *const GUID).cast(),
            16,
            (&mut function as *mut usize).cast(),
            8,
            &mut used,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) != 0
            || used != 8
            || function == 0
        {
            return Err("cached ConnectEx discovery failed".into());
        }
        Ok(function)
    }
}
pub fn check(cached: usize) -> Result<(), Box<dyn std::error::Error>> {
    type Create = unsafe extern "system" fn(i32, i32, i32, *const c_void, u32, u32) -> usize;
    type Bind = unsafe extern "system" fn(usize, *const u8, i32) -> i32;
    type Ioctl = unsafe extern "system" fn(
        usize,
        u32,
        *const c_void,
        u32,
        *mut c_void,
        u32,
        *mut u32,
        *mut c_void,
        *mut c_void,
    ) -> i32;
    type Result = unsafe extern "system" fn(usize, *mut Overlapped, *mut u32, i32, *mut u32) -> i32;
    type SetOption = unsafe extern "system" fn(usize, i32, i32, *const c_void, i32) -> i32;
    type Error = unsafe extern "system" fn() -> i32;
    unsafe {
        let module = LoadLibraryW(w!("ws2_32.dll"))?;
        let create: Create = std::mem::transmute(
            GetProcAddress(module, s!("WSASocketW")).ok_or("WSASocket missing")?,
        );
        let close: Close = std::mem::transmute(
            GetProcAddress(module, s!("closesocket")).ok_or("closesocket missing")?,
        );
        let bind: Bind =
            std::mem::transmute(GetProcAddress(module, s!("bind")).ok_or("bind missing")?);
        let ioctl: Ioctl =
            std::mem::transmute(GetProcAddress(module, s!("WSAIoctl")).ok_or("WSAIoctl missing")?);
        let result: Result = std::mem::transmute(
            GetProcAddress(module, s!("WSAGetOverlappedResult"))
                .ok_or("overlapped result missing")?,
        );
        let error: Error = std::mem::transmute(
            GetProcAddress(module, s!("WSAGetLastError")).ok_or("Winsock error missing")?,
        );
        let set_option: SetOption = std::mem::transmute(
            GetProcAddress(module, s!("setsockopt")).ok_or("setsockopt missing")?,
        );
        let raw = create(2, 1, 6, std::ptr::null(), 0, 1);
        if raw == usize::MAX {
            return Err(format!("overlapped socket creation: {}", error()).into());
        }
        let socket = Socket(raw, close);
        let mut bound = [0u8; 16];
        bound[..2].copy_from_slice(&2u16.to_ne_bytes());
        if bind(socket.0, bound.as_ptr(), 16) != 0 {
            return Err("overlapped socket bind failed".into());
        }
        let id = GUID::from_u128(0x25a207b9_ddf3_4660_8ee9_76e58c74063e);
        let mut function = 0usize;
        let mut used = 0;
        if ioctl(
            socket.0,
            0xc8000006,
            (&id as *const GUID).cast(),
            16,
            (&mut function as *mut usize).cast(),
            8,
            &mut used,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) != 0
            || used != 8
            || function == 0
        {
            return Err(format!("ConnectEx extension query: {}", error()).into());
        }
        if function != cached {
            return Err("ConnectEx implementation unexpectedly changed".into());
        }
        let connect: Connect = std::mem::transmute(cached);
        // Bounded owned fixture allocation remains alive through any pending
        // kernel completion, including a failed test; the process then exits.
        let overlapped = Box::leak(Box::new(Overlapped {
            internal: 0,
            internal_high: 0,
            offsets: 0,
            event: 0,
        }));
        let mut remote = bound;
        remote[2..4].copy_from_slice(&9u16.to_be_bytes());
        remote[4..8].copy_from_slice(&[192, 0, 2, 1]);
        let mut sent = 0;
        if connect(
            socket.0,
            remote.as_ptr(),
            16,
            std::ptr::null(),
            0,
            &mut sent,
            overlapped,
        ) != 0
            || error() != 10013
        {
            return Err("ConnectEx external destination was not denied".into());
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let mut local = bound;
        local[2..4].copy_from_slice(&listener.local_addr()?.port().to_be_bytes());
        local[4..8].copy_from_slice(&[127, 0, 0, 1]);
        if connect(
            socket.0,
            local.as_ptr(),
            16,
            b"async".as_ptr().cast(),
            5,
            &mut sent,
            overlapped,
        ) == 0
            && error() != 997
        {
            return Err(format!("loopback ConnectEx initiation: {}", error()).into());
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut flags = 0;
        loop {
            if result(socket.0, overlapped, &mut sent, 0, &mut flags) != 0 {
                break;
            }
            let error = error();
            if error != 996 || std::time::Instant::now() >= deadline {
                return Err(format!("loopback ConnectEx completion: {error}").into());
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        if sent != 5 {
            return Err("ConnectEx initial payload length changed".into());
        }
        if set_option(socket.0, 0xffff, 0x7010, std::ptr::null(), 0) != 0 {
            return Err("ConnectEx context update failed".into());
        }
        let mut client = std::net::TcpStream::from_raw_socket(socket.0 as _);
        std::mem::forget(socket);
        let (mut server, _) = listener.accept()?;
        server.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
        let mut bytes = [0u8; 5];
        server.read_exact(&mut bytes)?;
        if &bytes != b"async" {
            return Err("ConnectEx initial payload changed".into());
        }
        client.write_all(b"after")?;
        server.read_exact(&mut bytes)?;
        if &bytes != b"after" {
            return Err("ConnectEx connected payload changed".into());
        }
        Ok(())
    }
}
