//! 优选结果上报：Cloudflare Worker（cfnew preferred-ips 接口）与 GitHub 仓库两个出口

use crate::task::Row;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 上报配置，独立于测速参数，持久化到 <数据目录>/report.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportConfig {
    /// Worker 域名，如 example.workers.dev
    pub worker_domain: String,
    /// UUID 或自定义路径
    pub uuid: String,
    /// GitHub 仓库，owner/repo
    pub github_repo: String,
    /// GitHub Token（只落盘，不下发给前端）
    pub github_token: String,
    /// 仓库内文件路径
    pub github_path: String,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            worker_domain: String::new(),
            uuid: String::new(),
            github_repo: String::new(),
            github_token: String::new(),
            github_path: "cloudflare_ips.txt".into(),
        }
    }
}

pub fn report_path(dir: &Path) -> PathBuf {
    dir.join("report.json")
}

pub fn load(dir: &Path) -> ReportConfig {
    match std::fs::read_to_string(report_path(dir)) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => ReportConfig::default(),
    }
}

pub fn save(dir: &Path, cfg: &ReportConfig) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(report_path(dir), s).map_err(|e| e.to_string())
}

// ---------- 地区码中文名 ----------

const COLO_NAMES: &[(&str, &str)] = &[
    ("HKG", "香港"), ("TPE", "台北"), ("NRT", "东京成田"), ("KIX", "大阪"), ("ITM", "大阪伊丹"),
    ("FUK", "福冈"), ("ICN", "首尔仁川"), ("SIN", "新加坡"), ("BKK", "曼谷"), ("HAN", "河内"),
    ("SGN", "胡志明市"), ("MNL", "马尼拉"), ("CGK", "雅加达"), ("KUL", "吉隆坡"), ("RGN", "仰光"),
    ("PNH", "金边"), ("BOM", "孟买"), ("DEL", "新德里"), ("MAA", "金奈"), ("BLR", "班加罗尔"),
    ("HYD", "海得拉巴"), ("CCU", "加尔各答"), ("SYD", "悉尼"), ("MEL", "墨尔本"), ("BNE", "布里斯班"),
    ("PER", "珀斯"), ("AKL", "奥克兰"), ("LAX", "洛杉矶"), ("SJC", "圣何塞"), ("SEA", "西雅图"),
    ("SFO", "旧金山"), ("PDX", "波特兰"), ("SAN", "圣地亚哥"), ("PHX", "凤凰城"), ("LAS", "拉斯维加斯"),
    ("EWR", "纽瓦克"), ("IAD", "华盛顿"), ("BOS", "波士顿"), ("PHL", "费城"), ("ATL", "亚特兰大"),
    ("MIA", "迈阿密"), ("MCO", "奥兰多"), ("ORD", "芝加哥"), ("DFW", "达拉斯"), ("IAH", "休斯顿"),
    ("DEN", "丹佛"), ("MSP", "明尼阿波利斯"), ("DTW", "底特律"), ("STL", "圣路易斯"), ("MCI", "堪萨斯城"),
    ("YYZ", "多伦多"), ("YVR", "温哥华"), ("YUL", "蒙特利尔"), ("LHR", "伦敦"), ("CDG", "巴黎"),
    ("FRA", "法兰克福"), ("AMS", "阿姆斯特丹"), ("BRU", "布鲁塞尔"), ("ZRH", "苏黎世"), ("VIE", "维也纳"),
    ("MUC", "慕尼黑"), ("DUS", "杜塞尔多夫"), ("HAM", "汉堡"), ("MAD", "马德里"), ("BCN", "巴塞罗那"),
    ("MXP", "米兰"), ("FCO", "罗马"), ("ATH", "雅典"), ("LIS", "里斯本"), ("ARN", "斯德哥尔摩"),
    ("CPH", "哥本哈根"), ("OSL", "奥斯陆"), ("HEL", "赫尔辛基"), ("WAW", "华沙"), ("PRG", "布拉格"),
    ("BUD", "布达佩斯"), ("OTP", "布加勒斯特"), ("SOF", "索非亚"), ("DXB", "迪拜"), ("TLV", "特拉维夫"),
    ("BAH", "巴林"), ("AMM", "安曼"), ("KWI", "科威特"), ("DOH", "多哈"), ("MCT", "马斯喀特"),
    ("GRU", "圣保罗"), ("GIG", "里约热内卢"), ("EZE", "布宜诺斯艾利斯"), ("BOG", "波哥大"), ("LIM", "利马"),
    ("SCL", "圣地亚哥"), ("JNB", "约翰内斯堡"), ("CPT", "开普敦"), ("CAI", "开罗"), ("LOS", "拉各斯"),
    ("NBO", "内罗毕"), ("ACC", "阿克拉"),
];

fn colo_name(code: &str) -> String {
    if let Some((_, name)) = COLO_NAMES.iter().find(|(c, _)| *c == code) {
        return (*name).to_string();
    }
    if code.is_empty() {
        "未知地区".to_string()
    } else {
        code.to_string()
    }
}

