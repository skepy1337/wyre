use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

pub const SOCKET_DIR: &str = "/var/run/wyre";
pub const SOCKET_PATH: &str = "/var/run/wyre/wyre.sock";

pub const IPPROTO_ICMP: u8 = 1;
pub const IPPROTO_TCP: u8 = 6;
pub const IPPROTO_UDP: u8 = 17;
pub const IPPROTO_ICMPV6: u8 = 58;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Conn {
    pub protocol: u8,
    pub local_address: SocketAddr,
    pub remote_address: SocketAddr,
    // states from include/net/tcp_states.h
    pub state: Option<u8>,
    pub recv_rate: u64,
    pub send_rate: u64,
    pub recv_total: u64,
    pub send_total: u64,
}

impl Conn {
    pub fn is_tcp(&self) -> bool {
        self.protocol == IPPROTO_TCP
    }

    pub fn is_udp(&self) -> bool {
        self.protocol == IPPROTO_UDP
    }

    pub fn is_icmp(&self) -> bool {
        matches!(self.protocol, IPPROTO_ICMP | IPPROTO_ICMPV6)
    }

    pub fn protocol_name(&self) -> &str {
        match self.protocol {
            IPPROTO_TCP => "TCP",
            IPPROTO_UDP => "UDP",
            IPPROTO_ICMP | IPPROTO_ICMPV6 => "ICMP",
            _ => "RAW",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Process {
    pub name: String,
    pub pid: i32,
    pub ppid: i32,
    pub connections: Vec<Conn>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessGroup {
    pub name: String,
    pub processes: Vec<Process>,
}
