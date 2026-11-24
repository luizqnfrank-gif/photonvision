use serde::{Deserialize, Serialize};
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeSyncPingMetadata {
    pub offset_ns: i64,
    pub rtt2_ns: i64,
    pub pings_sent: i64,
    pub pongs_received: i64,
    pub last_pong_time_ns: i64,
}

impl TimeSyncPingMetadata {
    pub fn time_since_last_pong_ns(&self) -> i64 {
        if self.last_pong_time_ns == 0 {
            return 0;
        }
        now_nanos().saturating_sub(self.last_pong_time_ns)
    }
}

pub fn now_nanos() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

pub fn now_micros() -> i64 {
    now_nanos() / 1000
}

/// UDP TimeSync client (`org.photonvision.jni.TimeSyncClient`).
#[derive(Clone)]
pub struct TimeSyncClient {
    server_ip: Arc<Mutex<String>>,
    port: u16,
    metadata: Arc<Mutex<TimeSyncPingMetadata>>,
}

impl TimeSyncClient {
    pub fn new(server_ip: impl Into<String>, port: u16) -> Self {
        Self {
            server_ip: Arc::new(Mutex::new(server_ip.into())),
            port,
            metadata: Arc::new(Mutex::new(TimeSyncPingMetadata::default())),
        }
    }

    pub fn set_server(&self, new_ip: impl Into<String>) {
        if let Ok(mut guard) = self.server_ip.lock() {
            *guard = new_ip.into();
        }
    }

    pub fn get_server(&self) -> String {
        self.server_ip
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "127.0.0.1".to_string())
    }

    pub fn get_offset(&self) -> i64 {
        self.metadata.lock().map(|m| m.offset_ns).unwrap_or(0)
    }

    pub fn get_ping_metadata(&self) -> TimeSyncPingMetadata {
        self.metadata.lock().map(|m| *m).unwrap_or_default()
    }

    /// Perform a single UDP ping-pong round-trip (16-byte payload: `[client_tx_ns: i64, server_rx_ns: i64]`).
    pub fn send_ping_once(&self) -> std::io::Result<TimeSyncPingMetadata> {
        let target = format!("{}:{}", self.get_server(), self.port);
        let sock = UdpSocket::bind("0.0.0.0:0")?;
        sock.set_read_timeout(Some(Duration::from_millis(150)))?;

        let t1 = now_nanos();
        let mut req = [0u8; 16];
        req[..8].copy_from_slice(&t1.to_le_bytes());

        if let Ok(mut meta) = self.metadata.lock() {
            meta.pings_sent += 1;
        }

        sock.send_to(&req, &target)?;

        let mut resp = [0u8; 16];
        let (n, _) = sock.recv_from(&mut resp)?;
        let t4 = now_nanos();

        if n >= 16 {
            let mut b1 = [0u8; 8];
            let mut b2 = [0u8; 8];
            b1.copy_from_slice(&resp[..8]);
            b2.copy_from_slice(&resp[8..16]);
            let t1_echo = i64::from_le_bytes(b1);
            let t2_server = i64::from_le_bytes(b2);
            let rtt = (t4 - t1_echo).max(0);
            let rtt2 = rtt / 2;
            let offset = t2_server - (t1_echo + rtt2);

            if let Ok(mut meta) = self.metadata.lock() {
                meta.offset_ns = offset;
                meta.rtt2_ns = rtt2;
                meta.pongs_received += 1;
                meta.last_pong_time_ns = t4;
                return Ok(*meta);
            }
        }
        Ok(self.get_ping_metadata())
    }
}

/// UDP TimeSync server (`org.photonvision.jni.TimeSyncServer`).
pub struct TimeSyncServer {
    port: u16,
}

impl TimeSyncServer {
    pub const fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn spawn_udp_responder(&self) -> std::io::Result<(u16, std::thread::JoinHandle<()>)> {
        let sock = UdpSocket::bind(format!("127.0.0.1:{}", self.port))?;
        sock.set_read_timeout(Some(Duration::from_millis(200)))?;
        let bound_port = sock.local_addr()?.port();

        let handle = std::thread::spawn(move || {
            let mut buf = [0u8; 16];
            while let Ok((n, peer)) = sock.recv_from(&mut buf) {
                if n >= 8 {
                    let server_time = now_nanos();
                    let mut reply = [0u8; 16];
                    reply[..8].copy_from_slice(&buf[..8]);
                    reply[8..16].copy_from_slice(&server_time.to_le_bytes());
                    let _ = sock.send_to(&reply, peer);
                }
            }
        });
        Ok((bound_port, handle))
    }
}

/// Manages time synchronization over UDP port 5810 (`org.photonvision.common.dataflow.networktables.TimeSyncManager`).
#[derive(Clone)]
pub struct TimeSyncManager {
    client: TimeSyncClient,
    run_server: bool,
}

impl Default for TimeSyncManager {
    fn default() -> Self {
        Self::new("127.0.0.1", 5810, false)
    }
}

impl TimeSyncManager {
    pub fn new(server_ip: impl Into<String>, port: u16, run_server: bool) -> Self {
        Self {
            client: TimeSyncClient::new(server_ip, port),
            run_server,
        }
    }

    pub fn client(&self) -> &TimeSyncClient {
        &self.client
    }

    pub fn get_offset(&self) -> i64 {
        if self.run_server {
            0
        } else {
            self.client.get_offset()
        }
    }

    pub fn get_time_since_last_pong(&self) -> i64 {
        if self.run_server {
            0
        } else {
            self.client.get_ping_metadata().time_since_last_pong_ns()
        }
    }
}
