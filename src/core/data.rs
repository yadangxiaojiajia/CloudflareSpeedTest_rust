//! 数据结构、排序过滤与 CSV 导出 —— 逻辑与上游 Rust 核心保持一致

use std::cmp::Ordering;
use std::net::IpAddr;
use std::time::Duration;

/// 单 IP 延迟测速结果
#[derive(Debug, Clone)]
pub struct PingData {
    pub ip: IpAddr,
    pub sent: u32,
    pub received: u32,
    pub delay: Duration, // 平均延迟
    pub colo: String,    // 地区码，未知为空
}

/// 含下载速度的完整结果
#[derive(Debug, Clone)]
pub struct CloudflareIpData {
    pub ping_data: PingData,
    pub download_speed: f64, // bytes per second
}

impl CloudflareIpData {
    pub fn loss_rate(&self) -> f32 {
        let lost = self.ping_data.sent - self.ping_data.received;
        if self.ping_data.sent == 0 {
            return 1.0;
        }
        lost as f32 / self.ping_data.sent as f32
    }
}

/// 延迟结果集 —— 丢包率 → 延迟 升序；`filter_*` 只做 retain，不改变顺序
#[derive(Debug)]
pub struct IpDataSet(pub Vec<CloudflareIpData>);

impl IpDataSet {
    pub fn new(mut data: Vec<CloudflareIpData>) -> Self {
        data.sort_by(|a, b| {
            a.loss_rate()
                .partial_cmp(&b.loss_rate())
                .unwrap_or(Ordering::Equal)
                .then(a.ping_data.delay.cmp(&b.ping_data.delay))
        });
        Self(data)
    }

    pub fn filter_max_delay(mut self, max: Duration) -> Self {
        self.0.retain(|x| x.ping_data.delay <= max);
        self
    }

    pub fn filter_min_delay(mut self, min: Duration) -> Self {
        self.0.retain(|x| x.ping_data.delay >= min);
        self
    }

    pub fn filter_max_loss_rate(mut self, max_rate: f32) -> Self {
        self.0.retain(|x| x.loss_rate() <= max_rate);
        self
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_inner(self) -> Vec<CloudflareIpData> {
        self.0
    }
}

/// 下载速度结果集 —— 速度降序
#[derive(Debug)]
pub struct DownloadSpeedSet(pub Vec<CloudflareIpData>);

impl DownloadSpeedSet {
    pub fn new(mut data: Vec<CloudflareIpData>) -> Self {
        data.sort_by(|a, b| {
            b.download_speed
                .partial_cmp(&a.download_speed)
                .unwrap_or(Ordering::Equal)
        });
        Self(data)
    }

    pub fn into_inner(self) -> Vec<CloudflareIpData> {
        self.0
    }
}

/// 导出 CSV：IP 地址,已发送,已接收,丢包率,平均延迟,下载速度(MB/s),地区码
pub fn export_csv(data: &[CloudflareIpData], output: &str) -> Result<usize, String> {
    if output.is_empty() || output == " " {
        return Ok(0);
    }
    let mut wtr = csv::Writer::from_path(output)
        .map_err(|e| format!("创建文件 [{}] 失败: {}", output, e))?;

    wtr.write_record([
        "IP 地址",
        "已发送",
        "已接收",
        "丢包率",
        "平均延迟",
        "下载速度(MB/s)",
        "地区码",
    ])
    .map_err(|e| format!("写入 CSV 表头失败: {}", e))?;

    for d in data {
        let ip = d.ping_data.ip.to_string();
        let sent = d.ping_data.sent.to_string();
        let received = d.ping_data.received.to_string();
        let loss = format!("{:.2}", d.loss_rate());
        let delay = format!("{}", d.ping_data.delay.as_millis());
        let speed = format!("{:.2}", bytes_to_mb(d.download_speed));
        let colo = d.ping_data.colo.clone();
        wtr.write_record([ip, sent, received, loss, delay, speed, colo])
            .map_err(|e| format!("写入 CSV 记录失败: {}", e))?;
    }
    wtr.flush().map_err(|e| format!("刷新 CSV 失败: {}", e))?;
    Ok(data.len())
}

#[inline]
pub fn bytes_to_mb(bytes_per_sec: f64) -> f64 {
    bytes_per_sec / 1024.0 / 1024.0
}
