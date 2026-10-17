#![no_std]
#![no_main]

use aya_ebpf::helpers::{
    bpf_get_current_pid_tgid, bpf_get_current_uid_gid, bpf_ktime_get_ns,
    bpf_probe_read_kernel,
};
use aya_ebpf::macros::{kprobe, kretprobe, map, tracepoint};
use aya_ebpf::maps::LruHashMap;
use aya_ebpf::programs::{ProbeContext, RetProbeContext, TracePointContext};
use ebpf::{
    AF_INET, AF_INET6, Endpoint, Flow, FlowKey, IPPROTO_ICMP, IPPROTO_ICMPV6,
    IPPROTO_RAW, IPPROTO_TCP, IPPROTO_UDP,
};

// LRU in case of overflow
#[map]
static FLOWS: LruHashMap<FlowKey, Flow> = LruHashMap::with_max_entries(32768, 0);
#[map]
static TCP_SENDS: LruHashMap<u32, FlowKey> = LruHashMap::with_max_entries(1024, 0);
#[map]
static TCP_RECVS: LruHashMap<u32, FlowKey> = LruHashMap::with_max_entries(1024, 0);
#[map]
static DATAGRAM_SENDS: LruHashMap<u32, SendCall> = LruHashMap::with_max_entries(1024, 0);
#[map]
static DATAGRAM_RECVS: LruHashMap<u32, RecvCall> = LruHashMap::with_max_entries(1024, 0);

#[derive(Clone, Copy)]
struct SendCall {
    key: FlowKey,
    family: u16,
}

#[derive(Clone, Copy)]
struct RecvCall {
    sk: u64,
    msg: u64,
}

const RAW: u8 = 0;

// struct sock_common (include/net/sock.h)
const SKC_DADDR: usize = 0;
const SKC_RCV_SADDR: usize = 4;
const SKC_DPORT: usize = 12; // network byte order
const SKC_NUM: usize = 14; // local port
const SKC_FAMILY: usize = 16;
const SKC_STATE: usize = 18;
const SKC_V6_DADDR: usize = 56;
const SKC_V6_RCV_SADDR: usize = 72;

#[kprobe]
pub fn tcp_connect(ctx: ProbeContext) -> u32 {
    if let Some(sk) = ctx.arg(0) {
        record_tcp_socket(sk);
    }
    0
}

#[kprobe]
pub fn tcp_sendmsg_enter(ctx: ProbeContext) -> u32 {
    if let Some(key) = ctx.arg(0).and_then(record_tcp_socket) {
        let _ = TCP_SENDS.insert(&current_tid(), &key, 0);
    }
    0
}

#[kretprobe]
pub fn tcp_sendmsg_exit(ctx: RetProbeContext) -> u32 {
    if let (Some(key), Some(bytes)) = (take(&TCP_SENDS), bytes_moved(&ctx)) {
        add_traffic(&key, Traffic::Sent(bytes));
    }
    0
}

#[kprobe]
pub fn tcp_recvmsg_enter(ctx: ProbeContext) -> u32 {
    if let Some(key) = ctx.arg(0).and_then(record_tcp_socket) {
        let _ = TCP_RECVS.insert(&current_tid(), &key, 0);
    }
    0
}

#[kretprobe]
pub fn tcp_recvmsg_exit(ctx: RetProbeContext) -> u32 {
    if let (Some(key), Some(bytes)) = (take(&TCP_RECVS), bytes_moved(&ctx)) {
        add_traffic(&key, Traffic::Received(bytes));
    }
    0
}

#[inline(always)]
fn record_tcp_socket(sk: *const u8) -> Option<FlowKey> {
    let (local, remote) = socket_ends(sk)?;
    let key = FlowKey::new(0, IPPROTO_TCP, local, remote);
    let mut flow = get_flow(&key);
    claim(&mut flow);
    if let Some(state) = read(sk, SKC_STATE) {
        flow.state = state;
    }
    save_flow(&key, &flow);
    Some(key)
}

