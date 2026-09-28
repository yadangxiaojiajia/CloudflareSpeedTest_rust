//! TCPing 延迟测速 —— buffer_unordered 有界并发

use crate::core::constants::{
    DEFAULT_PING_ROUTINES, DEFAULT_PING_TIMES, DEFAULT_PORT, MAX_PING_ROUTINES,
    TCP_CONNECT_TIMEOUT_SECS,
};
use crate::core::data::{CloudflareIpData, PingData};
use crate::core::HooksRef;
use futures::stream::{self, StreamExt};
use std::net::IpAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::{timeout, Instant};

const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(TCP_CONNECT_TIMEOUT_SECS);

#[derive(Debug, Clone)]
pub struct TcpingConfig {
    pub routines: usize,
    pub port: u16,
    pub ping_times: u32,
}

impl Default for TcpingConfig {
    fn default() -> Self {
        Self {
            routines: DEFAULT_PING_ROUTINES,
            port: DEFAULT_PORT,
            ping_times: DEFAULT_PING_TIMES,
        }
    }
}

impl TcpingConfig {
    pub fn normalize(&mut self) {
        if self.routines == 0 || self.routines > MAX_PING_ROUTINES {
            self.routines = DEFAULT_PING_ROUTINES;
        }
        if self.port == 0 {
            self.port = DEFAULT_PORT;
        }
        if self.ping_times == 0 {
            self.ping_times = DEFAULT_PING_TIMES;
        }
    }
}

pub async fn run_tcping(
    ips: Vec<IpAddr>,
    config: TcpingConfig,
    hooks: HooksRef,
) -> Vec<CloudflareIpData> {
    let total = ips.len();
    if total == 0 {
        return Vec::new();
    }

    let routines = config.routines;
    let stream = stream::iter(ips).map(|ip| {
        let cfg = config.clone();
        async move { tcping_ip(&ip, &cfg).await }
    });

    let mut results: Vec<CloudflareIpData> = Vec::new();
    let mut done = 0usize;
    let mut stream = stream.buffer_unordered(routines);

    while let Some(data) = stream.next().await {
        done += 1;
        if let Some(d) = data {
            results.push(d);
        }
        if hooks.cancelled() {
            break;
        }
        // 每 8 个或最后一个推一次，避免事件过多
        if done % 8 == 0 || done == total {
            hooks.tick("ping", done, total, &format!("可用 {}", results.len()));
        }
    }

    results
}

/// 单 IP TCP 握手测延迟：成功 n 次取平均，全部失败则丢弃
async fn tcping_ip(ip: &IpAddr, config: &TcpingConfig) -> Option<CloudflareIpData> {
    let mut received = 0u32;
    let mut total_delay = Duration::ZERO;

    let addr = match ip {
        IpAddr::V4(v4) => format!("{}:{}", v4, config.port),
        IpAddr::V6(v6) => format!("[{}]:{}", v6, config.port),
    };

    for _ in 0..config.ping_times {
        let start = Instant::now();
        if let Ok(Ok(stream)) = timeout(TCP_CONNECT_TIMEOUT, TcpStream::connect(&addr)).await {
            drop(stream);
            received += 1;
            total_delay += start.elapsed();
        }
    }

    if received == 0 {
        return None;
    }

    Some(CloudflareIpData {
        ping_data: PingData {
            ip: *ip,
            sent: config.ping_times,
            received,
            delay: total_delay / received,
            colo: String::new(),
        },
        download_speed: 0.0,
    })
}
