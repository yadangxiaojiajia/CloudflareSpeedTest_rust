//! 参数模型 —— GUI / CLI / 配置文件三者同源。
//! 新增参数只需在此结构体加一个字段（同时加 #[arg] 与 #[serde] 属性）。

use crate::core::constants::{
    DEFAULT_DELAY_MAX, DEFAULT_DOWNLOAD_TEST_COUNT, DEFAULT_DOWNLOAD_TIMEOUT_SECS,
    DEFAULT_DOWNLOAD_URL, DEFAULT_INPUT_FILE, DEFAULT_MAX_LOSS_RATE, DEFAULT_OUTPUT_FILE,
    DEFAULT_PING_ROUTINES, DEFAULT_PING_TIMES, DEFAULT_PORT, DEFAULT_PRINT_NUM,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, clap::Args)]
#[serde(default)]
pub struct SpeedConfig {
    /// 延迟测速并发数（默认 200，最多 1000）
    #[arg(short = 'n', long, default_value_t = DEFAULT_PING_ROUTINES)]
    pub routines: usize,

    /// 每 IP 延迟测速次数
    #[arg(short = 't', long, default_value_t = DEFAULT_PING_TIMES)]
    pub ping_times: u32,

    /// 下载测速数量（达标 IP 数量）
    #[arg(long = "dn", default_value_t = DEFAULT_DOWNLOAD_TEST_COUNT)]
    pub download_num: usize,

    /// 下载测速时间上限（秒）
    #[arg(long = "dt", default_value_t = DEFAULT_DOWNLOAD_TIMEOUT_SECS)]
    pub download_time: u64,

    /// 测速端口
    #[arg(long = "tp", default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// 测速地址
    #[arg(long = "url", default_value_t = String::from(DEFAULT_DOWNLOAD_URL))]
    pub url: String,

    /// 使用 HTTPing 模式（默认 TCPing）
    #[arg(long = "httping", action = clap::ArgAction::SetTrue)]
    pub httping: bool,

    /// 有效 HTTP 状态码（仅 HTTPing 模式，0 表示接受 100~599）
    #[arg(long = "httping-code", default_value = "0")]
    pub httping_code: i32,

    /// 地区码过滤，逗号分隔（仅 HTTPing 模式）
    #[arg(long = "cfcolo", default_value = "")]
    pub cfcolo: String,

    /// 平均延迟上限（ms）
    #[arg(long = "tl", default_value_t = DEFAULT_DELAY_MAX)]
    pub max_delay: u64,

    /// 平均延迟下限（ms）
    #[arg(long = "tll", default_value = "0")]
    pub min_delay: u64,

    /// 丢包率上限（0.00~1.00）
    #[arg(long = "tlr", default_value_t = DEFAULT_MAX_LOSS_RATE)]
    pub max_loss_rate: f32,

    /// 下载速度下限（MB/s，0 表示不做速度筛选）
    #[arg(long = "sl", default_value = "0.0")]
    pub min_speed: f64,

    /// 显示结果数量（0 表示不限制）
    #[arg(short = 'p', long, default_value_t = DEFAULT_PRINT_NUM)]
    pub print_num: usize,

    /// IP 段数据文件
    #[arg(short = 'f', long, default_value_t = String::from(DEFAULT_INPUT_FILE))]
    pub ip_file: String,

    /// 直接指定 IP 段数据（逗号/换行分隔，优先于文件）
    #[arg(long = "ip", default_value = "")]
    pub ip_text: String,

    /// 测速全部 IP（不做抽样）
    #[arg(long = "allip", action = clap::ArgAction::SetTrue)]
    pub all_ip: bool,

    /// 输出结果文件
    #[arg(short = 'o', long, default_value_t = String::from(DEFAULT_OUTPUT_FILE))]
    pub output: String,

    /// 禁用下载测速
    #[arg(long = "dd", action = clap::ArgAction::SetTrue)]
    pub disable_download: bool,