// sys/kernel/tracing/events/sock/inet_sock_set_state/format
const TP_OLDSTATE: usize = 16;
const TP_NEWSTATE: usize = 20;
const TP_SPORT: usize = 24;
const TP_DPORT: usize = 26;
const TP_FAMILY: usize = 28;
const TP_PROTOCOL: usize = 30;
const TP_SADDR: usize = 32;
const TP_DADDR: usize = 36;
const TP_SADDR_V6: usize = 40;
const TP_DADDR_V6: usize = 56;

const TCP_SYN_SENT: i32 = 2;
const TCP_LISTEN: i32 = 10;

#[tracepoint]
pub fn inet_sock_set_state(ctx: TracePointContext) -> u32 {
    let _ = record_state_change(&ctx);
    0
}

#[inline(always)]
fn record_state_change(ctx: &TracePointContext) -> Option<()> {
    let protocol: u16 = field(ctx, TP_PROTOCOL)?;
    if protocol != IPPROTO_TCP as u16 {
        return None;
    }
    let old_state: i32 = field(ctx, TP_OLDSTATE)?;
    let state: i32 = field(ctx, TP_NEWSTATE)?;

    if old_state == TCP_LISTEN || state == TCP_LISTEN || state == TCP_SYN_SENT {
        return None;
    }

    let family: u16 = field(ctx, TP_FAMILY)?;
    let mut local = Endpoint { family, ip: [0; 16], port: field(ctx, TP_SPORT)? };
    let mut remote = Endpoint { family, ip: [0; 16], port: field(ctx, TP_DPORT)? };
    match family {
        AF_INET => {
            local.ip[..4].copy_from_slice(&field::<[u8; 4]>(ctx, TP_SADDR)?);
            remote.ip[..4].copy_from_slice(&field::<[u8; 4]>(ctx, TP_DADDR)?);
        }
        AF_INET6 => {
            local.ip = field(ctx, TP_SADDR_V6)?;
            remote.ip = field(ctx, TP_DADDR_V6)?;
        }
        _ => return None,
    }

    let key = FlowKey::new(0, IPPROTO_TCP, local, remote);
    let mut flow = get_flow(&key);
    flow.state = state as u8;
    save_flow(&key, &flow);
    Some(())
}

#[kprobe]
pub fn udp_sendmsg_enter(ctx: ProbeContext) -> u32 {
    let _ = start_send(&ctx, AF_INET, IPPROTO_UDP);
    0
}

#[kretprobe]
pub fn udp_sendmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_send(&ctx, AF_INET);
    0
}

#[kprobe]
pub fn udpv6_sendmsg_enter(ctx: ProbeContext) -> u32 {
    let _ = start_send(&ctx, AF_INET6, IPPROTO_UDP);
    0
}

#[kretprobe]
pub fn udpv6_sendmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_send(&ctx, AF_INET6);
    0
}

#[kprobe]
pub fn raw_sendmsg_enter(ctx: ProbeContext) -> u32 {
    let _ = start_send(&ctx, AF_INET, RAW);
    0
}

#[kretprobe]
pub fn raw_sendmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_send(&ctx, AF_INET);
    0
}

#[kprobe]
pub fn rawv6_sendmsg_enter(ctx: ProbeContext) -> u32 {
    let _ = start_send(&ctx, AF_INET6, RAW);
    0
}

#[kretprobe]
pub fn rawv6_sendmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_send(&ctx, AF_INET6);
    0
}

#[kprobe]
pub fn ping_v4_sendmsg_enter(ctx: ProbeContext) -> u32 {
    let _ = start_send(&ctx, AF_INET, IPPROTO_ICMP);
    0
}

#[kretprobe]
pub fn ping_v4_sendmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_send(&ctx, AF_INET);
    0
}

#[kprobe]
pub fn ping_v6_sendmsg_enter(ctx: ProbeContext) -> u32 {
    let _ = start_send(&ctx, AF_INET6, IPPROTO_ICMPV6);
    0
}

#[kretprobe]
pub fn ping_v6_sendmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_send(&ctx, AF_INET6);
    0
}

#[kprobe]
pub fn udp_recvmsg_enter(ctx: ProbeContext) -> u32 {
    start_receive(&ctx);
    0
}

#[kretprobe]
pub fn udp_recvmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_receive(&ctx, IPPROTO_UDP);
    0
}

