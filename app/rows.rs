use crate::geoip::{Geo, GeoDb};
use common::{Conn, IPPROTO_TCP, IPPROTO_UDP, Process, ProcessGroup};
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, SocketAddr};

pub struct View {
    pub search: String,
    pub show_tcp: bool,
    pub show_udp: bool,
    pub show_icmp: bool,
    // hides connections on ports 53 and 5353
    pub hide_dns: bool,
    pub sort_by: String,
    pub sort_desc: bool,
    pub conn_sort_by: String,
    pub conn_sort_desc: bool,
    pub expanded_groups: HashMap<String, bool>,
    pub expanded_processes: HashMap<i32, bool>,
}

impl View {
    fn shows(&self, conn: &Conn) -> bool {
        if self.hide_dns && is_dns(conn) {
            false
        } else if conn.is_tcp() {
            self.show_tcp
        } else if conn.is_udp() {
            self.show_udp
        } else if conn.is_icmp() {
            self.show_icmp
        } else {
            true
        }
    }

    fn shown_connections(&self, process: &Process) -> Vec<Conn> {
        process.connections.iter().copied().filter(|c| self.shows(c)).collect()
    }

    pub fn is_group_expanded(&self, name: &str) -> bool {
        let toggled = self.expanded_groups.get(name).copied();
        toggled.unwrap_or(!self.matches_name(name))
    }

    pub fn is_process_expanded(&self, pid: i32, group: &str) -> bool {
        let toggled = self.expanded_processes.get(&pid).copied();
        toggled.unwrap_or(!self.matches_name(group))
    }

    fn matches_name(&self, name: &str) -> bool {
        name.to_lowercase().contains(&self.search)
    }

    fn matches_address(&self, conn: &Conn) -> bool {
        conn.remote_address.to_string().to_lowercase().contains(&self.search)
            || conn.local_address.to_string().to_lowercase().contains(&self.search)
    }
}

#[derive(Serialize)]
#[serde(tag = "t")]
enum Row {
    #[serde(rename = "G")]
    Group { name: String, recv: String, send: String, conns: usize, exp: bool },
    #[serde(rename = "P")]
    Process {
        name: String,
        pid: i32,
        ppid: i32,
        recv: String,
        send: String,
        conns: usize,
        exp: bool,
    },
    #[serde(rename = "C")]
    Connection {
        pid: i32,
        ppid: i32,
        proc_name: String,
        proto: String,
        remote: String,
        local: String,
        state: Option<u8>,
        recv: String,
        send: String,
        recv_b: u64,
        send_b: u64,
        recv_total: String,
        send_total: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        country: Option<String>,
    },
}

#[derive(Default)]
pub struct Totals {
    pub recv: u64,
    pub send: u64,
    pub conns: usize,
}

pub fn totals(groups: &[ProcessGroup], view: &View) -> Totals {
    let mut totals = Totals::default();
    for group in groups {
        let group = group_totals(group, view);
        totals.recv += group.recv;
        totals.send += group.send;
        totals.conns += group.conns;
    }
    totals
}

fn group_totals(group: &ProcessGroup, view: &View) -> Totals {
    let mut totals = Totals::default();
    for process in &group.processes {
        let shown = view.shown_connections(process);
        totals.recv += shown.iter().map(|c| c.recv_rate).sum::<u64>();
        totals.send += shown.iter().map(|c| c.send_rate).sum::<u64>();
        totals.conns += merge_by_address(shown).len();
    }
    totals
}

struct Summary {
    lowercase_name: String,
    matches_name: bool,
    totals: Totals,
}

pub fn build(groups: &[ProcessGroup], view: &View, geo: &GeoDb) -> String {
    let mut summaries = Vec::new();
    for group in groups {
        let matches_name = view.matches_name(&group.name);
        if !matches_name && !has_match(group, view) {
            continue;
        }
        let totals = group_totals(group, view);
        if totals.conns == 0 {
            continue;
        }
        let lowercase_name = group.name.to_lowercase();
        summaries.push((group, Summary { lowercase_name, matches_name, totals }));
    }
    summaries.sort_by(|(_, a), (_, b)| compare_groups(a, b, view));

    let mut rows = Vec::new();
    for (group, summary) in summaries {
        let expanded = view.is_group_expanded(&group.name);
        rows.push(Row::Group {
            name: group.name.clone(),
            recv: format_bytes(summary.totals.recv),
            send: format_bytes(summary.totals.send),
            conns: summary.totals.conns,
            exp: expanded,
        });
        if !expanded {
            continue;
        }

        let mut processes: Vec<&Process> = group.processes.iter().collect();
        processes.sort_by_key(|p| p.pid);
        for process in processes {
            let shown = view.shown_connections(process);
            let recv = shown.iter().map(|c| c.recv_rate).sum();
            let send = shown.iter().map(|c| c.send_rate).sum();

            let listed: Vec<Conn> = if summary.matches_name {
                shown
            } else {
                shown.into_iter().filter(|c| view.matches_address(c)).collect()
            };
            if listed.is_empty() {
                continue;
            }
            let mut conns = merge_by_address(listed);
            sort_conns(&mut conns, &view.conn_sort_by, view.conn_sort_desc);

            let expanded = view.is_process_expanded(process.pid, &group.name);
            rows.push(Row::Process {
                name: process.name.clone(),
                pid: process.pid,
                ppid: process.ppid,
                recv: format_bytes(recv),
                send: format_bytes(send),
                conns: conns.len(),
                exp: expanded,
            });
            if expanded {
                rows.extend(conns.iter().map(|conn| connection_row(process, conn, geo)));
            }
        }
    }
    serde_json::to_string(&rows).expect("rows serialize")
}