    /// 调试输出
    #[arg(long = "debug", action = clap::ArgAction::SetTrue)]
    pub debug: bool,
}

impl Default for SpeedConfig {
    fn default() -> Self {
        Self {
            routines: DEFAULT_PING_ROUTINES,
            ping_times: DEFAULT_PING_TIMES,
            download_num: DEFAULT_DOWNLOAD_TEST_COUNT,
            download_time: DEFAULT_DOWNLOAD_TIMEOUT_SECS,
            port: DEFAULT_PORT,
            url: DEFAULT_DOWNLOAD_URL.to_string(),
            httping: false,
            httping_code: 0,
            cfcolo: String::new(),
            max_delay: DEFAULT_DELAY_MAX,
            min_delay: 0,
            max_loss_rate: DEFAULT_MAX_LOSS_RATE,
            min_speed: 0.0,
            print_num: DEFAULT_PRINT_NUM,
            ip_file: DEFAULT_INPUT_FILE.to_string(),
            ip_text: String::new(),
            all_ip: false,
            output: DEFAULT_OUTPUT_FILE.to_string(),
            disable_download: false,
            debug: false,
        }
    }
}

impl SpeedConfig {
    /// 把 GUI 参数还原成等价的命令行参数串，方便复制 / 定时任务复用
    pub fn to_cli_args(&self) -> Vec<String> {
        let mut a = Vec::new();
        let d = SpeedConfig::default();
        if self.routines != d.routines {
            a.push(format!("-n {}", self.routines));
        }
        if self.ping_times != d.ping_times {
            a.push(format!("-t {}", self.ping_times));
        }
        if self.download_num != d.download_num {
            a.push(format!("--dn {}", self.download_num));
        }
        if self.download_time != d.download_time {
            a.push(format!("--dt {}", self.download_time));
        }
        if self.port != d.port {
            a.push(format!("--tp {}", self.port));
        }
        if !self.url.is_empty() && self.url != d.url {
            a.push(format!("--url {}", self.url));
        }
        if self.httping {
            a.push("--httping".into());
        }
        if self.httping_code != d.httping_code {
            a.push(format!("--httping-code {}", self.httping_code));
        }
        if !self.cfcolo.is_empty() {
            a.push(format!("--cfcolo {}", self.cfcolo));
        }
        if self.max_delay != d.max_delay {
            a.push(format!("--tl {}", self.max_delay));
        }
        if self.min_delay != d.min_delay {
            a.push(format!("--tll {}", self.min_delay));
        }
        if (self.max_loss_rate - d.max_loss_rate).abs() > f32::EPSILON {
            a.push(format!("--tlr {}", self.max_loss_rate));
        }
        if (self.min_speed - d.min_speed).abs() > f64::EPSILON {
            a.push(format!("--sl {}", self.min_speed));
        }
        if self.print_num != d.print_num {
            a.push(format!("-p {}", self.print_num));
        }
        if !self.ip_file.is_empty() && self.ip_file != d.ip_file {
            a.push(format!("-f {}", self.ip_file));
        }
        if !self.ip_text.is_empty() {
            a.push(format!("--ip {}", self.ip_text.replace('\n', ",")));
        }
        if self.all_ip {
            a.push("--allip".into());
        }
        if !self.output.is_empty() && self.output != d.output {
            a.push(format!("-o {}", self.output));
        }
        if self.disable_download {
            a.push("--dd".into());
        }
        if self.debug {
            a.push("--debug".into());
        }
        a
    }
}

/// 配置文件路径：<数据目录>/config.json
pub fn config_path(dir: &std::path::Path) -> PathBuf {
    dir.join("config.json")
}

pub fn load(dir: &std::path::Path) -> SpeedConfig {
    let path = config_path(dir);
    match std::fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => SpeedConfig::default(),
    }
}

pub fn save(dir: &std::path::Path, cfg: &SpeedConfig) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(config_path(dir), s).map_err(|e| e.to_string())
}
