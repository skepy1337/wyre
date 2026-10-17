mod flows;
mod procfs;
mod server;
mod users;

use anyhow::Context;
use aya::Ebpf;
use aya::maps::HashMap;
use aya::programs::{KProbe, TracePoint};
use flows::Flows;
use tokio::signal::unix::{SignalKind, signal};

const KPROBES: &[(&str, &str)] = &[
    ("tcp_connect", "tcp_connect"),
    ("tcp_sendmsg_enter", "tcp_sendmsg_locked"),
    ("tcp_sendmsg_exit", "tcp_sendmsg_locked"),
    ("tcp_recvmsg_enter", "tcp_recvmsg"),
    ("tcp_recvmsg_exit", "tcp_recvmsg"),
    ("udp_sendmsg_enter", "udp_sendmsg"),
    ("udp_sendmsg_exit", "udp_sendmsg"),
    ("udp_recvmsg_enter", "udp_recvmsg"),
    ("udp_recvmsg_exit", "udp_recvmsg"),
    ("udpv6_sendmsg_enter", "udpv6_sendmsg"),
    ("udpv6_sendmsg_exit", "udpv6_sendmsg"),
    ("udpv6_recvmsg_enter", "udpv6_recvmsg"),
    ("udpv6_recvmsg_exit", "udpv6_recvmsg"),
    ("raw_sendmsg_enter", "raw_sendmsg"),
    ("raw_sendmsg_exit", "raw_sendmsg"),
    ("raw_recvmsg_enter", "raw_recvmsg"),
    ("raw_recvmsg_exit", "raw_recvmsg"),
    ("rawv6_sendmsg_enter", "rawv6_sendmsg"),
    ("rawv6_sendmsg_exit", "rawv6_sendmsg"),
    ("rawv6_recvmsg_enter", "rawv6_recvmsg"),
    ("rawv6_recvmsg_exit", "rawv6_recvmsg"),
    ("ping_v4_sendmsg_enter", "ping_v4_sendmsg"),
    ("ping_v4_sendmsg_exit", "ping_v4_sendmsg"),
    ("ping_v6_sendmsg_enter", "ping_v6_sendmsg"),
    ("ping_v6_sendmsg_exit", "ping_v6_sendmsg"),
    ("ping_recvmsg_enter", "ping_recvmsg"),
    ("ping_recvmsg_exit", "ping_recvmsg"),
];

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let mut bpf = Ebpf::load(aya::include_bytes_aligned!(env!("EBPF_OBJECT")))?;

    for &(program, function) in KPROBES {
        let probe: &mut KProbe = bpf
            .program_mut(program)
            .with_context(|| format!("{program} is missing from the eBPF object"))?
            .try_into()?;
        probe.load()?;
        probe
            .attach(function, 0)
            .with_context(|| format!("attaching {program} to {function}"))?;
    }
    let tracepoint: &mut TracePoint = bpf
        .program_mut("inet_sock_set_state")
        .context("inet_sock_set_state is missing from the eBPF object")?
        .try_into()?;
    tracepoint.load()?;
    tracepoint
        .attach("sock", "inet_sock_set_state")
        .context("attaching to the sock/inet_sock_set_state tracepoint")?;

    let map = bpf.take_map("FLOWS").context("FLOWS is missing from the eBPF object")?;
    let mut flows = Flows::new(HashMap::try_from(map)?);
    flows.add_existing(procfs::tcp_connections());

    let result = tokio::select! {
        result = server::serve(flows) => result,
        result = shutdown_signal() => result,
    };
    let _ = std::fs::remove_file(common::SOCKET_PATH);
    result
}

async fn shutdown_signal() -> anyhow::Result<()> {
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let mut quit = signal(SignalKind::quit())?;
    let mut hangup = signal(SignalKind::hangup())?;
    tokio::select! {
        _ = interrupt.recv() => println!("Received SIGINT"),
        _ = terminate.recv() => println!("Received SIGTERM"),
        _ = quit.recv() => println!("Received SIGQUIT"),
        _ = hangup.recv() => println!("Received SIGHUP"),
    }
    Ok(())
}
