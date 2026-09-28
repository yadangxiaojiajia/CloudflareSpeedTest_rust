//! HTTPing 延迟测速 + CDN 地区码识别

use crate::core::constants::{
    DEFAULT_DOWNLOAD_URL, DEFAULT_PING_ROUTINES, DEFAULT_PING_TIMES, DEFAULT_PORT,
    HTTP_TIMEOUT_SECS, MAX_PING_ROUTINES, USER_AGENT,
};
use crate::core::data::{CloudflareIpData, PingData};
use crate::core::HooksRef;
use futures::stream::{self, StreamExt};
use regex::Regex;
use reqwest::Client;
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

const HTTP_TIMEOUT: Duration = Duration::from_secs(HTTP_TIMEOUT_SECS);

static RE_COLO_IATA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Z]{3}").unwrap());
static RE_COLO_COUNTRY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Z]{2}").unwrap());
static RE_COLO_GCORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]{2}").unwrap());

#[derive(Debug, Clone)]
pub struct HttpingConfig {
    pub routines: usize,
    pub port: u16,
    pub ping_times: u32,
    pub url: String,
    pub httping_status_code: i32,
    pub httping_cf_colo: String,
    pub debug: bool,
}

impl Default for HttpingConfig {
    fn default() -> Self {
        Self {
            routines: DEFAULT_PING_ROUTINES,
            port: DEFAULT_PORT,
            ping_times: DEFAULT_PING_TIMES,
            url: DEFAULT_DOWNLOAD_URL.to_string(),
            httping_status_code: 0,
            httping_cf_colo: String::new(),
            debug: false,
        }
    }
}

impl HttpingConfig {
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
        if self.url.is_empty() {
            self.url = DEFAULT_DOWNLOAD_URL.to_string();
        }
    }
}

/// 从响应头识别 CDN 地区码
pub fn get_header_colo(headers: &reqwest::header::HeaderMap) -> String {
    if let Some(cf_ray) = headers.get("cf-ray").and_then(|v| v.to_str().ok()) {
        if let Some(m) = RE_COLO_IATA.find(cf_ray) {
            return m.as_str().to_string();
        }
    }
    if let Some(x77) = headers.get("x-77-pop").and_then(|v| v.to_str().ok()) {
        if let Some(m) = RE_COLO_COUNTRY.find(x77) {
            return m.as_str().to_string();
        }
    }
    if let Some(server) = headers.get("server").and_then(|v| v.to_str().ok()) {
        if let Some(stripped) = server.strip_prefix("BunnyCDN-") {
            if let Some(m) = RE_COLO_COUNTRY.find(stripped) {
                return m.as_str().to_string();
            }
        }
    }
    if let Some(cf_pop) = headers.get("x-amz-cf-pop").and_then(|v| v.to_str().ok()) {
        if let Some(m) = RE_COLO_IATA.find(cf_pop) {
            return m.as_str().to_string();
        }
    }
    if let Some(x_served) = headers.get("x-served-by").and_then(|v| v.to_str().ok()) {
        if let Some(last) = RE_COLO_IATA.find_iter(x_served).last() {
            return last.as_str().to_string();
        }
    }
    if let Some(x_id_fe) = headers.get("x-id-fe").and_then(|v| v.to_str().ok()) {
        if let Some(m) = RE_COLO_GCORE.find(x_id_fe) {
            return m.as_str().to_uppercase();
        }
    }
    String::new()
}

fn build_colo_filter(colos: &str) -> Option<Arc<HashSet<String>>> {
    if colos.is_empty() {
        return None;
    }
    let set: HashSet<String> = colos.split(',').map(|s| s.trim().to_uppercase()).collect();
    Some(Arc::new(set))
}

