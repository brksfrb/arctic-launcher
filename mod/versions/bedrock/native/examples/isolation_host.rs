//! Disposable headless process: exercises the probe guard on owned sentinel
//! directories, native relative opens, real Winsock and the actual GDK export.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(windows)]
#[path = "support/loopback_connect.rs"]
mod loopback_connect;
#[cfg(windows)]
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|v| v == "--pipe-child") {
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"isolated-child");
        return;
    }
    if !(2..=3).contains(&args.len())
        || args
            .get(2)
            .is_some_and(|v| v != "--loopback" && v != "--lan-loopback" && v != "--replay-loopback")
    {
        return;
    }
    let dir = std::path::PathBuf::from(&args[1]);
    let result = run(&dir,args.len()==3,args.get(2).is_some_and(|v|v=="--lan-loopback" || v=="--replay-loopback"),args.get(2).is_some_and(|v|v=="--replay-loopback")).unwrap_or_else(|e| serde_json::json!({"error":e.to_string(),"isolation":arctic_bedrock::probe_isolation::status()}));
    let _ = std::fs::write(dir.join("isolation-report.json"), result.to_string());
}
#[cfg(windows)]
fn run(
    dir: &std::path::Path,
    loopback: bool,
    lan: bool,
    replay: bool,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    use hudhook::windows::Win32::Storage::FileSystem::*;
    use hudhook::windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use hudhook::windows::core::{GUID, HRESULT, HSTRING, PCWSTR, s, w};
    use std::ffi::c_void;
    use std::io::Read;
    use std::os::windows::io::FromRawHandle;
    let source = dir.join("source");
    let profile = dir.join("profile");
    std::fs::create_dir(&source)?;
    std::fs::create_dir(&profile)?;
    std::fs::write(source.join("sentinel"), b"original")?;
    std::fs::write(profile.join("sentinel"), b"fresh")?;
    let old_socket = std::net::UdpSocket::bind("127.0.0.1:0")?;
    let replay_receiver = if replay {
        Some(std::net::UdpSocket::bind("127.0.0.2:19132")?)
    } else {
        None
    };
    let cached_connect = if loopback {
        Some(loopback_connect::discover()?)
    } else {
        None
    };
    use windows::Win32::NetworkManagement::IpHelper::{GetIfEntry, MIB_IFROW};
    let mut interface = MIB_IFROW {
        dwIndex: 1,
        ..Default::default()
    };
    if unsafe { GetIfEntry(&mut interface) } != 0 {
        return Err("baseline local interface query failed".into());
    }
    unsafe {
        let name = HSTRING::from(source.as_os_str());
        let handle = CreateFileW(
            PCWSTR(name.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )?;
        if replay {
            arctic_bedrock::probe_isolation::install_replay_loopback(&profile, &source)?;
        } else if lan {
            arctic_bedrock::probe_isolation::install_lan_loopback(&profile, &source)?;
        } else if loopback {
            arctic_bedrock::probe_isolation::install_loopback(&profile, &source)?;
        } else {
            arctic_bedrock::probe_isolation::install(&profile, &source)?;
        }
        interface = MIB_IFROW {
            dwIndex: 1,
            ..Default::default()
        };
        let interface_status = GetIfEntry(&mut interface);
        if (interface_status == 0) != loopback {
            return Err(format!(
                "local interface query guard mismatch: {interface_status}, loopback={loopback}"
            )
            .into());
        }
        if old_socket.send_to(b"probe", "192.0.2.1:9").is_ok() {
            return Err("pre-existing socket escaped network IO guard".into());
        }
        if std::fs::read(source.join("sentinel"))? != b"fresh" {
            return Err("absolute source read was not redirected".into());
        }
        std::fs::write(source.join("written"), b"test output")?;
        std::fs::rename(source.join("written"), source.join("renamed"))?;
        if std::fs::read(profile.join("renamed"))? != b"test output" {
            return Err("native rename destination escaped redirect".into());
        }
        std::fs::remove_file(source.join("renamed"))?;
        #[repr(C)]
        struct Unicode {
            length: u16,
            maximum: u16,
            buffer: *const u16,
        }
        #[repr(C)]
        struct Attributes {
            length: u32,
            root: *mut c_void,
            name: *const Unicode,
            flags: u32,
            security: *mut c_void,
            qos: *mut c_void,
        }
        #[repr(C)]
        struct Status {
            status: usize,
            information: usize,
        }
        type Open = unsafe extern "system" fn(
            *mut *mut c_void,
            u32,
            *const Attributes,
            *mut Status,
            u32,
            u32,
        ) -> i32;
        let open: Open = std::mem::transmute(
            GetProcAddress(LoadLibraryW(w!("ntdll.dll"))?, s!("NtOpenFile"))
                .ok_or("NtOpenFile unavailable")?,
        );
        let path: Vec<u16> = "sentinel".encode_utf16().collect();
        let name = Unicode {
            length: (path.len() * 2) as u16,
            maximum: (path.len() * 2) as u16,
            buffer: path.as_ptr(),
        };
        let attributes = Attributes {
            length: size_of::<Attributes>() as u32,
            root: handle.0,
            name: &name,
            flags: 0x40,
            security: std::ptr::null_mut(),
            qos: std::ptr::null_mut(),
        };
        let mut out = std::ptr::null_mut();
        let mut status = Status {
            status: 0,
            information: 0,
        };
        let result = open(
            &mut out,
            FILE_GENERIC_READ.0,
            &attributes,
            &mut status,
            (FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0,
            0x20,
        );
        hudhook::windows::Win32::Foundation::CloseHandle(handle)?;
        if result < 0 {
            return Err(format!("relative native open failed: {result:08x}").into());
        }
        let mut file = std::fs::File::from_raw_handle(out);
        let mut value = String::new();
        file.read_to_string(&mut value)?;
        if value != "fresh" {
            return Err("relative native open escaped profile redirect".into());
        }
        // There is no real connection: AFD endpoint creation/IO must fail first.
        let address = "192.0.2.1:9".parse()?;
        if std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(50))
            .is_ok()
        {
            return Err("network guard allowed a connection".into());
        }
        type Api = unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;
        let api: Api = std::mem::transmute(
            GetProcAddress(LoadLibraryW(w!("xgameruntime.dll"))?, s!("QueryApiImpl"))
                .ok_or("GDK query unavailable")?,
        );
        for (class, iid) in [
            (
                GUID::from_u128(0x01acd177_91f9_4763_a38e_ccbb55ce32e0),
                GUID::from_u128(0x5131d685_4394_4ee6_8c18_bfb5d4aef1ff),
            ),
            (
                GUID::from_u128(0x7d824997_10dc_45ab_86b7_2737767c0bf1),
                GUID::from_u128(0x0cc6a956_e7e1_4fdf_9341_9d5da649ebc8),
            ),
        ] {
            let mut object = std::ptr::dangling_mut::<c_void>();
            if api(&class, &iid, &mut object).0 != 0x80070005u32 as i32 || !object.is_null() {
                return Err("GDK user/device provider was not denied".into());
            }
        }
    }
    let report = arctic_bedrock::probe_isolation::status();
    if loopback && report["localNetworkQueries"].as_u64().unwrap_or(0) < 2 {
        return Err("local interface query did not exercise the NSI guard".into());
    }
    if loopback {
        loopback_connect::check(cached_connect.ok_or("missing cached ConnectEx")?)?;
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("0.0.0.0:0")
            .map_err(|e| format!("local TCP listener: {e}"))?;
        if !listener.local_addr()?.ip().is_loopback() {
            return Err("wildcard listener was not restricted to loopback".into());
        }
        let mut client = std::net::TcpStream::connect(listener.local_addr()?)
            .map_err(|e| format!("local TCP connect: {e}"))?;
        let (mut server, _) = listener
            .accept()
            .map_err(|e| format!("local TCP accept: {e}"))?;
        client.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
        server.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
        client.write_all(b"local")?;
        let mut bytes = [0u8; 5];
        server.read_exact(&mut bytes)?;
        if &bytes != b"local" {
            return Err("loopback TCP bytes changed".into());
        }
        let udp = std::net::UdpSocket::bind("0.0.0.0:0")?;
        if !udp.local_addr()?.ip().is_loopback() {
            return Err("wildcard datagram bind was not restricted to loopback".into());
        }
        udp.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
        udp.set_broadcast(true)?;
        if let Some(receiver) = &replay_receiver {
            receiver.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
            udp.send_to(b"replay-discovery", "255.255.255.255:19132")?;
            let mut payload = [0u8; 64];
            let (length, peer) = receiver.recv_from(&mut payload)?;
            if &payload[..length] != b"replay-discovery" || !peer.ip().is_loopback() {
                return Err("replay discovery did not reach the owned loopback receiver".into());
            }
            if udp.send_to(b"blocked", "255.255.255.255:19133").is_ok() {
                return Err("replay discovery admitted a different broadcast port".into());
            }
        } else if udp.send_to(b"blocked", "255.255.255.255:19132").is_ok() {
            return Err("ordinary loopback mode admitted replay broadcast translation".into());
        }
        if udp.send_to(b"probe", "192.0.2.1:19132").is_ok() {
            return Err("broadcast configuration bypassed datagram destination guard".into());
        }
        #[link(name = "ws2_32")]
        unsafe extern "system" {
            fn socket(af: i32, kind: i32, protocol: i32) -> usize;
            fn setsockopt(
                socket: usize,
                level: i32,
                option: i32,
                value: *const i32,
                size: i32,
            ) -> i32;
            fn closesocket(socket: usize) -> i32;
            fn bind(socket: usize, address: *const u8, length: i32) -> i32;
            fn getsockopt(
                socket: usize,
                level: i32,
                option: i32,
                value: *mut i32,
                size: *mut i32,
            ) -> i32;
            fn WSAIoctl(
                socket: usize,
                code: u32,
                input: *const c_void,
                input_size: u32,
                output: *mut c_void,
                output_size: u32,
                returned: *mut u32,
                overlapped: *mut c_void,
                completion: *mut c_void,
            ) -> i32;
        }
        use std::os::windows::io::AsRawSocket;
        let mut addresses = [0u64; 8192];
        let mut returned = 0;
        let address_query = unsafe {
            WSAIoctl(
                old_socket.as_raw_socket() as usize,
                0x48000016,
                std::ptr::null(),
                0,
                addresses.as_mut_ptr().cast(),
                size_of_val(&addresses) as u32,
                &mut returned,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if address_query != 0 || returned < 4 {
            return Err(format!(
                "read-only Winsock address-list query failed: {address_query}, bytes={returned}"
            )
            .into());
        }
        #[repr(C)]
        struct SocketAddress {
            address: *const u8,
            length: i32,
        }
        #[repr(C)]
        struct AddressList {
            count: i32,
            addresses: [SocketAddress; 2],
        }
        let mut local6 = [0u8; 28];
        local6[..2].copy_from_slice(&23u16.to_ne_bytes());
        local6[23] = 1;
        let mut local4 = [0u8; 28];
        local4[..2].copy_from_slice(&23u16.to_ne_bytes());
        local4[18..24].copy_from_slice(&[255, 255, 127, 0, 0, 1]);
        let list = AddressList {
            count: 2,
            addresses: [
                SocketAddress {
                    address: local6.as_ptr(),
                    length: 28,
                },
                SocketAddress {
                    address: local4.as_ptr(),
                    length: 28,
                },
            ],
        };
        let sort_socket = std::net::UdpSocket::bind("[::1]:0")?;
        let sort_result = unsafe {
            WSAIoctl(
                sort_socket.as_raw_socket() as usize,
                0xc8000019,
                (&list as *const AddressList).cast(),
                size_of::<AddressList>() as u32,
                addresses.as_mut_ptr().cast(),
                size_of_val(&addresses) as u32,
                &mut returned,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if sort_result != 0 || returned < 4 {
            return Err(format!(
                "read-only Winsock address sorting failed: {sort_result}, bytes={returned}"
            )
            .into());
        }
        let ipv6_setup = unsafe {
            let socket = socket(23, 2, 17);
            if socket == usize::MAX {
                return Err("IPv6 socket creation failed".into());
            }
            let value = 0;
            let result = setsockopt(socket, 41, 27, &value, 4);
            closesocket(socket);
            result == 0
        };
        if !ipv6_setup {
            return Err("pre-bind IPv6 configuration was denied".into());
        }
        use std::os::windows::io::FromRawSocket;
        let (mapped, before) = unsafe {
            let socket = socket(23, 2, 17);
            if socket == usize::MAX {
                return Err("dual-stack socket creation failed".into());
            }
            let value = 0;
            let mut address = [0; 28];
            address[..2].copy_from_slice(&23u16.to_ne_bytes());
            if setsockopt(socket, 41, 27, &value, 4) != 0 {
                closesocket(socket);
                return Err("dual-stack setup failed".into());
            }
            let mut before = 99;
            let mut size = 4;
            let query = getsockopt(socket, 41, 27, &mut before, &mut size);
            if bind(socket, address.as_ptr(), 28) != 0 {
                closesocket(socket);
                return Err("dual-stack wildcard bind failed".into());
            }
            (
                std::net::UdpSocket::from_raw_socket(socket as _),
                (query, size, before),
            )
        };
        let std::net::IpAddr::V6(mapped_ip) = mapped.local_addr()?.ip() else {
            return Err("dual-stack address lost IPv6 family".into());
        };
        if mapped_ip.to_ipv4_mapped() != Some(std::net::Ipv4Addr::LOCALHOST) {
            return Err(format!(
                "dual-stack bind did not retain mapped loopback: {mapped_ip}, before={before:?}"
            )
            .into());
        }
        mapped.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
        old_socket.send_to(
            b"local",
            (std::net::Ipv4Addr::LOCALHOST, mapped.local_addr()?.port()),
        )?;
        let (length, _) = mapped.recv_from(&mut bytes)?;
        if length != 5 || &bytes != b"local" {
            return Err("dual-stack IPv4 reception failed".into());
        }
        if lan {
            let translated = std::net::UdpSocket::bind("192.0.2.1:0")?;
            if !translated.local_addr()?.ip().is_loopback() {
                return Err("LAN interface bind escaped loopback translation".into());
            }
            if translated.send_to(b"probe", "192.0.2.1:19132").is_ok() {
                return Err("LAN interface translation allowed external traffic".into());
            }
        } else if std::net::UdpSocket::bind("192.0.2.1:0").is_ok() {
            return Err("ordinary loopback mode translated an explicit foreign bind".into());
        }
        let udp6 = std::net::UdpSocket::bind("[::]:0")?;
        if !udp6.local_addr()?.ip().is_loopback() {
            return Err("wildcard IPv6 bind was not restricted to loopback".into());
        }
        udp6.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
        let sender6 = std::net::UdpSocket::bind("[::1]:0")?;
        if sender6.send_to(b"probe", "[2001:db8::1]:19132").is_ok() {
            return Err("IPv6 configuration bypassed datagram destination guard".into());
        }
        sender6.send_to(b"local", udp6.local_addr()?)?;
        let (length, _) = udp6.recv_from(&mut bytes)?;
        if length != 5 || &bytes != b"local" {
            return Err("IPv6 loopback bytes changed".into());
        }
        old_socket.send_to(b"local", udp.local_addr()?)?;
        let (length, _) = udp.recv_from(&mut bytes)?;
        if length != 5 || &bytes != b"local" {
            return Err("loopback datagram bytes changed".into());
        }
    }
    // Encoder launch needs anonymous pipes. This must remain available while
    // the isolated profile and network/user denial guards are active.
    use std::os::windows::process::CommandExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreatePipe(
            read: *mut *mut c_void,
            write: *mut *mut c_void,
            security: *const c_void,
            size: u32,
        ) -> i32;
    }
    let mut pipe_read = std::ptr::null_mut();
    let mut pipe_write = std::ptr::null_mut();
    if unsafe { CreatePipe(&mut pipe_read, &mut pipe_write, std::ptr::null(), 0) } == 0 {
        return Err(format!(
            "anonymous pipe creation: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    unsafe {
        let _ = hudhook::windows::Win32::Foundation::CloseHandle(
            hudhook::windows::Win32::Foundation::HANDLE(pipe_read),
        );
        let _ = hudhook::windows::Win32::Foundation::CloseHandle(
            hudhook::windows::Win32::Foundation::HANDLE(pipe_write),
        );
    }
    let child = std::process::Command::new(std::env::current_exe()?)
        .arg("--pipe-child")
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|e| format!("child launch/stdio: {e}"))?;
    if !child.status.success() || child.stdout != b"isolated-child" {
        return Err("isolated child pipe round trip failed".into());
    }
    let report = arctic_bedrock::probe_isolation::status();
    if report["networkDenied"].as_u64().unwrap_or(0) == 0 {
        return Err("Winsock never reached the network guard".into());
    }
    Ok(
        serde_json::json!({"passed":true,"absoluteRedirect":true,"relativeRedirect":true,"loopbackTcp":loopback,"loopbackUdp":loopback,"loopbackConnectEx":loopback,"childPipes":true,"isolation":report}),
    )
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows isolation probe requires Windows");
}
