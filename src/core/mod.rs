//! 测速内核：算法与上游 Rust 核心一致，仅增加进度回调与取消钩子

pub mod constants;
pub mod data;
pub mod download;
pub mod httping;
pub mod ip;
pub mod pipeline;
pub mod tcping;

use crate::core::data::CloudflareIpData;
use std::sync::Arc;

/// 测速过程中的回调钩子：Web 层实现，内核按 B 的算法推进时顺手调用
pub trait Hooks: Send + Sync {
    /// 进度推进：stage = load / ping / filter / download
    fn tick(&self, stage: &str, current: usize, total: usize, note: &str);
    /// 一行日志
    fn log(&self, stage: &str, message: &str);
    /// 下载测速每测完一个 IP 回调一次
    fn item(&self, data: &CloudflareIpData);
    /// 是否已请求停止
    fn cancelled(&self) -> bool;
}

pub type HooksRef = Arc<dyn Hooks>;

/// 无任何输出的钩子实现，CLI 模式使用
pub struct NoopHooks;

impl Hooks for NoopHooks {
    fn tick(&self, _stage: &str, _current: usize, _total: usize, _note: &str) {}
    fn log(&self, _stage: &str, _message: &str) {}
    fn item(&self, _data: &CloudflareIpData) {}
    fn cancelled(&self) -> bool {
        false
    }
}

pub use pipeline::run_speedtest;