pub fn connection(
    groups: &[ProcessGroup],
    pid: i32,
    proto: &str,
    local: &str,
    remote: &str,
    geo: &GeoDb,
) -> Option<String> {
    let (local, remote) = (address_ip(local)?, address_ip(remote)?);
    let process = groups.iter().flat_map(|g| &g.processes).find(|p| p.pid == pid)?;
    let conns = process.connections.iter().copied().filter(|c| {
        c.protocol_name() == proto
            && c.local_address.ip() == local
            && c.remote_address.ip() == remote
    });
    let conn = merge_by_address(conns.collect()).pop()?;
    let row = connection_row(process, &conn, geo);
    Some(serde_json::to_string(&row).expect("row serializes"))
}

fn has_match(group: &ProcessGroup, view: &View) -> bool {
    group
        .processes
        .iter()
        .flat_map(|p| &p.connections)
        .any(|c| view.shows(c) && view.matches_address(c))
}

fn compare_groups(a: &Summary, b: &Summary, view: &View) -> Ordering {
    let unknown = (a.lowercase_name == "unknown").cmp(&(b.lowercase_name == "unknown"));
    if unknown != Ordering::Equal {
        return unknown;
    }
    let order = match view.sort_by.as_str() {
        "name" => a.lowercase_name.cmp(&b.lowercase_name),
        "upload" => a.totals.send.cmp(&b.totals.send),
        "conns" => a.totals.conns.cmp(&b.totals.conns),
        _ => a.totals.recv.cmp(&b.totals.recv),
    };
    if view.sort_desc { order.reverse() } else { order }
}

fn merge_by_address(conns: Vec<Conn>) -> Vec<Conn> {
    let mut merged: BTreeMap<(u8, IpAddr, IpAddr), Conn> = BTreeMap::new();
    for conn in conns {
        let key = (conn.protocol, conn.local_address.ip(), conn.remote_address.ip());
        merged
            .entry(key)
            .and_modify(|m| {
                m.recv_rate += conn.recv_rate;
                m.send_rate += conn.send_rate;
                m.recv_total += conn.recv_total;
                m.send_total += conn.send_total;
            })
            .or_insert(conn);
    }
    merged.into_values().collect()
}

fn sort_conns(conns: &mut [Conn], sort_by: &str, desc: bool) {
    conns.sort_by(|a, b| {
        let order = match sort_by {
            "download" => a.recv_rate.cmp(&b.recv_rate),
            "upload" => a.send_rate.cmp(&b.send_rate),
            "state" => a.state.cmp(&b.state),
            _ => a.remote_address.cmp(&b.remote_address),
        };
        let order = if desc { order.reverse() } else { order };
        order
            .then(protocol_rank(a).cmp(&protocol_rank(b)))
            .then(a.remote_address.cmp(&b.remote_address))
    });
}

fn is_dns(conn: &Conn) -> bool {
    let ports = [conn.local_address.port(), conn.remote_address.port()];
    (conn.is_tcp() || conn.is_udp()) && ports.iter().any(|port| matches!(port, 53 | 5353))
}

fn protocol_rank(conn: &Conn) -> u8 {
    match conn.protocol {
        IPPROTO_TCP => 0,
        IPPROTO_UDP => 1,
        _ => 2,
    }
}

fn connection_row(process: &Process, conn: &Conn, geo: &GeoDb) -> Row {
    let found = geo.lookup(conn.remote_address.ip());
    Row::Connection {
        pid: process.pid,
        ppid: process.ppid,
        proc_name: process.name.clone(),
        proto: conn.protocol_name().to_string(),
        remote: address_text(conn.remote_address, conn.is_icmp()),
        local: address_text(conn.local_address, conn.is_icmp()),
        state: conn.state,
        recv: format_bytes(conn.recv_rate),
        send: format_bytes(conn.send_rate),
        recv_b: conn.recv_rate,
        send_b: conn.send_rate,
        recv_total: format_bytes(conn.recv_total),
        send_total: format_bytes(conn.send_total),
        location: found.as_ref().and_then(geo_line),
        country: found.and_then(|found| found.country),
    }
}

fn address_text(address: SocketAddr, is_icmp: bool) -> String {
    if is_icmp { address.ip().to_string() } else { address.to_string() }
}

fn address_ip(text: &str) -> Option<IpAddr> {
    text.parse::<SocketAddr>().map(|address| address.ip()).or_else(|_| text.parse()).ok()
}

fn geo_line(geo: &Geo) -> Option<String> {
    // replace US with the state which it's in
    let country = match (&geo.city, &geo.state, geo.country.as_deref()) {
        (Some(_), Some(_), Some("US")) => None,
        (_, _, country) => country,
    };
    let place: Vec<&str> = [geo.city.as_deref(), geo.state.as_deref(), country]
        .into_iter()
        .flatten()
        .collect();
    let place = (!place.is_empty()).then(|| place.join(", "));
    let parts: Vec<String> = [place, geo.org.clone()].into_iter().flatten().collect();
    // non-breaking spaces to prevent a new line from starting with them
    (!parts.is_empty()).then(|| parts.join("\u{a0}\u{a0}·  "))
}

pub fn format_bytes(bytes: u64) -> String {
    match bytes {
        1_000_000_000.. => format!("{:.1} GB", bytes as f64 / 1e9),
        1_000_000.. => format!("{:.1} MB", bytes as f64 / 1e6),
        1_000.. => format!("{:.1} KB", bytes as f64 / 1e3),
        _ => format!("{bytes} B"),
    }
}
