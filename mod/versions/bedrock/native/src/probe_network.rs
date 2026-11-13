//! Optional loopback-only transport for the isolated installed-game test.
//! Address validation uses public Winsock APIs; AFD operations without a checked
//! destination scope or a real loopback peer remain denied. This is test
//! instrumentation, not a general-purpose network sandbox.
use hudhook::{
    mh::{MH_CreateHook, MH_STATUS},
    windows::{
        Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW},
        core::{GUID, s, w},
    },
};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::{cell::Cell, ffi::c_void, sync::OnceLock};

type AddressCall = unsafe extern "system" fn(usize, *const u8, i32) -> i32;
type ConnectEx = unsafe extern "system" fn(
    usize,
    *const u8,
    i32,
    *const c_void,
    u32,
    *mut u32,
    *mut c_void,
) -> i32;
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
const CONNECT_EX_ID: GUID = GUID::from_u128(0x25a207b9_ddf3_4660_8ee9_76e58c74063e);
static CONNECT_EX: OnceLock<usize> = OnceLock::new();
static EXTENSION_QUERIES: AtomicU64 = AtomicU64::new(0);
static EXTENSION_CONNECTS: AtomicU64 = AtomicU64::new(0);
static CONNECTS: AtomicU64 = AtomicU64::new(0);
static LAST_DENIED_IO: AtomicU64 = AtomicU64::new(0);
static LAST_DENIED_NAME: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
static HELPER_CONNECTS: AtomicU64 = AtomicU64::new(0);
static LAST_TRANSPORT: std::sync::Mutex<Vec<serde_json::Value>> = std::sync::Mutex::new(Vec::new());
static DENIED_OPERATIONS: [AtomicU64; 65] = [const { AtomicU64::new(0) }; 65];
static REJECTED_BINDS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
static ADDRESS_QUERIES: AtomicU64 = AtomicU64::new(0);
static ADDRESS_SORTS: AtomicU64 = AtomicU64::new(0);
static DUAL_STACK_BINDS: AtomicU64 = AtomicU64::new(0);
static LAN_BIND_TRANSLATION: AtomicBool = AtomicBool::new(false);
static TRANSLATED_BINDS: AtomicU64 = AtomicU64::new(0);
static REPLAY_DISCOVERY: AtomicBool = AtomicBool::new(false);
static DISCOVERY_TRANSLATIONS: AtomicU64 = AtomicU64::new(0);
pub(super) fn enable_replay_discovery() {
    REPLAY_DISCOVERY.store(true, Ordering::Release);
    enable_lan_bind_translation();
}
pub(super) fn replay_discovery_status() -> serde_json::Value {
    serde_json::json!({"enabled":REPLAY_DISCOVERY.load(Ordering::Acquire),
        "translatedDatagrams":DISCOVERY_TRANSLATIONS.load(Ordering::Relaxed)})
}
pub(super) fn enable_lan_bind_translation() {
    LAN_BIND_TRANSLATION.store(true, Ordering::Release);
}
pub(super) fn status() -> serde_json::Value {
    let denied: Vec<_> = DENIED_OPERATIONS
        .iter()
        .enumerate()
        .filter_map(|(operation, count)| {
            let count = count.load(Ordering::Relaxed);
            (count > 0).then(|| serde_json::json!({"operation":operation,"count":count}))
        })
        .collect();
    let rejected_binds: Vec<_> = REJECTED_BINDS
        .iter()
        .map(|count| count.load(Ordering::Relaxed))
        .collect();
    serde_json::json!({"connectExHooked":CONNECT_EX.get().is_some(),"connectExProviderQueries":EXTENSION_QUERIES.load(Ordering::Relaxed),"connectExLoopbackCalls":EXTENSION_CONNECTS.load(Ordering::Relaxed),"loopbackConnectCalls":CONNECTS.load(Ordering::Relaxed),"addressListQueries":ADDRESS_QUERIES.load(Ordering::Relaxed),"addressListSorts":ADDRESS_SORTS.load(Ordering::Relaxed),"dualStackLoopbackBinds":DUAL_STACK_BINDS.load(Ordering::Relaxed),"lanInterfaceBindTranslation":LAN_BIND_TRANSLATION.load(Ordering::Acquire),"translatedInterfaceBinds":TRANSLATED_BINDS.load(Ordering::Relaxed),"lastDeniedOperation":LAST_DENIED_IO.load(Ordering::Relaxed),"deniedOperations":denied,"rejectedBindsUnknownIpv4Ipv6":rejected_binds,"lastDeniedDevice":LAST_DENIED_NAME.lock().ok().and_then(|v|v.clone()),"helperLoopbackConnects":HELPER_CONNECTS.load(Ordering::Relaxed),"transport":LAST_TRANSPORT.lock().ok().map(|v|v.clone())})
}
type SendTo = unsafe extern "system" fn(usize, *const u8, i32, i32, *const u8, i32) -> i32;
type WsaSendTo = unsafe extern "system" fn(
    usize,
    *const c_void,
    u32,
    *mut u32,
    u32,
    *const u8,
    i32,
    *mut c_void,
    *mut c_void,
) -> i32;
type WsaConnect = unsafe extern "system" fn(
    usize,
    *const u8,
    i32,
    *const c_void,
    *mut c_void,
    *const c_void,
    *const c_void,
) -> i32;
type GetAddress = unsafe extern "system" fn(usize, *mut u8, *mut i32) -> i32;
type GetOption = unsafe extern "system" fn(usize, i32, i32, *mut i32, *mut i32) -> i32;
type RawProc = unsafe extern "system" fn() -> isize;
type SetError = unsafe extern "system" fn(i32);
struct Originals {
    bind: AddressCall,
    connect: AddressCall,
    sendto: SendTo,
    wsa_sendto: WsaSendTo,
    wsa_connect: WsaConnect,
    peer: GetAddress,
    local: GetAddress,
    error: SetError,
    option: GetOption,
}
static ORIGINALS: OnceLock<Originals> = OnceLock::new();
#[derive(Clone, Copy, Default)]
struct CheckedScope {
    socket: usize,
    helper: usize,
    operation: u32,
    address: [u8; 28],
    length: usize,
}
thread_local! {static SCOPE:Cell<CheckedScope>=const {Cell::new(CheckedScope{socket:0,helper:0,operation:0,address:[0;28],length:0})};}
struct Scope(CheckedScope);
impl Scope {
    fn new(socket: usize, operation: u32) -> Self {
        Self(SCOPE.with(|s| {
            s.replace(CheckedScope {
                socket,
                operation,
                ..Default::default()
            })
        }))
    }
    fn connect(socket: usize, address: &[u8]) -> Self {
        let mut checked = CheckedScope {
            socket,
            operation: 2,
            ..Default::default()
        };
        if matches!(address.len(), 16 | 28) && loopback(address) {
            checked.address[..address.len()].copy_from_slice(address);
            checked.length = address.len();
        }
        Self(SCOPE.with(|s| s.replace(checked)))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        SCOPE.with(|s| s.set(self.0));
    }
}
fn denied() -> i32 {
    super::NETWORK_DENIALS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    unsafe { (ORIGINALS.get().unwrap().error)(10013) };
    -1
}

