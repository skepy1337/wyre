use crate::flows::Flows;
use common::{SOCKET_DIR, SOCKET_PATH};
use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;
use tokio::time::{sleep, sleep_until, timeout};

const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn serve(mut flows: Flows) -> anyhow::Result<()> {
    fs::create_dir_all(SOCKET_DIR)?;
    let _ = fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH)?;
    fs::set_permissions(SOCKET_PATH, Permissions::from_mode(0o666))?;
    println!("Listening on {SOCKET_PATH}");

    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((mut stream, _)) => {
                    // an unknown uid sees only system processes
                    let uid = stream.peer_cred().map_or(u32::MAX, |cred| cred.uid());
                    let json = flows.snapshot(uid);
                    tokio::spawn(async move {
                        match timeout(WRITE_TIMEOUT, stream.write_all(&json)).await {
                            Ok(Ok(())) => {}
                            Ok(Err(e)) => eprintln!("Failed to write to client: {e}"),
                            Err(_) => eprintln!("Gave up on a client that wasn't reading"),
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Accept error: {e}");
                    sleep(Duration::from_millis(100)).await;
                }
            },
            () = sleep_until(flows.sweep_due().into()) => flows.sweep(),
        }
    }
}
