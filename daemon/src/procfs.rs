use ebpf::{AF_INET, AF_INET6, Endpoint, FlowKey, IPPROTO_TCP};
use std::collections::HashMap;
use std::fs;

const TCP_LISTEN: u8 = 10;

pub struct ProcessInfo {
    pub name: String,
    pub ppid: u32,
}

pub fn process_info(pid: u32) -> ProcessInfo {
    if pid == 0 {
        return ProcessInfo { name: "Unknown".to_string(), ppid: 0 };
    }
    let status = fs::read_to_string(format!("/proc/{pid}/status")).unwrap_or_default();
    let mut name = String::new();
    let mut ppid = 0;
    for line in status.lines() {
        if let Some(value) = line.strip_prefix("Name:\t") {
            name = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("PPid:\t") {
            ppid = value.trim().parse().unwrap_or(0);
        }
    }
    if name.is_empty() {
        // already exited
        return ProcessInfo { name: format!("pid {pid}"), ppid: 0 };
    }
    ProcessInfo { name: full_name(pid, name), ppid }
}

fn full_name(pid: u32, name: String) -> String {
    // max name length
    if name.len() < 15 {
        return name;
    }
    let exe = fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .and_then(|path| file_name(&path.to_string_lossy()));
    let argument = fs::read(format!("/proc/{pid}/cmdline")).ok().and_then(|cmdline| {
        let first = cmdline.split(|&byte| byte == 0).next()?;
        file_name(&String::from_utf8_lossy(first))
    });
    [exe, argument]
        .into_iter()
        .flatten()
        .find(|longer| longer.len() > name.len() && longer.starts_with(&name))
        .unwrap_or(name)
}

fn file_name(path: &str) -> Option<String> {
    let path = path.strip_suffix(" (deleted)").unwrap_or(path);
    let name = path.rsplit('/').next()?;
    (!name.is_empty()).then(|| name.to_string())
}

/// the real uid of a running process
fn uid(pid: u32) -> Option<u32> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let uids = status.lines().find_map(|line| line.strip_prefix("Uid:"))?;
    uids.split_whitespace().next()?.parse().ok()
}

pub struct TcpConnection {
    pub key: FlowKey,
    pub pid: u32,
    pub uid: u32,
    pub state: u8,
}

pub fn tcp_connections() -> Vec<TcpConnection> {
    let owners = socket_owners();
    let mut uids = HashMap::new();
    let mut connections = Vec::new();
    for (path, family) in [("/proc/net/tcp", AF_INET), ("/proc/net/tcp6", AF_INET6)] {
        let table = fs::read_to_string(path).unwrap_or_default();
        for line in table.lines().skip(1) {
            if let Some(mut connection) = parse_tcp_line(line, family, &owners) {
                connection.uid = *uids
                    .entry(connection.pid)
                    .or_insert_with(|| uid(connection.pid).unwrap_or(0));
                connections.push(connection);
            }
        }
    }
    connections
}

fn parse_tcp_line(
    line: &str,
    family: u16,
    owners: &HashMap<u64, u32>,
) -> Option<TcpConnection> {
    let fields: Vec<&str> = line.split_whitespace().collect();
    let state = u8::from_str_radix(fields.get(3)?, 16).ok()?;
    if state == TCP_LISTEN {
        return None;
    }
    let local = parse_endpoint(fields.get(1)?, family)?;
    let remote = parse_endpoint(fields.get(2)?, family)?;
    let inode: u64 = fields.get(9)?.parse().ok()?;
    Some(TcpConnection {
        key: FlowKey::new(0, IPPROTO_TCP, local, remote),
        pid: owners.get(&inode).copied().unwrap_or(0),
        uid: 0,
        state,
    })
}

/// parse hex addresses
fn parse_endpoint(text: &str, family: u16) -> Option<Endpoint> {
    let (address, port) = text.split_once(':')?;
    let words = if family == AF_INET { 1 } else { 4 };
    if address.len() != words * 8 {
        return None;
    }
    let mut ip = [0; 16];
    for i in 0..words {
        let word = u32::from_str_radix(address.get(i * 8..i * 8 + 8)?, 16).ok()?;
        ip[i * 4..i * 4 + 4].copy_from_slice(&word.to_ne_bytes());
    }
    Some(Endpoint { family, ip, port: u16::from_str_radix(port, 16).ok()? })
}

fn socket_owners() -> HashMap<u64, u32> {
    let mut owners = HashMap::new();
    let Ok(processes) = fs::read_dir("/proc") else {
        return owners;
    };
    for process in processes.flatten() {
        let Ok(pid) = process.file_name().to_string_lossy().parse() else {
            continue;
        };
        let Ok(fds) = fs::read_dir(process.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let Ok(link) = fs::read_link(fd.path()) else {
                continue;
            };
            let link = link.to_string_lossy();
            if let Some(inode) =
                link.strip_prefix("socket:[").and_then(|s| s.strip_suffix(']'))
                && let Ok(inode) = inode.parse()
            {
                owners.insert(inode, pid);
            }
        }
    }
    owners
}