#[kprobe]
pub fn udpv6_recvmsg_enter(ctx: ProbeContext) -> u32 {
    start_receive(&ctx);
    0
}

#[kretprobe]
pub fn udpv6_recvmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_receive(&ctx, IPPROTO_UDP);
    0
}

#[kprobe]
pub fn raw_recvmsg_enter(ctx: ProbeContext) -> u32 {
    start_receive(&ctx);
    0
}

#[kretprobe]
pub fn raw_recvmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_receive(&ctx, RAW);
    0
}

#[kprobe]
pub fn rawv6_recvmsg_enter(ctx: ProbeContext) -> u32 {
    start_receive(&ctx);
    0
}

#[kretprobe]
pub fn rawv6_recvmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_receive(&ctx, RAW);
    0
}

#[kprobe]
pub fn ping_recvmsg_enter(ctx: ProbeContext) -> u32 {
    start_receive(&ctx);
    0
}

#[kretprobe]
pub fn ping_recvmsg_exit(ctx: RetProbeContext) -> u32 {
    let _ = finish_receive(&ctx, IPPROTO_ICMP);
    0
}

/// sendmsg(sk, msg, len)
#[inline(always)]
fn start_send(ctx: &ProbeContext, family: u16, protocol: u8) -> Option<()> {
    let sk: *const u8 = ctx.arg(0)?;
    let msg: *const u8 = ctx.arg(1)?;
    if read::<u16>(sk, SKC_FAMILY)? != family {
        return None;
    }
    let tid = current_tid();
    let ends = datagram_ends(sk, msg, false);
    let (Some((local, remote)), Some(protocol)) = (ends, socket_protocol(sk, protocol))
    else {
        // a call whose return was missed would otherwise be counted at this one's
        let _ = DATAGRAM_SENDS.remove(&tid);
        return None;
    };
    let key = FlowKey::new(sk as u64, protocol, local, remote);
    let _ = DATAGRAM_SENDS.insert(&tid, &SendCall { key, family }, 0);
    Some(())
}

#[inline(always)]
fn finish_send(ctx: &RetProbeContext, family: u16) -> Option<()> {
    let tid = current_tid();
    let call = unsafe { DATAGRAM_SENDS.get(&tid) }.copied()?;
    if call.family != family {
        // udp_sendmsg returning inside udpv6_sendmsg
        return None;
    }
    let _ = DATAGRAM_SENDS.remove(&tid);
    let bytes = bytes_moved(ctx)?;
    add_datagram(call.key, Traffic::Sent(bytes))
}

#[inline(always)]
fn start_receive(ctx: &ProbeContext) {
    if let (Some(sk), Some(msg)) = (ctx.arg(0), ctx.arg(1)) {
        let _ = DATAGRAM_RECVS.insert(&current_tid(), &RecvCall { sk, msg }, 0);
    }
}

#[inline(always)]
fn finish_receive(ctx: &RetProbeContext, protocol: u8) -> Option<()> {
    let call = take(&DATAGRAM_RECVS)?;
    let bytes = bytes_moved(ctx)?;
    let sk = call.sk as *const u8;
    let (local, remote) = datagram_ends(sk, call.msg as *const u8, true)?;
    let protocol = socket_protocol(sk, protocol)?;
    add_datagram(FlowKey::new(call.sk, protocol, local, remote), Traffic::Received(bytes))
}

#[inline(always)]
fn socket_protocol(sk: *const u8, protocol: u8) -> Option<u8> {
    if protocol != RAW {
        return Some(protocol);
    }
    let raw: u16 = read(sk, SKC_NUM)?;

    Some(match raw as u8 {
        IPPROTO_TCP | IPPROTO_UDP => IPPROTO_RAW,
        protocol => protocol,
    })
}

#[inline(always)]
fn add_datagram(key: FlowKey, traffic: Traffic) -> Option<()> {
    if current_pid() == 0 {
        return None;
    }
    add_traffic(&key, traffic);
    Some(())
}

#[inline(always)]
fn datagram_ends(
    sk: *const u8,
    msg: *const u8,
    receiving: bool,
) -> Option<(Endpoint, Endpoint)> {
    let (local, remote) = socket_ends(sk)?;
    if !is_unspecified(&remote.ip) {
        return Some((local, remote));
    }
    match message_address(msg) {
        Some(address) if !is_unspecified(&address.ip) => Some((local, address)),
        _ if receiving => Some((local, remote)),
        _ => None,
    }
}

