#![no_std]

pub const AF_INET: u16 = 2;
pub const AF_INET6: u16 = 10;

pub const IPPROTO_ICMP: u8 = 1;
pub const IPPROTO_TCP: u8 = 6;
pub const IPPROTO_UDP: u8 = 17;
pub const IPPROTO_ICMPV6: u8 = 58;
pub const IPPROTO_RAW: u8 = 255;

#[derive(Clone, Copy)]
pub struct Endpoint {
    pub family: u16,
    pub ip: [u8; 16],
    pub port: u16,
}

impl Endpoint {
    /// reverts an ipv6 mapped ipv4 address
    #[inline(always)]
    pub fn unmapped(self) -> Endpoint {
        let ip = self.ip;
        if self.family == AF_INET6
            && ip[..10] == [0; 10]
            && ip[10] == 0xff
            && ip[11] == 0xff
        {
            let mut v4 = [0; 16];
            v4[..4].copy_from_slice(&ip[12..]);
            return Endpoint { family: AF_INET, ip: v4, port: self.port };
        }
        self
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FlowKey {
    pub sk: u64,
    pub src_ip: [u8; 16],
    pub dst_ip: [u8; 16],
    pub src_port: u16,
    pub dst_port: u16,
    pub family: u16,
    pub protocol: u8,
    pub _pad: u8,
}

impl FlowKey {
    #[inline(always)]
    pub fn new(sk: u64, protocol: u8, local: Endpoint, remote: Endpoint) -> FlowKey {
        let remote = remote.unmapped();
        let mut local = local.unmapped();
        if local.family != remote.family {
            local = Endpoint { family: remote.family, ip: [0; 16], port: local.port };
        }

        let mut key = FlowKey {
            sk,
            src_ip: local.ip,
            dst_ip: remote.ip,
            src_port: local.port,
            dst_port: remote.port,
            family: remote.family,
            protocol,
            _pad: 0,
        };
        if protocol != IPPROTO_TCP && protocol != IPPROTO_UDP {
            key.src_port = 0;
            key.dst_port = 0;
        }
        if protocol == IPPROTO_ICMP || protocol == IPPROTO_ICMPV6 {
            key.protocol =
                if key.family == AF_INET { IPPROTO_ICMP } else { IPPROTO_ICMPV6 };
        }
        key
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Flow {
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    // bpf_ktime_get_ns() of the last traffic
    pub last_seen: u64,
    pub pid: u32,
    // of the process `pid`
    pub uid: u32,
    pub state: u8,
    pub _pad: [u8; 7],
}

#[cfg(feature = "user")]
unsafe impl aya::Pod for FlowKey {}

#[cfg(feature = "user")]
unsafe impl aya::Pod for Flow {}