fn family(bytes: &[u8]) -> Option<(usize, usize)> {
    match u16::from_ne_bytes(bytes.get(..2)?.try_into().ok()?) {
        2 if bytes.len() >= 16 => Some((4, 4)),
        23 if bytes.len() >= 28 => Some((8, 16)),
        _ => None,
    }
}
fn loopback(bytes: &[u8]) -> bool {
    match family(bytes) {
        Some((4, 4)) => bytes[4] == 127,
        Some((8, 16)) => {
            bytes[8..23].iter().all(|v| *v == 0) && bytes[23] == 1
                || bytes[8..18].iter().all(|v| *v == 0)
                    && bytes[18..20] == [255, 255]
                    && bytes[20] == 127
        }
        _ => false,
    }
}
fn address(pointer: *const u8, length: i32, binding: bool) -> Option<Vec<u8>> {
    if !(2..=128).contains(&length) {
        return None;
    }
    let mut bytes = vec![0u8; length as usize];
    crate::world_hooks::read(pointer as usize, &mut bytes).ok()?;
    let (offset, size) = family(&bytes)?;
    let wildcard = bytes[offset..offset + size].iter().all(|v| *v == 0);
    if binding && (wildcard || LAN_BIND_TRANSLATION.load(Ordering::Acquire) && !loopback(&bytes)) {
        let mapped = size == 16 && bytes[8..18] == [0; 10] && bytes[18..20] == [255; 2];
        bytes[offset..offset + size].fill(0);
        if size == 4 {
            bytes[offset] = 127;
            bytes[offset + 3] = 1;
        } else if mapped {
            bytes[18..24].copy_from_slice(&[255, 255, 127, 0, 0, 1]);
        } else {
            bytes[offset + 15] = 1;
        }
        if size == 16 {
            bytes[24..28].fill(0);
        }
        if !wildcard {
            TRANSLATED_BINDS.fetch_add(1, Ordering::Relaxed);
        }
    }
    loopback(&bytes).then_some(bytes)
}
fn socket_address_result(socket: usize, peer: bool) -> Option<bool> {
    let originals = ORIGINALS.get()?;
    let mut bytes = [0u8; 128];
    let mut length = 128;
    let call = if peer {
        originals.peer
    } else {
        originals.local
    };
    if (unsafe { call(socket, bytes.as_mut_ptr(), &mut length) }) != 0
        || !(2..=128).contains(&length)
    {
        return None;
    }
    Some(loopback(&bytes[..length as usize]))
}
fn socket_address(socket: usize, peer: bool) -> bool {
    socket_address_result(socket, peer).unwrap_or(false)
}
fn checked_helper_connect(scope: CheckedScope, bytes: &[u8]) -> bool {
    scope.operation == 2
        && scope.socket != 0
        && matches!(scope.length, 16 | 28)
        && bytes.len() == 24 + scope.length
        && bytes[8..16] == [0; 8]
        && usize::from_ne_bytes(bytes[16..24].try_into().unwrap()) == scope.socket
        && loopback(&scope.address[..scope.length])
        && bytes[24..] == scope.address[..scope.length]
}
fn socket_status_query(bytes: &[u8]) -> bool {
    // AFD_TL_IO_CONTROL_INFO: getsockopt(SO_ERROR/SO_TYPE/IPV6_V6ONLY),
    // with no input pointer/data. These read status and cannot start a send.
    bytes.len() == 32
        && u32::from_ne_bytes(bytes[0..4].try_into().unwrap()) == 2
        && matches!(
            (
                u32::from_ne_bytes(bytes[4..8].try_into().unwrap()),
                u32::from_ne_bytes(bytes[8..12].try_into().unwrap())
            ),
            (0xffff, 0x1007 | 0x1008) | (41, 27)
        )
        && bytes[12] == 1
        && bytes[16..32] == [0; 16]
}
fn loopback_socket_option(bytes: &[u8]) -> bool {
    if bytes.len() != 32 || bytes[12] != 1 {
        return false;
    }
    let kind = u32::from_ne_bytes(bytes[0..4].try_into().unwrap());
    let level = u32::from_ne_bytes(bytes[4..8].try_into().unwrap());
    let option = u32::from_ne_bytes(bytes[8..12].try_into().unwrap());
    kind == 1
        && matches!((level, option), (6, 1) | (0xffff, 8 | 0x1001 | 0x1002))
        && usize::from_ne_bytes(bytes[24..32].try_into().unwrap()) == 4
        && usize::from_ne_bytes(bytes[16..24].try_into().unwrap()) != 0
}
fn socket_setup_option(bytes: &[u8]) -> bool {
    // These boolean setsockopt calls configure an endpoint before bind or
    // connect. They send no data and authorize no destination. All subsequent
    // bind/connect/send operations still pass their own loopback checks.
    if bytes.len() != 32
        || u32::from_ne_bytes(bytes[..4].try_into().unwrap()) != 1
        || bytes[12] != 1
        || !matches!((
                u32::from_ne_bytes(bytes[4..8].try_into().unwrap()),
                u32::from_ne_bytes(bytes[8..12].try_into().unwrap())
            ), (0xffff, 32) | (41, 27)) // SO_BROADCAST, IPV6_V6ONLY
        || usize::from_ne_bytes(bytes[24..32].try_into().unwrap()) != 4
    {
        return false;
    }
    let pointer = usize::from_ne_bytes(bytes[16..24].try_into().unwrap());
    let mut value = [0; 4];
    crate::world_hooks::read(pointer, &mut value).is_ok()
        && matches!(u32::from_ne_bytes(value), 0 | 1)
}
fn socket_address_sort(bytes: &[u8]) -> bool {
    // SIO_ADDRESS_LIST_SORT only orders an address list. It cannot send data.
    // Check its precise transport header and bounded SOCKET_ADDRESS_LIST shape;
    // these addresses never authorize connect/send operations.
    if bytes.len() != 32
        || u32::from_ne_bytes(bytes[..4].try_into().unwrap()) != 3
        || u32::from_ne_bytes(bytes[4..8].try_into().unwrap()) != 0
        || u32::from_ne_bytes(bytes[8..12].try_into().unwrap()) != 0xc8000019
        || bytes[12] != 1
    {
        return false;
    }
    let pointer = usize::from_ne_bytes(bytes[16..24].try_into().unwrap());
    let length = usize::from_ne_bytes(bytes[24..32].try_into().unwrap());
    if !(24..=65536).contains(&length) {
        return false;
    }
    let mut list = vec![0; length];
    if crate::world_hooks::read(pointer, &mut list).is_err() {
        return false;
    }
    let count = u32::from_ne_bytes(list[..4].try_into().unwrap()) as usize;
    if !(1..=128).contains(&count) || 8 + count * 16 > length {
        return false;
    }
    (0..count).all(|i| {
        let entry = &list[8 + i * 16..8 + (i + 1) * 16];
        if u32::from_ne_bytes(entry[8..12].try_into().unwrap()) != 28 {
            return false;
        }
        let pointer = usize::from_ne_bytes(entry[..8].try_into().unwrap());
        let mut address = [0; 28];
        crate::world_hooks::read(pointer, &mut address).is_ok() && family(&address) == Some((8, 16))
    })
}
pub(super) fn allow_io(
    file: *mut c_void,
    code: u32,
    input: *const c_void,
    input_size: u32,
) -> bool {
    if code & 0xffff_f000 != 0x12000 {
        return false;
    }
    let operation = (code & 0xfff) >> 2;
    let socket = file as usize;
    let scope = SCOPE.with(Cell::get);
    // This OS also sends AFD_CONNECT through a provider helper handle. Its
    // request carries the actual socket at +16 and sockaddr at +24. Admit only
    // the bounded observed shape, exact socket and exact destination already
    // checked by the synchronous Winsock hook; unrelated AFD handles stay denied.
    let helper = operation == 1
        && scope.operation == 2
        && scope.length != 0
        && input_size as usize == 24 + scope.length
        && super::kernel_name(file).is_some_and(|name| name == super::wide("\\Device\\Afd"))
        && {
            let mut bytes = vec![0u8; input_size as usize];
            crate::world_hooks::read(input as usize, &mut bytes).is_ok()
                && checked_helper_connect(scope, &bytes)
        };
    if helper {
        HELPER_CONNECTS.fetch_add(1, Ordering::Relaxed);
        SCOPE.with(|s| {
            s.set(CheckedScope {
                helper: socket,
                ..scope
            })
        });
    }
    let status_query = operation == 47 && input_size == 32 && {
        let mut bytes = [0u8; 32];
        crate::world_hooks::read(input as usize, &mut bytes).is_ok()
            && ({
                let sort = socket_address_sort(&bytes);
                if sort {
                    ADDRESS_SORTS.fetch_add(1, Ordering::Relaxed);
                }
                sort || socket_status_query(&bytes)
                    || socket_setup_option(&bytes)
                    || loopback_socket_option(&bytes) && socket_address(socket, true)
            })
    };
    // AFD_ADDRESS_LIST_QUERY is read-only. Admit only the documented native
    // family argument, not arbitrary transport IOCTLs or address-change calls.
    let address_query = code == 0x120b3 && input_size == 2 && {
        let mut family = [0; 2];
        crate::world_hooks::read(input as usize, &mut family).is_ok()
            && matches!(u16::from_ne_bytes(family), 2 | 23)
    };
    if address_query {
        ADDRESS_QUERIES.fetch_add(1, Ordering::Relaxed);
    }
    let allowed = helper
        || status_query
        || address_query
        || match operation {
            0 => scope.socket == socket && (1..=3).contains(&scope.operation), // explicit/implicit bind
            // This installed OS also submits operation 47 from Winsock connect.
            // It is admitted only during that same socket's checked loopback call.
            1 => {
                (scope.socket == socket || scope.helper != 0 && scope.helper == socket)
                    && scope.operation == 2
            }
            47 | 49 => {
                (scope.socket == socket || scope.helper != 0 && scope.helper == socket)
                    && scope.operation == 2
            }
            8 => scope.socket == socket && scope.operation == 3, // every datagram needs its own destination check
            7 => socket_address(socket, true),
            2..=6 | 32 | 35 => socket_address(socket, true) || socket_address(socket, false),
            // Poll, shutdown and socket/context metadata do not initiate a new
            // connection or transmit application data. Unknown operations fail.
            9..=17 | 22..=25 | 30 | 33 | 34 | 41 | 42 => true,
            _ => false,
        };
    if !allowed {
        DENIED_OPERATIONS[(operation as usize).min(64)].fetch_add(1, Ordering::Relaxed);
        if operation == 47
            && input_size <= 64
            && let Ok(mut out) = LAST_TRANSPORT.try_lock()
        {
            let mut bytes = vec![0u8; input_size as usize];
            if bytes.len() == 32
                && out.len() < 32
                && crate::world_hooks::read(input as usize, &mut bytes).is_ok()
            {
                out.push(serde_json::json!({"type":u32::from_ne_bytes(bytes[..4].try_into().unwrap()),
                    "level":u32::from_ne_bytes(bytes[4..8].try_into().unwrap()),"option":u32::from_ne_bytes(bytes[8..12].try_into().unwrap()),
                    "endpoint":bytes[12],"input":usize::from_ne_bytes(bytes[24..32].try_into().unwrap()),"scoped":scope.operation}));
            }
        }
        LAST_DENIED_IO.store(operation as u64, Ordering::Relaxed);
        if let Ok(mut name) = LAST_DENIED_NAME.try_lock() {
            *name = super::kernel_name(file).map(|v| String::from_utf16_lossy(&v));
        }
    }
    allowed
}
unsafe extern "system" fn bind(socket: usize, to: *const u8, length: i32) -> i32 {
    let Some(mut local) = address(to, length, true) else {
        let mut bytes = [0; 2];
        let family = if length >= 2 && crate::world_hooks::read(to as usize, &mut bytes).is_ok() {
            match u16::from_ne_bytes(bytes) {
                2 => 1,
                23 => 2,
                _ => 0,
            }
        } else {
            0
        };
        REJECTED_BINDS[family].fetch_add(1, Ordering::Relaxed);
        return denied();
    };
    if family(&local) == Some((8, 16)) {
        let mut requested = [0; 28];
        let mut only_v6 = 1;
        let mut size = 4;
        if crate::world_hooks::read(to as usize, &mut requested).is_ok()
            && requested[8..24] == [0; 16]
            && unsafe { (ORIGINALS.get().unwrap().option)(socket, 41, 27, &mut only_v6, &mut size) }
                == 0
            && matches!(size, 1 | 4)
            && only_v6 == 0
        {
            // Binding a dual-stack wildcard to ::1 loses IPv4 reception.
            // Preserve its IPv4 capability on the mapped loopback endpoint.
            local[8..24].fill(0);
            local[18..24].copy_from_slice(&[255, 255, 127, 0, 0, 1]);
            DUAL_STACK_BINDS.fetch_add(1, Ordering::Relaxed);
        }
    }
    let _scope = Scope::new(socket, 1);
    unsafe { (ORIGINALS.get().unwrap().bind)(socket, local.as_ptr(), local.len() as i32) }
}
unsafe extern "system" fn connect(socket: usize, to: *const u8, length: i32) -> i32 {
    let Some(to) = address(to, length, false) else {
        return denied();
    };
    CONNECTS.fetch_add(1, Ordering::Relaxed);
    let _scope = Scope::connect(socket, &to);
    unsafe { (ORIGINALS.get().unwrap().connect)(socket, to.as_ptr(), to.len() as i32) }
}
unsafe extern "system" fn connect_ex(
    socket: usize,
    to: *const u8,
    length: i32,
    data: *const c_void,
    size: u32,
    sent: *mut u32,
    overlapped: *mut c_void,
) -> i32 {
    let Some(to) = address(to, length, false) else {
        denied();
        return 0; // BOOL, unlike Winsock's SOCKET_ERROR return
    };
    let Some(original) = CONNECT_EX.get() else {
        denied();
        return 0;
    };
    let original: ConnectEx = unsafe { std::mem::transmute(*original) };
    EXTENSION_CONNECTS.fetch_add(1, Ordering::Relaxed);
    let _scope = Scope::connect(socket, &to);
    unsafe {
        original(
            socket,
            to.as_ptr(),
            to.len() as i32,
            data,
            size,
            sent,
            overlapped,
        )
    }
}
unsafe extern "system" fn wsa_connect(
    socket: usize,
    to: *const u8,
    length: i32,
    caller: *const c_void,
    callee: *mut c_void,
    sqos: *const c_void,
    gqos: *const c_void,
) -> i32 {
    let Some(to) = address(to, length, false) else {
        return denied();
    };
    CONNECTS.fetch_add(1, Ordering::Relaxed);
    let _scope = Scope::connect(socket, &to);
    unsafe {
        (ORIGINALS.get().unwrap().wsa_connect)(
            socket,
            to.as_ptr(),
            to.len() as i32,
            caller,
            callee,
            sqos,
            gqos,
        )
    }
}
fn destination(socket: usize, to: *const u8, length: i32) -> Option<Option<Vec<u8>>> {
    // Winsock ignores lpTo for connected streams. A supplied loopback address
    // must therefore never authorize sending on an existing remote stream.
    match socket_address_result(socket, true) {
        Some(false) => return None,
        None => {
            let mut kind = 0i32;
            let mut size = 4i32;
            if unsafe { (ORIGINALS.get()?.option)(socket, 0xffff, 0x1008, &mut kind, &mut size) }
                != 0
                || size != 4
                || kind != 2
            {
                return None;
            }
        }
        Some(true) => {}
    }
    if to.is_null() {
        socket_address(socket, true).then_some(None)
    } else {
        if let Some(local) = address(to, length, false) {
            return Some(Some(local));
        }
        if !matches!(length, 16 | 28) {
            return None;
        }
        let mut local = vec![0u8; length as usize];
        crate::world_hooks::read(to as usize, &mut local).ok()?;
        if !translate_replay_discovery(&mut local, REPLAY_DISCOVERY.load(Ordering::Acquire)) {
            return None;
        }
        DISCOVERY_TRANSLATIONS.fetch_add(1, Ordering::Relaxed);
        Some(Some(local))
    }
}
fn translate_replay_discovery(bytes: &mut [u8], enabled: bool) -> bool {
    // Isolated replay discovery reaches only our other loopback interface.
    // This never permits an actual broadcast or changes connect destinations.
    if !enabled
        || bytes.len() < 8
        || !matches!(u16::from_be_bytes([bytes[2], bytes[3]]), 19132 | 7551)
    {
        return false;
    }
    let offset = match family(bytes) {
        Some((4, 4)) if bytes.len() == 16 => 4,
        Some((8, 16))
            if bytes.len() == 28 && bytes[8..20] == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 255, 255] =>
        {
            20
        }
        _ => return false,
    };
    if bytes[offset..offset + 4] != [255; 4] {
        return false;
    }
    bytes[offset..offset + 4].copy_from_slice(&[127, 0, 0, 2]);
    loopback(bytes)
}
unsafe extern "system" fn sendto(
    socket: usize,
    data: *const u8,
    bytes: i32,
    flags: i32,
    to: *const u8,
    length: i32,
) -> i32 {
    let Some(address) = destination(socket, to, length) else {
        return denied();
    };
    let _scope = Scope::new(socket, 3);
    let (to, length) = address
        .as_ref()
        .map(|a| (a.as_ptr(), a.len() as i32))
        .unwrap_or((std::ptr::null(), 0));
    unsafe { (ORIGINALS.get().unwrap().sendto)(socket, data, bytes, flags, to, length) }
}
unsafe extern "system" fn wsa_sendto(
    socket: usize,
    buffers: *const c_void,
    count: u32,
    sent: *mut u32,
    flags: u32,
    to: *const u8,
    length: i32,
    overlapped: *mut c_void,
    completion: *mut c_void,
) -> i32 {
    let Some(address) = destination(socket, to, length) else {
        return denied();
    };
    let _scope = Scope::new(socket, 3);
    let (to, length) = address
        .as_ref()
        .map(|a| (a.as_ptr(), a.len() as i32))
        .unwrap_or((std::ptr::null(), 0));
    unsafe {
        (ORIGINALS.get().unwrap().wsa_sendto)(
            socket, buffers, count, sent, flags, to, length, overlapped, completion,
        )
    }
}
pub(super) fn prepare() -> Result<Vec<usize>, String> {
    unsafe {
        let module = LoadLibraryW(w!("ws2_32.dll")).map_err(|e| e.to_string())?;
        let mut targets = Vec::new();
        let mut originals = Vec::new();
        // Query the real extension before enabling any guards, with the owned
        // game's entry thread still held. This creates only a temporary local
        // socket and never attempts a connection. Hooking the implementation
        // also covers pointers cached internally by WinHTTP/provider code.
        use std::os::windows::io::AsRawSocket;
        let socket = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
        let query: Ioctl = std::mem::transmute::<RawProc, Ioctl>(
            GetProcAddress(module, s!("WSAIoctl")).ok_or("Winsock extension query unavailable")?,
        );
        let mut extension = 0usize;
        let mut used = 0;
        EXTENSION_QUERIES.fetch_add(1, Ordering::Relaxed);
        if query(
            socket.as_raw_socket() as usize,
            0xc8000006,
            (&CONNECT_EX_ID as *const GUID).cast(),
            16,
            (&mut extension as *mut usize).cast(),
            8,
            &mut used,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) != 0
            || used != 8
            || extension == 0
        {
            return Err("ConnectEx provider discovery failed".into());
        }
        let mut original = std::ptr::null_mut();
        let result = MH_CreateHook(extension as _, connect_ex as *const () as _, &mut original);
        if result != MH_STATUS::MH_OK {
            return Err(format!("create ConnectEx guard: {result:?}"));
        }
        CONNECT_EX
            .set(original as usize)
            .map_err(|_| "ConnectEx guard already initialized")?;
        targets.push(extension);
        drop(socket);
        for (name, callback) in [
            (s!("bind"), bind as *const () as usize),
            (s!("connect"), connect as *const () as usize),
            (s!("sendto"), sendto as *const () as usize),
            (s!("WSASendTo"), wsa_sendto as *const () as usize),
            (s!("WSAConnect"), wsa_connect as *const () as usize),
        ] {
            let target = GetProcAddress(module, name).ok_or("Winsock probe API unavailable")?
                as *const () as usize;
            let mut original = std::ptr::null_mut();
            let result = MH_CreateHook(target as _, callback as _, &mut original);
            if result != MH_STATUS::MH_OK {
                return Err(format!("create loopback guard: {result:?}"));
            }
            targets.push(target);
            originals.push(original as usize);
        }
        ORIGINALS
            .set(Originals {
                bind: std::mem::transmute::<usize, AddressCall>(originals[0]),
                connect: std::mem::transmute::<usize, AddressCall>(originals[1]),
                sendto: std::mem::transmute::<usize, SendTo>(originals[2]),
                wsa_sendto: std::mem::transmute::<usize, WsaSendTo>(originals[3]),
                wsa_connect: std::mem::transmute::<usize, WsaConnect>(originals[4]),
                peer: std::mem::transmute::<RawProc, GetAddress>(
                    GetProcAddress(module, s!("getpeername"))
                        .ok_or("Winsock peer query unavailable")?,
                ),
                local: std::mem::transmute::<RawProc, GetAddress>(
                    GetProcAddress(module, s!("getsockname"))
                        .ok_or("Winsock bind query unavailable")?,
                ),
                error: std::mem::transmute::<RawProc, SetError>(
                    GetProcAddress(module, s!("WSASetLastError"))
                        .ok_or("Winsock error API unavailable")?,
                ),
                option: std::mem::transmute::<RawProc, GetOption>(
                    GetProcAddress(module, s!("getsockopt"))
                        .ok_or("Winsock socket type query unavailable")?,
                ),
            })
            .map_err(|_| "loopback guard already initialized")?;
        Ok(targets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_discovery_changes_only_the_exact_broadcast_and_preserves_other_destinations() {
        let mut destination = [0u8; 16];
        destination[..2].copy_from_slice(&2u16.to_ne_bytes());
        destination[2..4].copy_from_slice(&19132u16.to_be_bytes());
        destination[4..8].fill(255);
        let original = destination;
        assert!(!translate_replay_discovery(&mut destination, false));
        assert_eq!(destination, original);
        assert!(translate_replay_discovery(&mut destination, true));
        assert_eq!(&destination[4..8], &[127, 0, 0, 2]);
        assert!(loopback(&destination));
        for (host, port) in [
            ([255; 4], 19133u16),
            ([192, 0, 2, 1], 19132),
            ([224, 0, 0, 1], 19132),
        ] {
            let mut other = original;
            other[4..8].copy_from_slice(&host);
            other[2..4].copy_from_slice(&port.to_be_bytes());
            let before = other;
            assert!(!translate_replay_discovery(&mut other, true));
            assert_eq!(other, before);
        }
        let mut mapped = [0u8; 28];
        mapped[..2].copy_from_slice(&23u16.to_ne_bytes());
        mapped[2..4].copy_from_slice(&19132u16.to_be_bytes());
        mapped[18..24].fill(255);
        assert!(translate_replay_discovery(&mut mapped, true));
        assert_eq!(&mapped[18..24], &[255, 255, 127, 0, 0, 2]);
        assert!(loopback(&mapped));
    }
    #[test]
    fn helper_connect_requires_exact_checked_socket_address_and_shape() {
        let mut scope = CheckedScope {
            socket: 0x1234,
            operation: 2,
            length: 16,
            ..Default::default()
        };
        scope.address[..2].copy_from_slice(&2u16.to_ne_bytes());
        scope.address[2..8].copy_from_slice(&[0x12, 0x34, 127, 0, 0, 1]);
        let mut request = vec![0u8; 40];
        request[16..24].copy_from_slice(&scope.socket.to_ne_bytes());
        request[24..].copy_from_slice(&scope.address[..16]);
        assert!(checked_helper_connect(scope, &request));
        assert!(!checked_helper_connect(
            CheckedScope {
                socket: 0x1235,
                ..scope
            },
            &request
        ));
        assert!(!checked_helper_connect(
            CheckedScope {
                operation: 3,
                ..scope
            },
            &request
        ));
        assert!(!checked_helper_connect(scope, &request[..39]));
        request[26] ^= 1;
        assert!(!checked_helper_connect(scope, &request));
        request[26] ^= 1;
        request[8] = 1;
        assert!(!checked_helper_connect(scope, &request));
        request[8] = 0;
        scope.address[4] = 192;
        request[28] = 192;
        assert!(!checked_helper_connect(scope, &request));
    }
    #[test]
    fn transport_options_do_not_authorize_general_ioctls() {
        let mut query = [0u8; 32];
        query[..4].copy_from_slice(&2u32.to_ne_bytes());
        query[4..8].copy_from_slice(&0xffffu32.to_ne_bytes());
        query[8..12].copy_from_slice(&0x1007u32.to_ne_bytes());
        query[12] = 1;
        assert!(socket_status_query(&query));
        query[16] = 1;
        assert!(!socket_status_query(&query));
        query[16] = 0;
        query[8] = 1;
        assert!(!socket_status_query(&query));
        query[..4].copy_from_slice(&1u32.to_ne_bytes());
        query[4..8].copy_from_slice(&6u32.to_ne_bytes());
        query[8..12].copy_from_slice(&1u32.to_ne_bytes());
        query[16] = 1;
        query[24] = 4;
        assert!(loopback_socket_option(&query));
        query[8] = 2;
        assert!(!loopback_socket_option(&query));
        query[8] = 1;
        query[24] = 16;
        assert!(!loopback_socket_option(&query));
    }
    #[test]
    fn address_sort_rejects_unbounded_lists_and_other_transport_commands() {
        let mut address = [0u8; 28];
        address[..2].copy_from_slice(&23u16.to_ne_bytes());
        address[23] = 1;
        let mut list = [0u8; 24];
        list[..4].copy_from_slice(&1u32.to_ne_bytes());
        list[8..16].copy_from_slice(&(address.as_ptr() as usize).to_ne_bytes());
        list[16..20].copy_from_slice(&28u32.to_ne_bytes());
        let mut header = [0u8; 32];
        header[..4].copy_from_slice(&3u32.to_ne_bytes());
        header[8..12].copy_from_slice(&0xc8000019u32.to_ne_bytes());
        header[12] = 1;
        header[16..24].copy_from_slice(&(list.as_ptr() as usize).to_ne_bytes());
        header[24..32].copy_from_slice(&24usize.to_ne_bytes());
        assert!(socket_address_sort(&header));
        header[8..12].copy_from_slice(&0xc8000018u32.to_ne_bytes());
        assert!(!socket_address_sort(&header));
        header[8..12].copy_from_slice(&0xc8000019u32.to_ne_bytes());
        list[..4].copy_from_slice(&129u32.to_ne_bytes());
        assert!(!socket_address_sort(&header));
        list[..4].copy_from_slice(&2u32.to_ne_bytes());
        assert!(!socket_address_sort(&header));
        list[..4].copy_from_slice(&1u32.to_ne_bytes());
        list[8..16].fill(0);
        assert!(!socket_address_sort(&header));
        header[24..32].copy_from_slice(&65537usize.to_ne_bytes());
        assert!(!socket_address_sort(&header));
    }
    #[test]
    fn setup_options_accept_only_boolean_broadcast_and_ipv6_configuration() {
        let mut query = [0u8; 32];
        let mut value = 1u32;
        query[..4].copy_from_slice(&1u32.to_ne_bytes());
        query[4..8].copy_from_slice(&41u32.to_ne_bytes());
        query[8..12].copy_from_slice(&27u32.to_ne_bytes());
        query[12] = 1;
        query[16..24].copy_from_slice(&((&mut value as *mut u32) as usize).to_ne_bytes());
        query[24..32].copy_from_slice(&4usize.to_ne_bytes());
        assert!(socket_setup_option(&query));
        query[4..8].copy_from_slice(&0xffffu32.to_ne_bytes());
        query[8..12].copy_from_slice(&32u32.to_ne_bytes());
        assert!(socket_setup_option(&query));
        query[..4].copy_from_slice(&3u32.to_ne_bytes());
        assert!(!socket_setup_option(&query));
        query[..4].copy_from_slice(&1u32.to_ne_bytes());
        query[8..12].copy_from_slice(&0xc8000019u32.to_ne_bytes());
        assert!(!socket_setup_option(&query));
        query[8..12].copy_from_slice(&32u32.to_ne_bytes());
        query[24] = 88;
        assert!(!socket_setup_option(&query));
        query[24] = 4;
        query[16..24].fill(0);
        assert!(!socket_setup_option(&query));
    }
    #[test]
    fn only_loopback_addresses_and_owned_wildcard_binds_are_accepted() {
        let mut v4 = [0u8; 16];
        v4[..2].copy_from_slice(&2u16.to_ne_bytes());
        assert!(address(v4.as_ptr(), 16, false).is_none());
        let bound = address(v4.as_ptr(), 16, true).unwrap();
        assert_eq!(&bound[4..8], &[127, 0, 0, 1]);
        v4[4..8].copy_from_slice(&[192, 0, 2, 1]);
        assert!(address(v4.as_ptr(), 16, true).is_none());
        v4[4..8].copy_from_slice(&[127, 24, 42, 1]);
        assert!(address(v4.as_ptr(), 16, false).is_some());
        let mut v6 = [0u8; 28];
        v6[..2].copy_from_slice(&23u16.to_ne_bytes());
        assert!(address(v6.as_ptr(), 28, false).is_none());
        assert!(address(v6.as_ptr(), 28, true).is_some());
        v6[23] = 1;
        assert!(address(v6.as_ptr(), 28, false).is_some());
        v6[8] = 32;
        assert!(address(v6.as_ptr(), 28, false).is_none());
        v6[8] = 0;
        v6[18..24].copy_from_slice(&[255, 255, 127, 0, 0, 1]);
        assert!(address(v6.as_ptr(), 28, false).is_some());
        v6[20] = 192;
        assert!(address(v6.as_ptr(), 28, false).is_none());
        assert!(address(v4.as_ptr(), 2, false).is_none());
        assert!(address(std::ptr::null(), 16, false).is_none());
    }
}
