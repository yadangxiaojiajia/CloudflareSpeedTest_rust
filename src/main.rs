//! project-c —— Rust CloudflareSpeedTest 内核 + 浏览器 Web GUI

mod api;
mod config;
mod core;
mod task;
mod upload;
mod utils;
mod web;

use clap::{Parser, Subcommand};
use config::SpeedConfig;
use core::data::{bytes_to_mb, export_csv};
use core::{run_speedtest, NoopHooks, HooksRef};
use std::net::SocketAddr;
use std::sync::Arc;
use task::Runner;
use utils::{data_dir, resolve};

#[derive(Parser, Debug)]
#[command(
    name = "project-c",
    version,
    about = "Cloudflare CDN IP 延迟/速度测速工具（Rust 内核 + Web 控制台）"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// 启动 Web 控制台（默认命令）
    Web {
        /// 监听地址
        #[arg(long, default_value = "127.0.0.1:8080")]
        listen: String,
        /// 不自动打开浏览器
        #[arg(long, action = clap::ArgAction::SetTrue)]
        no_open: bool,
    },
    /// 命令行跑一次测速（参数与界面完全一致）
    Test {
        #[command(flatten)]
        cfg: SpeedConfig,
    },
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("错误: {}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let dir = data_dir();
    let cmd = cli.cmd.unwrap_or(Cmd::Web {
        listen: "127.0.0.1:8080".into(),
        no_open: false,
    });

    match cmd {
        Cmd::Web { listen, no_open } => web(&dir, &listen, !no_open).await,
        Cmd::Test { cfg } => cli_test(&dir, cfg).await,
    }
}

async fn web(dir: &std::path::Path, listen: &str, open: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("# project-c v{}", env!("CARGO_PKG_VERSION"));
    println!("数据目录: {}", dir.display());

    let runner = Runner::new(dir.to_path_buf());
    let app = api::routes(api::AppState {
        runner,
        data_dir: dir.to_path_buf(),
    });

    let addr: SocketAddr = listen.parse()?;
    let url = format!("http://{}", addr);
    println!("Web 控制台: {}", url);
    if open {
        open_browser(&url);
    }

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// 命令行模式：直接调用内核，不经 Web 层
async fn cli_test(dir: &std::path::Path, cfg: SpeedConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!("# CloudflareSpeedTest v{} (project-c)\n", env!("CARGO_PKG_VERSION"));

    let hooks: HooksRef = Arc::new(NoopHooks);
    match run_speedtest(&cfg, hooks).await {
        Ok(rows) => {
            let shown = if cfg.print_num > 0 {
                rows.len().min(cfg.print_num)
            } else {
                rows.len()
            };
            println!(
                "{:<40}{:<8}{:<8}{:<8}{:<12}{:<18}{:<8}",
                "IP 地址", "已发送", "已接收", "丢包率", "平均延迟", "下载速度(MB/s)", "地区码"
            );
            for d in rows.iter().take(shown) {
                println!(
                    "{:<40}{:<8}{:<8}{:<8}{:<12}{:<18}{:<8}",
                    d.ping_data.ip.to_string(),
                    d.ping_data.sent,
                    d.ping_data.received,
                    format!("{:.2}", (d.loss_rate() * 100.0).round() / 100.0),
                    format!("{}ms", d.ping_data.delay.as_millis()),
                    format!("{:.2}", bytes_to_mb(d.download_speed)),
                    d.ping_data.colo
                );
            }
            if rows.is_empty() {
                println!("[信息] 完整测速结果 IP 数量为 0。");
            }
            let out = resolve(&cfg.output);
            export_csv(&rows, &out.to_string_lossy())?;
            println!("\n结果已写入: {}", out.display());
        }
        Err(e) => eprintln!("测速失败: {}", e),
    }
    let _ = dir;
    Ok(())
}

fn open_browser(url: &str) {
    let _ = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
}
