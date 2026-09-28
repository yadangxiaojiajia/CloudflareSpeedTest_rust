//! IP 段加载与 CIDR 展开 —— 抽样规则与上游 Rust 核心一致

use crate::core::constants::DEFAULT_INPUT_FILE;
use ipnetwork::IpNetwork;
use rand::Rng;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::path::Path;

/// 加载待测试的 IP 列表。
/// `ip_text` 非空时优先解析它（逗号分隔 CIDR），否则读取 `ip_file`。
pub fn load_ip_ranges(ip_file: &str, ip_text: &str, test_all: bool) -> Result<Vec<IpAddr>, String> {
    let mut ips = Vec::new();

    if !ip_text.is_empty() {
        for part in ip_text.split([',', '\n', ';']) {
            let part = part.trim();
            if !part.is_empty() {
                expand_ip_range(part, test_all, &mut ips)?;
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
            let line = line.trim().to_string();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            expand_ip_range(&line, test_all, &mut ips)?;
        }
    }

    Ok(ips)
}

/// 解析一条 CIDR 或单个 IP 并展开成具体 IP
fn expand_ip_range(cidr: &str, test_all: bool, ips: &mut Vec<IpAddr>) -> Result<(), String> {
    let network: IpNetwork = cidr
        .parse()
        .map_err(|e| format!("解析 CIDR [{}] 失败: {}", cidr, e))?;
    let mut rng = rand::thread_rng();

    match network {
        IpNetwork::V4(net) => {
            if net.prefix() == 32 {
                ips.push(IpAddr::V4(net.network()));
            } else if test_all {
                for ip in net.iter() {
                    ips.push(IpAddr::V4(ip));
                }
            } else if net.prefix() >= 24 {
                // 单个 /24 或更小 —— 随机最后一段
                let mut octets = net.network().octets();
                octets[3] = rng.gen_range(1..=254u8);
                ips.push(IpAddr::V4(octets.into()));
            } else {
                // 逐个 /24 子网各抽一个随机 IP
                let base_ip = net.network();
                let step = 1u32 << (24 - net.prefix());
                let o = base_ip.octets();
                let base = (o[0] as u32) << 24 | (o[1] as u32) << 16 | (o[2] as u32) << 8 | (o[3] as u32);
                for i in 0..step {
                    let last_byte: u8 = rng.gen_range(1..=254u8);
                    let new_ip = base + i * 256 + last_byte as u32;
                    ips.push(IpAddr::V4(std::net::Ipv4Addr::from(new_ip)));
                }
            }
        }
        IpNetwork::V6(net) => {
            if net.prefix() == 128 {
                ips.push(IpAddr::V6(net.network()));
            } else {
                // IPv6：只随机主机位；可用空间不足时全量列出
                let host_bytes = ((128 - net.prefix()) / 8) as usize;
                let start_byte = (net.prefix() / 8) as usize;
                let num_ips = if host_bytes >= 4 {
                    256
                } else {
                    1usize << (host_bytes * 8)
                };
                for _ in 0..num_ips {
                    let mut octets = net.network().octets();
                    for j in 0..host_bytes.min(16 - start_byte) {
                        octets[start_byte + j] = rng.gen();
                    }
                    ips.push(IpAddr::V6(octets.into()));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_24_random_one() {
        let ips = load_ip_ranges("", "1.1.1.0/24", false).unwrap();
        assert_eq!(ips.len(), 1);
        assert!(ips[0].to_string().starts_with("1.1.1."));
    }

    #[test]
    fn ipv4_23_two() {
        assert_eq!(load_ip_ranges("", "1.1.0.0/23", false).unwrap().len(), 2);
    }

    #[test]
    fn ipv4_24_all() {
        assert_eq!(load_ip_ranges("", "1.1.1.0/24", true).unwrap().len(), 256);
    }

    #[test]
    fn ipv6_prefix_kept() {
        let ips = load_ip_ranges("", "2001:db8::/32", false).unwrap();
        assert!(!ips.is_empty());
        for ip in ips {
            assert!(ip.to_string().starts_with("2001:db8:"));
        }
    }
}