/// msg->msg_name
#[inline(always)]
fn message_address(msg: *const u8) -> Option<Endpoint> {
    let name: *const u8 = read(msg, 0)?;
    let family: u16 = read(name, 0)?;
    let port = u16::from_be(read(name, 2)?);
    let mut ip = [0; 16];
    match family {
        AF_INET => ip[..4].copy_from_slice(&read::<[u8; 4]>(name, 4)?),
        AF_INET6 => ip = read(name, 8)?,
        _ => return None,
    }
    Some(Endpoint { family, ip, port })
}

enum Traffic {
    Sent(u64),
    Received(u64),
}

#[inline(always)]
fn add_traffic(key: &FlowKey, traffic: Traffic) {
    let mut flow = get_flow(key);
    match traffic {
        Traffic::Sent(bytes) => flow.tx_bytes += bytes,
        Traffic::Received(bytes) => flow.rx_bytes += bytes,
    }
    claim(&mut flow);
    flow.last_seen = now();
    save_flow(key, &flow);
}

/// makes the current process the owner
#[inline(always)]
fn claim(flow: &mut Flow) {
    let pid = current_pid();
    if pid != 0 {
        flow.pid = pid;
        flow.uid = bpf_get_current_uid_gid() as u32;
    }
}

#[inline(always)]
fn get_flow(key: &FlowKey) -> Flow {
    match unsafe { FLOWS.get(key) } {
        Some(flow) => *flow,
        None => Flow { last_seen: now(), ..Flow::default() },
    }
}

#[inline(always)]
fn save_flow(key: &FlowKey, flow: &Flow) {
    let _ = FLOWS.insert(key, flow, 0);
}

#[inline(always)]
fn socket_ends(sk: *const u8) -> Option<(Endpoint, Endpoint)> {
    let family: u16 = read(sk, SKC_FAMILY)?;
    let mut local = Endpoint { family, ip: [0; 16], port: read(sk, SKC_NUM)? };
    let mut remote =
        Endpoint { family, ip: [0; 16], port: u16::from_be(read(sk, SKC_DPORT)?) };
    match family {
        AF_INET => {
            local.ip[..4].copy_from_slice(&read::<[u8; 4]>(sk, SKC_RCV_SADDR)?);
            remote.ip[..4].copy_from_slice(&read::<[u8; 4]>(sk, SKC_DADDR)?);
        }
        AF_INET6 => {
            local.ip = read(sk, SKC_V6_RCV_SADDR)?;
            remote.ip = read(sk, SKC_V6_DADDR)?;
        }
        _ => return None,
    }
    Some((local, remote))
}

#[inline(always)]
fn take<V: Copy>(map: &LruHashMap<u32, V>) -> Option<V> {
    let tid = current_tid();
    let value = unsafe { map.get(&tid) }.copied()?;
    let _ = map.remove(&tid);
    Some(value)
}

#[inline(always)]
fn bytes_moved(ctx: &RetProbeContext) -> Option<u64> {
    let ret: i32 = ctx.ret()?;
    (ret > 0).then_some(ret as u64)
}

#[inline(always)]
fn read<T>(base: *const u8, offset: usize) -> Option<T> {
    unsafe { bpf_probe_read_kernel(base.wrapping_add(offset) as *const T) }.ok()
}

#[inline(always)]
fn field<T>(ctx: &TracePointContext, offset: usize) -> Option<T> {
    unsafe { ctx.read_at(offset) }.ok()
}

#[inline(always)]
fn is_unspecified(ip: &[u8; 16]) -> bool {
    u128::from_ne_bytes(*ip) == 0
}

#[inline(always)]
fn now() -> u64 {
    unsafe { bpf_ktime_get_ns() }
}

#[inline(always)]
fn current_pid() -> u32 {
    (bpf_get_current_pid_tgid() >> 32) as u32
}

#[inline(always)]
fn current_tid() -> u32 {
    bpf_get_current_pid_tgid() as u32
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
