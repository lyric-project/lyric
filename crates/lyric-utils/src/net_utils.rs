use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashSet;
use std::net::TcpListener;
use std::time::Instant;
use tracing;

use crate::err::Error;

/// Get available port
pub fn listen_available_port(
    start_port: u16,
    end_port: u16,
    exclude_ports: HashSet<u16>,
) -> Option<(u16, TcpListener)> {
    let now = Instant::now();

    // Create a vector of all ports in the range
    let mut ports: Vec<u16> = (start_port..=end_port)
        .filter(|port| !exclude_ports.contains(port))
        .collect();

    // Shuffle the ports
    let mut rng = thread_rng();
    ports.shuffle(&mut rng);

    for (retry_times, port) in ports.into_iter().enumerate() {
        if let Ok(l) = TcpListener::bind(("127.0.0.1", port)) {
            tracing::info!(
                "Get available port {} success, retry_times: {}, time cost: {} ms",
                port,
                retry_times,
                now.elapsed().as_millis()
            );
            return Some((port, l));
        }
    }
    None
}

pub fn local_ip() -> Result<String, Error> {
    use local_ip_address::local_ip;
    local_ip()
        .map_err(|e| Error::InternalError(format!("Failed to get local IP: {}", e)))
        .map(|ip| ip.to_string())
}
