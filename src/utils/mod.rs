//! 运行目录解析：环境变量 CFST_DATA_DIR > 当前工作目录 > 可执行文件目录

use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CFST_DATA_DIR") {
        let p = PathBuf::from(dir);
        if std::fs::create_dir_all(&p).is_ok() {
            return p;
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if std::fs::create_dir_all(cwd.join(".cfst-write-test")).is_ok() {
            let _ = std::fs::remove_dir(cwd.join(".cfst-write-test"));
            return cwd;
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 把相对于数据目录的路径拼成绝对路径
pub fn resolve(name: &str) -> PathBuf {
    let p = PathBuf::from(name);
    if p.is_absolute() {
        p
    } else {
        data_dir().join(p)
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
