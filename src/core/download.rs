//! 下载测速 —— 每 IP 独占 Client + resolve()，EWMA 平滑采样

use crate::core::constants::{
    DEFAULT_DOWNLOAD_TEST_COUNT, DEFAULT_DOWNLOAD_TIMEOUT_SECS, DEFAULT_DOWNLOAD_URL,
    DOWNLOAD_CONCURRENCY, SPEED_SAMPLES, USER_AGENT,
};
use crate::core::data::{CloudflareIpData, DownloadSpeedSet};
use crate::core::httping::get_header_colo;
use crate::core::HooksRef;
use futures::stream::{self, StreamExt};
use reqwest::ClientBuilder;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct DownloadConfig {
    pub url: String,
    pub timeout: Duration,
    pub min_speed: f64, // MB/s，全局速度门槛
    /// 达标多少个就停（--dn）；0 表示不设上限，整个队列测完
    pub test_count: usize,
    pub port: u16,
    pub disable: bool,
    pub debug: bool,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            url: DEFAULT_DOWNLOAD_URL.to_string(),
            timeout: Duration::from_secs(DEFAULT_DOWNLOAD_TIMEOUT_SECS),
            min_speed: 0.0,
            test_count: DEFAULT_DOWNLOAD_TEST_COUNT,
            port: 443,
            disable: false,
            debug: false,
        }
    }
}

impl DownloadConfig {
    pub fn normalize(&mut self) {
        if self.url.is_empty() {
            self.url = DEFAULT_DOWNLOAD_URL.to_string();
        }
        if self.timeout <= Duration::ZERO {
            self.timeout = Duration::from_secs(DEFAULT_DOWNLOAD_TIMEOUT_SECS);
        }
    }
}

/// EWMA：alpha = 1/3，与 Go 版 github.com/VividCortex/ewma 一致
struct Ewma {
    alpha: f64,
    value: f64,
    initialized: bool,
}

impl Ewma {
    fn new() -> Self {
        Self { alpha: 1.0 / 3.0, value: 0.0, initialized: false }
    }
    fn add(&mut self, sample: f64) {
        if !self.initialized {
            self.value = sample;
            self.initialized = true;
        } else {
            self.value = self.alpha * sample + (1.0 - self.alpha) * self.value;
        }
    }
    fn get(&self) -> f64 {
        self.value
    }
}

pub async fn run_download(
    ip_data: Vec<CloudflareIpData>,
    config: DownloadConfig,
    hooks: HooksRef,
) -> DownloadSpeedSet {
    if ip_data.is_empty() {
        return DownloadSpeedSet::new(Vec::new());
    }
    // 只有「明确禁用」才跳过；速度门槛为 0 表示不筛速度，但仍然要逐个测完
    if config.disable {
        hooks.log("download", "已禁用下载测速，仅输出延迟结果");
        return DownloadSpeedSet::new(ip_data);
    }

    let parsed_url = match reqwest::Url::parse(&config.url) {
        Ok(u) => u,
        Err(_) => {
            hooks.log("download", &format!("下载测速地址解析失败: {}", config.url));
            return DownloadSpeedSet::new(ip_data);
        }
    };
    let scheme = parsed_url.scheme().to_string();
    let original_host = parsed_url.host_str().unwrap_or("").to_string();
    let path = parsed_url.path().to_string();
    let query = parsed_url.query().map(|q| format!("?{}", q)).unwrap_or_default();
    let path_and_query = format!("{}{}", path, query);

    let total = ip_data.len();
    let min_bytes = config.min_speed * 1024.0 * 1024.0;
    let cfg_timeout = config.timeout;

    let download_stream = stream::iter(ip_data.iter().cloned().take(total)).map(|d| {
        let ip = d.ping_data.ip;
        let download_url =
            format!("{}://{}:{}{}", scheme, original_host, config.port, path_and_query);
        let host = original_host.clone();
        let port = config.port;
        let timeout = cfg_timeout;

        async move {
            let client = match ClientBuilder::new()
                .timeout(timeout)
                .danger_accept_invalid_certs(true)
                .no_proxy()
                .resolve(&host, SocketAddr::new(ip, port))
                .redirect(reqwest::redirect::Policy::limited(10))
                .build()
            {
                Ok(c) => c,
                Err(_) => return None,
            };

            let mut response = match client
                .get(&download_url)
                .header("User-Agent", USER_AGENT)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(_) => return None,
            };

            if response.status() != reqwest::StatusCode::OK {
                return None;
            }

            let colo = get_header_colo(response.headers());
            let speed = measure_download_speed(&mut response, timeout).await;
            let mut result = d;
            result.download_speed = speed;
            if result.ping_data.colo.is_empty() {
                result.ping_data.colo = colo;
            }
            Some(result)
        }
    });

    // 并发只由常量决定，不再被「要几个结果」限制
    let concurrency = DOWNLOAD_CONCURRENCY.max(1);
    let mut stream = download_stream.buffer_unordered(concurrency);

    let mut speed_set: Vec<CloudflareIpData> = Vec::new();
    let mut done = 0usize;
    // --dn：达标多少个就停；0 = 不设上限，整个队列测完
    let target = config.test_count;

    while let Some(result) = stream.next().await {
        if hooks.cancelled() {
            break;
        }
        done += 1;
        if let Some(data) = result {
            hooks.item(&data);
            // 速度必须 > 0（排除测速失败）且达到全局门槛
            if data.download_speed > 0.0 && data.download_speed >= min_bytes {
                speed_set.push(data);
            }
        }
        hooks.tick(
            "download",
            done,
            total,
            &format!("已测 {}/{}，达标 {}", done, total, speed_set.len()),
        );

        // 达标数够了立刻收工，队列里剩下的不再测
        if target > 0 && speed_set.len() >= target {
            let left = total.saturating_sub(done);
            hooks.log(
                "download",
                &format!("达标 {} 个，提前结束下载测速（队列剩 {} 个未测）", speed_set.len(), left),
            );
            break;
        }
    }

    if config.debug && speed_set.is_empty() {
        hooks.log(
            "download",
            "[调试] 没有满足下载速度下限的 IP，忽略条件返回全部数据",
        );
        speed_set = ip_data.clone();
    }

    DownloadSpeedSet::new(speed_set)
}

/// 按 EWMA 采样下载速率，返回 bytes/second
async fn measure_download_speed(response: &mut reqwest::Response, timeout: Duration) -> f64 {
    let sample_interval = timeout / SPEED_SAMPLES;
    let time_end = Instant::now() + timeout;
    let mut next_sample = Instant::now() + sample_interval;

    let mut total_bytes: i64 = 0;
    let mut last_bytes: i64 = 0;
    let mut ewma = Ewma::new();

    while Instant::now() < time_end {
        if Instant::now() >= next_sample {
            let delta = total_bytes - last_bytes;
            let rate = delta as f64 / sample_interval.as_secs_f64();
            ewma.add(rate);
            last_bytes = total_bytes;
            next_sample = Instant::now() + sample_interval;
        }

        match response.chunk().await {
            Ok(Some(chunk)) => total_bytes += chunk.len() as i64,
            Ok(None) => break,
            Err(_) => break,
        }
    }

    ewma.get()
}
