//! IP 段加载与 CIDR 展开 —— 全量展开，不抽样、不静默截断

use crate::core::constants::DEFAULT_INPUT_FILE;
use ipnetwork::IpNetwork;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::path::Path;

/// 全量测速的硬上限。超过即报错，绝不静默截断——
/// 用户明确要求「无论多少个全部测完」，所以宁可拒绝也不偷偷少测。
pub const MAX_POOL_SIZE: usize = 1_000_000;

/// 加载待测试的 IP 列表。
/// `ip_text` 非空时优先解析它（逗号 / 换行 / 分号分隔），否则读取 `ip_file`。
/// 每个 CIDR 都会展开成具体 IP，重复项按首次出现顺序去重。
pub fn load_ip_ranges(ip_file: &str, ip_text: &str) -> Result<Vec<IpAddr>, String> {
    let mut ips = Vec::new();

    if !ip_text.is_empty() {
        for part in ip_text.split([',', '\n', ';']) {
            let part = part.trim();
            if !part.is_empty() {
                expand_ip_range(part, &mut ips)?;
            }
        }
    } else {
        let file_path = if ip_file.is_empty() {
            DEFAULT_INPUT_FILE
        } else {
            ip_file
        };
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(format!("IP 段文件 [{}] 不存在", file_path));
        }
        let file = File::open(path).map_err(|e| format!("打开文件 [{}] 失败: {}", file_path, e))?;
        for line in BufReader::new(file).lines() {
            let line = line.unwrap_or_default();
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            expand_ip_range(line, &mut ips)?;
        }
    }

    let mut seen = HashSet::with_capacity(ips.len());
    ips.retain(|ip| seen.insert(*ip));
    Ok(ips)
}

/// 解析一条 CIDR 或单个 IP，展开成网段内的每一个 IP
fn expand_ip_range(cidr: &str, ips: &mut Vec<IpAddr>) -> Result<(), String> {
    let network: IpNetwork = cidr
        .parse()
        .map_err(|e| format!("解析 CIDR [{}] 失败: {}", cidr, e))?;
    for ip in network.iter() {
        ips.push(ip);
        if ips.len() > MAX_POOL_SIZE {
            return Err(format!(
                "待测 IP 总数超过 {} 上限（触发于 [{}]）。全量测速不做抽样，请拆成更小的网段，或改用单 IP 列表",
                MAX_POOL_SIZE, cidr
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_ip() {
        let ips = load_ip_ranges("", "1.1.1.1").unwrap();
        assert_eq!(ips.len(), 1);
        assert_eq!(ips[0].to_string(), "1.1.1.1");
    }

    #[test]
    fn ipv4_24_expands_all() {
        assert_eq!(load_ip_ranges("", "1.1.1.0/24").unwrap().len(), 256);
    }

    #[test]
    fn ipv4_23_expands_all() {
        assert_eq!(load_ip_ranges("", "1.1.0.0/23").unwrap().len(), 512);
    }

    #[test]
    fn duplicates_are_removed() {
        let ips = load_ip_ranges("", "1.1.1.1\n1.1.1.1\n1.1.1.2").unwrap();
        assert_eq!(ips.len(), 2);
    }

    #[test]
    fn ipv6_small_range_expands() {
        let ips = load_ip_ranges("", "2001:db8::/126").unwrap();
        assert_eq!(ips.len(), 4);
        for ip in ips {
            assert!(ip.to_string().starts_with("2001:db8:"));
        }
    }

    #[test]
    fn oversized_range_is_rejected() {
        let err = load_ip_ranges("", "10.0.0.0/8").unwrap_err();
        assert!(err.contains("上限"));
    }
}
