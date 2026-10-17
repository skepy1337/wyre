use crate::procfs::{self, ProcessInfo, TcpConnection};
use crate::users::Users;
use aya::maps::MapData;
use common::{Conn, Process, ProcessGroup};
use ebpf::{AF_INET, Flow, FlowKey, IPPROTO_TCP};
use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

const HIDE_AFTER: Duration = Duration::from_secs(30);
const DATAGRAM_EXPIRY: Duration = Duration::from_mins(1);
const TCP_EXPIRY: Duration = Duration::from_mins(10);
const FRESH: Duration = Duration::from_millis(500);
const IDLE: Duration = Duration::from_secs(30);

// include/net/tcp_states.h
const TCP_ESTABLISHED: u8 = 1;
const TCP_TIME_WAIT: u8 = 6;
const TCP_CLOSE: u8 = 7;

const BPF_NOEXIST: u64 = 1;

pub struct Flows {
    map: aya::maps::HashMap<MapData, FlowKey, Flow>,
    // sent and recv byte counts
    last_counts: HashMap<FlowKey, (u64, u64)>,
    last_poll: Instant,
    // the last poll's processes with their uids
    listed: Option<Vec<(u32, Process)>>,
    snapshots: HashMap<u32, Arc<[u8]>>,
    processes: HashMap<u32, ProcessInfo>,
    users: Users,
}

struct Owned {
    uid: u32,
    connections: Vec<Conn>,
}

impl Flows {
    pub fn new(map: aya::maps::HashMap<MapData, FlowKey, Flow>) -> Flows {
        Flows {
            map,
            last_counts: HashMap::new(),
            last_poll: Instant::now(),
            listed: None,
            snapshots: HashMap::new(),
            processes: HashMap::new(),
            users: Users::load(),
        }
    }

    pub fn add_existing(&mut self, connections: Vec<TcpConnection>) {
        let now = monotonic_ns();
        for connection in connections {
            let flow = Flow {
                last_seen: now,
                pid: connection.pid,
                uid: connection.uid,
                state: connection.state,
                ..Flow::default()
            };
            let _ = self.map.insert(connection.key, flow, BPF_NOEXIST);
        }
    }

    /// the connections a user can see
    pub fn snapshot(&mut self, client: u32) -> Arc<[u8]> {
        if self.listed.is_none() || self.last_poll.elapsed() >= FRESH {
            let owned = self.poll();
            self.listed = Some(self.list(owned));
            self.snapshots.clear();
            self.users.refresh();
        }
        if let Some(json) = self.snapshots.get(&client) {
            return Arc::clone(json);
        }
        let users = &self.users;
        let visible = self
            .listed
            .iter()
            .flatten()
            .filter(|(uid, process)| users.can_see(client, process.pid as u32, *uid))
            .map(|(_, process)| process.clone());
        let json: Arc<[u8]> =
            serde_json::to_vec(&group(visible)).expect("snapshot serializes").into();
        self.snapshots.insert(client, Arc::clone(&json));
        json
    }

    pub fn sweep_due(&self) -> Instant {
        self.last_poll + IDLE
    }

    pub fn sweep(&mut self) {
        self.poll();
        self.listed = None;
        self.snapshots = HashMap::new();
        self.processes = HashMap::new();
        release_free_memory();
    }

    /// delete finished connections
    fn poll(&mut self) -> HashMap<u32, Owned> {
        let now = monotonic_ns();
        let seconds = self.last_poll.elapsed().as_secs_f64().max(0.001);
        self.last_poll = Instant::now();

        let mut owned: HashMap<u32, Owned> = HashMap::new();
        let mut counts = HashMap::new();
        let mut finished = Vec::new();
        for (key, flow) in self.map.iter().filter_map(Result::ok) {
            let idle = Duration::from_nanos(now.saturating_sub(flow.last_seen));
            if is_finished(&key, &flow, idle) {
                finished.push(key);
                continue;
            }
            if counts.insert(key, (flow.rx_bytes, flow.tx_bytes)).is_some() {
                continue;
            }
            if idle >= HIDE_AFTER && !is_established(&key, &flow) {
                continue;
            }

            let (last_rx, last_tx) =
                self.last_counts.get(&key).copied().unwrap_or_default();
            let process = owned
                .entry(flow.pid)
                .or_insert_with(|| Owned { uid: flow.uid, connections: Vec::new() });
            process.connections.push(Conn {
                protocol: key.protocol,
                local_address: socket_address(key.family, key.src_ip, key.src_port),
                remote_address: socket_address(key.family, key.dst_ip, key.dst_port),
                state: (key.protocol == IPPROTO_TCP && flow.state != 0)
                    .then_some(flow.state),
                recv_rate: per_second(flow.rx_bytes.saturating_sub(last_rx), seconds),
                send_rate: per_second(flow.tx_bytes.saturating_sub(last_tx), seconds),
                recv_total: flow.rx_bytes,
                send_total: flow.tx_bytes,
            });
        }

        for key in finished {
            let _ = self.map.remove(&key);
        }
        self.last_counts = counts;
        owned
    }

    fn list(&mut self, owned: HashMap<u32, Owned>) -> Vec<(u32, Process)> {
        self.processes.retain(|pid, _| owned.contains_key(pid));
        owned
            .into_iter()
            .map(|(pid, Owned { uid, connections })| {
                let info = self
                    .processes
                    .entry(pid)
                    .or_insert_with(|| procfs::process_info(pid));
                let process = Process {
                    name: info.name.clone(),
                    pid: pid as i32,
                    ppid: info.ppid as i32,
                    connections,
                };
                (uid, process)
            })
            .collect()
    }
}

/// grouped by program name
fn group(processes: impl Iterator<Item = Process>) -> Vec<ProcessGroup> {
    let mut groups: BTreeMap<String, Vec<Process>> = BTreeMap::new();
    for process in processes {
        groups.entry(process.name.clone()).or_default().push(process);
    }
    groups.into_iter().map(|(name, processes)| ProcessGroup { name, processes }).collect()
}

fn is_finished(key: &FlowKey, flow: &Flow, idle: Duration) -> bool {
    if key.protocol != IPPROTO_TCP {
        return idle >= DATAGRAM_EXPIRY;
    }
    match flow.state {
        TCP_ESTABLISHED => false,
        TCP_CLOSE | TCP_TIME_WAIT => idle >= HIDE_AFTER,
        _ => idle >= TCP_EXPIRY,
    }
}

fn is_established(key: &FlowKey, flow: &Flow) -> bool {
    key.protocol == IPPROTO_TCP && flow.state == TCP_ESTABLISHED
}

fn socket_address(family: u16, ip: [u8; 16], port: u16) -> SocketAddr {
    if family == AF_INET {
        SocketAddr::from(([ip[0], ip[1], ip[2], ip[3]], port))
    } else {
        SocketAddr::from((ip, port))
    }
}

fn per_second(bytes: u64, seconds: f64) -> u64 {
    (bytes as f64 / seconds) as u64
}

/// same as bpf_ktime_get_ns()
fn monotonic_ns() -> u64 {
    let mut now = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) };
    now.tv_sec as u64 * 1_000_000_000 + now.tv_nsec as u64
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn release_free_memory() {
    unsafe { libc::malloc_trim(0) };
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn release_free_memory() {}