pub async fn run_httping(
    ips: Vec<IpAddr>,
    config: HttpingConfig,
    hooks: HooksRef,
) -> Result<Vec<CloudflareIpData>, String> {
    let total = ips.len();
    if total == 0 {
        return Ok(Vec::new());
    }

    let parsed_url = reqwest::Url::parse(&config.url)
        .map_err(|e| format!("解析测速地址失败 [{}]: {}", config.url, e))?;
    let scheme: Arc<str> = Arc::from(parsed_url.scheme());
    let path: Arc<str> = Arc::from(parsed_url.path());
    let original_host: Arc<str> = Arc::from(parsed_url.host_str().unwrap_or_default());

    let colo_filter = build_colo_filter(&config.httping_cf_colo);
    let routines = config.routines;

    let stream = stream::iter(ips).map(move |ip| {
        let scheme = Arc::clone(&scheme);
        let path = Arc::clone(&path);
        let original_host = Arc::clone(&original_host);
        let colo_filter = colo_filter.clone();
        let cfg = config.clone();
        async move { httping_ip(ip, &scheme, &path, &original_host, &cfg, &colo_filter).await }
    });

    let mut results = Vec::with_capacity(total);
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
        if done % 8 == 0 || done == total {
            hooks.tick("ping", done, total, &format!("可用 {}", results.len()));
        }
    }

    Ok(results)
}

/// 单 IP HTTPing：per-IP Client + resolve() 直连，保留 TLS SNI
async fn httping_ip(
    ip: IpAddr,
    scheme: &str,
    path: &str,
    original_host: &str,
    config: &HttpingConfig,
    colo_filter: &Option<Arc<HashSet<String>>>,
) -> Option<CloudflareIpData> {
    let url = format!("{}://{}:{}{}", scheme, original_host, config.port, path);

    let addr = SocketAddr::new(ip, config.port);
    let client = Client::builder()
        .timeout(HTTP_TIMEOUT)
        .danger_accept_invalid_certs(true)
        .no_proxy()
        .resolve(original_host, addr)
        .build()
        .ok()?;

    // 首个 HEAD：校验状态码并取地区码
    let req = client
        .head(&url)
        .header("User-Agent", USER_AGENT)
        .build()
        .ok()?;

    let colo = match client.execute(req).await {
        Ok(response) => {
            let sc = response.status().as_u16() as i32;
            let code_valid = config.httping_status_code > 0
                && (100..=599).contains(&config.httping_status_code);
            let is_valid = if !code_valid {
                (100..600).contains(&sc)
            } else {
                sc == config.httping_status_code
            };
            if !is_valid {
                if config.debug {
                    hooks_debug(ip, sc, &url);
                }
                return None;
            }
            let colo = get_header_colo(response.headers());
            drop(response);
            colo
        }
        Err(_) => return None,
    };

    if let Some(filter) = colo_filter {
        if !colo.is_empty() && !filter.contains(&colo.to_uppercase()) {
            return None;
        }
    }

    let mut received = 0u32;
    let mut total_delay = Duration::ZERO;
    for _ in 0..config.ping_times {
        let start = Instant::now();
        if let Ok(r) = client.head(&url).header("User-Agent", USER_AGENT).build() {
            if let Ok(resp) = client.execute(r).await {
                received += 1;
                total_delay += start.elapsed();
                drop(resp);
            }
        }
    }

    if received == 0 {
        return None;
    }

    Some(CloudflareIpData {
        ping_data: PingData {
            ip,
            sent: config.ping_times,
            received,
            delay: total_delay / received,
            colo,
        },
        download_speed: 0.0,
    })
}

fn hooks_debug(ip: IpAddr, status: i32, url: &str) {
    eprintln!("[调试] IP: {}, 延迟测速终止, HTTP 状态码: {}, 地址: {}", ip, status, url);
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn colo_from_cf_ray() {
        let mut h = HeaderMap::new();
        h.insert("cf-ray", HeaderValue::from_static("7bd32409eda7b020-SJC"));
        assert_eq!(get_header_colo(&h), "SJC");
    }

    #[test]
    fn colo_from_x77_pop() {
        let mut h = HeaderMap::new();
        h.insert("x-77-pop", HeaderValue::from_static("frankfurtDE"));
        assert_eq!(get_header_colo(&h), "DE");
    }

    #[test]
    fn colo_from_bunny() {
        let mut h = HeaderMap::new();
        h.insert("server", HeaderValue::from_static("BunnyCDN-TW1-1121"));
        assert_eq!(get_header_colo(&h), "TW");
    }
}
