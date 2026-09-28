//! 测速任务调度：单任务串行 + 取消 + 事件广播（事件格式与参考 Web UI 对齐）

use crate::config::SpeedConfig;
use crate::core::data::{bytes_to_mb, export_csv, CloudflareIpData};
use crate::core::{run_speedtest, Hooks};
use crate::utils::now_ms;
use serde::Serialize;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

const HISTORY_LIMIT: usize = 500;
const BROADCAST_CAP: usize = 256;

/// 一行测速结果
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub ip: String,
    pub sent: u32,
    pub received: u32,
    pub loss: f32,
    pub delay: u64,
    pub speed: f64,
    pub colo: String,
    pub port: u16,
    /// 节点备注，如「东京成田-37.79MB/s」，复制/上报共用
    pub remark: String,
}

impl Row {
    fn from_data(d: &CloudflareIpData, port: u16) -> Self {
        let mut row = Row {
            ip: d.ping_data.ip.to_string(),
            sent: d.ping_data.sent,
            received: d.ping_data.received,
            loss: (d.loss_rate() * 100.0).round() / 100.0,
            delay: d.ping_data.delay.as_millis() as u64,
            speed: (bytes_to_mb(d.download_speed) * 100.0).round() / 100.0,
            colo: d.ping_data.colo.clone(),
            port,
            remark: String::new(),
        };
        row.remark = crate::upload::node_name(&row);
        row
    }
}

/// 推送给前端的事件
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    #[serde(rename = "type")]
    pub kind: String,
    pub stage: String,
    pub message: String,
    pub current: usize,
    pub total: usize,
    pub result: Option<Row>,
    pub results: Option<Vec<Row>>,
    pub finished: bool,
    pub at: i64,
}

impl Event {
    fn new(kind: &str, stage: &str, message: &str) -> Self {
        Event {
            kind: kind.into(),
            stage: stage.into(),
            message: message.into(),
            current: 0,
            total: 0,
            result: None,
            results: None,
            finished: false,
            at: now_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub stage: String,
    pub message: String,
    pub at: i64,
}

struct State {
    running: bool,
    results: Vec<Row>,
    logs: VecDeque<LogLine>,
}

pub struct Runner {
    tx: broadcast::Sender<Event>,
    state: Arc<Mutex<State>>,
    cancel: Arc<AtomicBool>,
    data_dir: PathBuf,
}

/// 把内核回调接成事件：tick → progress，log → log，item → 逐条 result
struct Bridge {
    tx: broadcast::Sender<Event>,
    state: Arc<Mutex<State>>,
    cancel: Arc<AtomicBool>,
    port: u16,
}

impl Bridge {
    fn emit(&self, e: Event) {
        let _ = self.tx.send(e);
    }
}

impl Hooks for Bridge {
    fn tick(&self, stage: &str, current: usize, total: usize, note: &str) {
        self.emit(Event {
            current,
            total,
            ..Event::new("progress", stage, note)
        });
    }

    fn log(&self, stage: &str, message: &str) {
        if let Ok(mut s) = self.state.lock() {
            s.logs.push_back(LogLine {
                stage: stage.into(),
                message: message.into(),
                at: now_ms(),
            });
            if s.logs.len() > HISTORY_LIMIT {
                s.logs.pop_front();
            }
        }
        self.emit(Event::new("log", stage, message));
    }

    fn item(&self, data: &CloudflareIpData) {
        let row = Row::from_data(data, self.port);
        if let Ok(mut s) = self.state.lock() {
            s.results.push(row.clone());
        }
        self.emit(Event {
            result: Some(row),
            message: format!("{} 下载速度 {:.2} MB/s", data.ping_data.ip, bytes_to_mb(data.download_speed)),
            ..Event::new("result", "download", "")
        });
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}

impl Runner {
    pub fn new(data_dir: PathBuf) -> Arc<Self> {
        let (tx, _) = broadcast::channel(BROADCAST_CAP);
        Arc::new(Runner {
            tx,
            state: Arc::new(Mutex::new(State {
                running: false,
                results: Vec::new(),
                logs: VecDeque::new(),
            })),
            cancel: Arc::new(AtomicBool::new(false)),
            data_dir,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub fn running(&self) -> bool {
        self.state.lock().map(|s| s.running).unwrap_or(false)
    }

    pub fn results(&self) -> Vec<Row> {
        self.state.lock().map(|s| s.results.clone()).unwrap_or_default()
    }

    pub fn logs(&self) -> Vec<LogLine> {
        self.state
            .lock()
            .map(|s| s.logs.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// 启动一次测速；已有任务在跑时返回 false
    pub fn start(self: &Arc<Self>, cfg: SpeedConfig) -> Result<bool, String> {
        {
            let mut s = self.state.lock().map_err(|_| "状态锁损坏".to_string())?;
            if s.running {
                return Ok(false);
            }
            s.running = true;
            s.results.clear();
            s.logs.clear();
        }
        self.cancel.store(false, Ordering::SeqCst);

        let bridge = Arc::new(Bridge {
            tx: self.tx.clone(),
            state: self.state.clone(),
            cancel: self.cancel.clone(),
            port: cfg.port,
        }) as Arc<dyn Hooks>;

        let state = self.state.clone();
        let tx = self.tx.clone();
        let data_dir = self.data_dir.clone();
        let output = cfg.output.clone();
        let print_num = cfg.print_num;
        let port = cfg.port;
        let cancel_flag = self.cancel.clone();

        tokio::spawn(async move {
            let outcome = run_speedtest(&cfg, bridge).await;

            let mut e = if cancel_flag.load(Ordering::SeqCst) {
                let mut stopped = Event::new("error", "done", "测速已停止");
                stopped.finished = true;
                stopped
            } else {
                match outcome {
                    Ok(rows) => {
                        let out = resolve_in(&data_dir, &output);
                        match export_csv(&rows, &out.to_string_lossy()) {
                            Ok(n) => {
                                let shown: Vec<Row> = rows
                                    .iter()
                                    .take(if print_num > 0 { print_num } else { rows.len() })
                                    .map(|d| Row::from_data(d, port))
                                    .collect();
                                let mut ev = Event::new(
                                    "done",
                                    "done",
                                    &format!("测速完成，{} 个结果，已写入 {}", n, out.display()),
                                );
                                ev.results = Some(shown);
                                ev.current = n;
                                ev.total = n;
                                ev.finished = true;
                                ev
                            }
                            Err(err) => {
                                let mut ev = Event::new("error", "done", &err);
                                ev.finished = true;
                                ev
                            }
                        }
                    }
                    Err(err) => {
                        let mut ev = Event::new("error", "error", &err);
                        ev.finished = true;
                        ev
                    }
                }
            };

            if e.kind == "done" {
                if let Some(rows) = e.results.take() {
                    if let Ok(mut s) = state.lock() {
                        s.results = rows.clone();
                    }
                    e.results = Some(rows);
                }
            }

            let _ = tx.send(e);
            if let Ok(mut s) = state.lock() {
                s.running = false;
            }
        });

        Ok(true)
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        let _ = self.tx.send(Event {
            results: None,
            ..Event::new("log", "cancel", "收到停止指令，正在收尾…")
        });
    }
}

fn resolve_in(dir: &std::path::Path, name: &str) -> PathBuf {
    let p = PathBuf::from(name);
    if p.is_absolute() {
        p
    } else {
        dir.join(p)
    }
}
