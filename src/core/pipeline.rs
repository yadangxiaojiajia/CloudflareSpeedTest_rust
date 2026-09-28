//! 测速主流程编排（等价于原 CLI 的 main 流程）：加载 → 延迟测速 → 排序筛选 → 下载测速

use crate::config::SpeedConfig;
use crate::core::constants::DEFAULT_DELAY_MAX;
use crate::core::data::{CloudflareIpData, IpDataSet};
use crate::core::download::{run_download, DownloadConfig};
use crate::core::httping::{run_httping, HttpingConfig};
use crate::core::ip;
use crate::core::tcping::{run_tcping, TcpingConfig};
use crate::core::HooksRef;
use std::time::Duration;

pub async fn run_speedtest(
    cfg: &SpeedConfig,
    hooks: HooksRef,
) -> Result<Vec<CloudflareIpData>, String> {
    // ---- 1. 加载 IP ----
    let source = if cfg.ip_text.is_empty() {
        cfg.ip_file.clone()
    } else {
        "直接输入的 IP 段".to_string()
    };
    hooks.log("load", &format!("正在加载 IP 段：{}", source));

    let ips = ip::load_ip_ranges(&cfg.ip_file, &cfg.ip_text, cfg.all_ip)?;
    if ips.is_empty() {
        return Err("未加载到任何 IP 地址，请检查 IP 数据文件或参数".to_string());
    }
    hooks.log("load", &format!("共加载 {} 个待测试 IP", ips.len()));
    if hooks.cancelled() {
        return Ok(Vec::new());
    }

    // ---- 2. 延迟测速 ----
    let ping_data = if cfg.httping {
        let mut hcfg = HttpingConfig {
            routines: cfg.routines,
            port: cfg.port,
            ping_times: cfg.ping_times,
            url: cfg.url.clone(),
            httping_status_code: cfg.httping_code,
            httping_cf_colo: cfg.cfcolo.clone(),
            debug: cfg.debug,
        };
        hcfg.normalize();
        hooks.log(
            "ping",
            &format!(
                "开始延迟测速（模式: HTTP, 端口: {}, 范围: {}~{} ms, 丢包: {:.2}）",
                hcfg.port, cfg.min_delay, cfg.max_delay, cfg.max_loss_rate
            ),
        );
        run_httping(ips, hcfg, hooks.clone()).await?
    } else {
        let mut tcfg = TcpingConfig {
            routines: cfg.routines,
            port: cfg.port,
            ping_times: cfg.ping_times,
        };
        tcfg.normalize();
        hooks.log(
            "ping",
            &format!(
                "开始延迟测速（模式: TCP, 端口: {}, 范围: {}~{} ms, 丢包: {:.2}）",
                tcfg.port, cfg.min_delay, cfg.max_delay, cfg.max_loss_rate
            ),
        );
        run_tcping(ips, tcfg, hooks.clone()).await
    };

    if hooks.cancelled() {
        hooks.log("ping", "收到停止指令，中断延迟测速");
        return Ok(Vec::new());
    }

    // ---- 3. 排序 + 筛选（IpDataSet::new 排序一次，filter 只 retain） ----
    let set = IpDataSet::new(ping_data).filter_max_loss_rate(cfg.max_loss_rate);
    let set = if cfg.max_delay < DEFAULT_DELAY_MAX || cfg.min_delay > 0 {
        set.filter_max_delay(Duration::from_millis(cfg.max_delay))
            .filter_min_delay(Duration::from_millis(cfg.min_delay))
    } else {
        set
    };

    hooks.log("filter", &format!("延迟测速后剩余 {} 个可用 IP", set.len()));

    if set.is_empty() {
        hooks.log("filter", "没有符合延迟/丢包条件的 IP，跳过下载测速");
        return Ok(Vec::new());
    }

    let filtered = set.into_inner();

    // ---- 4. 下载测速 ----
    let mut dl_cfg = DownloadConfig {
        url: cfg.url.clone(),
        timeout: Duration::from_secs(cfg.download_time),
        test_count: cfg.download_num,
        min_speed: cfg.min_speed,
        port: cfg.port,
        disable: cfg.disable_download,
        debug: cfg.debug,
    };
    dl_cfg.normalize();

    if !dl_cfg.disable {
        hooks.log(
            "download",
            &format!(
                "开始下载测速（下限: {:.2} MB/s, 数量: {}, 队列: {}）",
                dl_cfg.min_speed,
                dl_cfg.test_count,
                filtered.len()
            ),
        );
    }

    let speed_set = run_download(filtered, dl_cfg, hooks).await;
    Ok(speed_set.into_inner())
}