/// 节点备注，如「香港-8.34MB/s」：选优选 IP 看的是速度，延迟放名字里参考价值低
pub fn node_name(r: &Row) -> String {
    format!("{}-{:.2}MB/s", colo_name(&r.colo), r.speed)
}

fn limit_rows(rows: &[Row], limit: usize) -> &[Row] {
    if limit > 0 && limit < rows.len() {
        &rows[..limit]
    } else {
        rows
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

fn truncate(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let cut: String = s.chars().take(n).collect();
        format!("{}...", cut)
    }
}

// ---------- Worker 上报 ----------

/// 拼 cfnew 的优选 IP 接口地址；允许显式 http:// 用于本地自建
fn worker_url(domain: &str, uuid: &str) -> String {
    let d = domain.trim();
    let (scheme, rest) = if let Some(r) = d.strip_prefix("http://") {
        ("http", r)
    } else if let Some(r) = d.strip_prefix("https://") {
        ("https", r)
    } else {
        ("https", d)
    };
    let host = rest.split('/').next().unwrap_or("").trim_end_matches('/');
    let path = uuid.trim().trim_matches('/');
    format!("{}://{}/{}/api/preferred-ips", scheme, host, path)
}

#[derive(Serialize)]
struct ApiItem {
    ip: String,
    port: u16,
    name: String,
}

pub async fn upload_worker(
    cfg: &ReportConfig,
    rows: &[Row],
    limit: usize,
    clear: bool,
) -> Result<usize, String> {
    if cfg.worker_domain.trim().is_empty() || cfg.uuid.trim().is_empty() {
        return Err("请先填写 Worker 域名和 UUID".into());
    }
    let rows = limit_rows(rows, limit);
    if rows.is_empty() {
        return Err("没有可上报的结果".into());
    }
    let client = client()?;
    let url = worker_url(&cfg.worker_domain, &cfg.uuid);

    if clear {
        let resp = client
            .delete(&url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(r#"{"all":true}"#)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("清空失败 HTTP {}: {}", status, truncate(&body, 160)));
        }
    }

    let items: Vec<ApiItem> = rows
        .iter()
        .map(|r| ApiItem {
            ip: r.ip.clone(),
            port: r.port,
            name: r.remark.clone(),
        })
        .collect();
    let payload = serde_json::to_string(&items).map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("上传失败 HTTP {}: {}", status, truncate(&body, 200)));
    }
    Ok(items.len())
}

// ---------- GitHub 上报 ----------

/// GitHub REST API 强制要求 User-Agent 头，缺失直接 403
const GH_USER_AGENT: &str = concat!("CloudflareSpeedTest_rust/", env!("CARGO_PKG_VERSION"));

/// 把优选列表写入 GitHub 仓库文件，已存在则带 sha 更新
pub async fn upload_github(
    cfg: &ReportConfig,
    token: &str,
    rows: &[Row],
    limit: usize,
) -> Result<usize, String> {
    let repo = cfg.github_repo.trim().trim_matches('/').to_string();
    if repo.is_empty() || token.trim().is_empty() {
        return Err("请先填写 GitHub 仓库和 Token".into());
    }
    if !repo.contains('/') {
        return Err("仓库格式应为 owner/repo".into());
    }
    let path = if cfg.github_path.trim().is_empty() {
        "cloudflare_ips.txt".to_string()
    } else {
        cfg.github_path.trim().to_string()
    };
    let rows = limit_rows(rows, limit);
    if rows.is_empty() {
        return Err("没有可上报的结果".into());
    }

    let mut content = String::new();
    for r in rows {
        content.push_str(&format!("{}:{}#{}\n", r.ip, r.port, r.remark));
    }
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &content);
    let api = format!("https://api.github.com/repos/{}/contents/{}", repo, path);

    let client = client()?;
    // 文件已存在时必须带上 sha 才能覆盖
    let sha = {
        let resp = client
            .get(&api)
            .header(reqwest::header::USER_AGENT, GH_USER_AGENT)
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token))
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .send()
            .await;
        match resp {
            Ok(resp) if resp.status().is_success() => {
                let text = resp.text().await.unwrap_or_default();
                let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                v.get("sha").and_then(|s| s.as_str()).map(|s| s.to_string())
            }
            _ => None,
        }
    };

    let mut payload = serde_json::json!({
        "message": format!("更新优选 IP ({} 个)", rows.len()),
        "content": encoded,
    });
    if let Some(sha) = sha {
        payload["sha"] = serde_json::Value::String(sha);
    }
    let resp = client
        .put(&api)
        .header(reqwest::header::USER_AGENT, GH_USER_AGENT)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {}", token))
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(payload.to_string())
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!("GitHub 上传失败 HTTP {}: {}", status, truncate(&body, 200)));
    }
    Ok(rows.len())
}
