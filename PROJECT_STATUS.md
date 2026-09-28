# PROJECT_STATUS.md

> 记录时间：2026-09-28 19:40（GMT+8）
> 项目：project-c —— Rust CloudflareSpeedTest 内核 + 浏览器 Web GUI
> 工作区：`C:\Users\24200\Desktop\qoder\CFST`（已从 WorkBuddy 搬迁）
> 状态：**release 构建已打通；新增「上报 Worker / 上报 GitHub」功能并完成界面验证；真实上报待用户凭自己凭据实测**

---

## 1. 当前完成情况

### 1.1 已完成的模块

| 模块 | 状态 | 说明 |
| --- | --- | --- |
| `core`（测速内核） | ✅ 完成 | 6 个文件，算法与上游 Rust 核心一致：TCPing / HTTPing、EWMA 下载测速、CIDR 抽样、多级排序过滤、CSV 导出、流程编排 |
| `config`（参数模型） | ✅ 完成 | `SpeedConfig` 一个结构体同时给 clap、serde、Web 表单用；含 CLI 参数反向生成与 JSON 持久化 |
| `task`（任务调度） | ✅ 完成 | 单任务串行 + 取消 + broadcast 事件；事件结构与参考 Web UI 对齐（`type/stage/message/current/total/result/results/finished/at`） |
| `api`（HTTP 层） | ✅ 完成 | 11 个路由，含 SSE 事件流；只做收发，不含测速逻辑 |
| `web`（资源服务） | ✅ 完成 | `include_str!` 编译时内嵌前端，零外部文件依赖 |
| `frontend`（前端） | ✅ 完成 | 原生 HTML/CSS/JS，无构建步骤、无框架 |
| `utils` | ✅ 完成 | 数据目录解析、时间工具 |
| `upload`（上报） | ✅ 完成 | 2026-09-28 新增：Worker（cfnew preferred-ips，可上报前清空）与 GitHub（contents API 取 sha 覆盖）两个出口；`report.json` 持久化，Token 只落盘不下发；含地区码中文名表 |
| README / ip.txt / ipv6.txt | ✅ 完成 | 含编译运行说明、参数对照表、接口文档、常见问题 |

### 1.2 已创建 / 修改的文件清单

| 文件 | 大小 | 作用 |
| --- | --- | --- |
| `Cargo.toml` | 1.2 KB | 依赖与 `[features]`（native-tls / rustls 可选） |
| `Cargo.lock` | 50.5 KB | 已锁定依赖版本（tokio 降至 1.47.1） |
| `README.md` | 9.7 KB | 编译 / 运行 / 浏览器访问 / 参数说明 / 接口文档 |
| `ip.txt` | 420 B | 默认 Cloudflare IPv4 段（15 条 CIDR） |
| `ipv6.txt` | 270 B | 默认 Cloudflare IPv6 段（7 条 CIDR） |
| `src/main.rs` | 4.3 KB | CLI 入口：`web`（默认）与 `test` 两个子命令；自动打开浏览器 |
| `src/web/mod.rs` | 415 B | 三个常量持有 `include_str!` 的前端资源 |
| `src/api/mod.rs` | 5.3 KB | 路由表 + handler + SSE 事件流 |
| `src/task/mod.rs` | 8.4 KB | `Runner` / `Row` / `Event` / `Bridge`（把内核回调转成事件） |
| `src/config/mod.rs` | 7.2 KB | `SpeedConfig`（clap + serde 同源）、`to_cli_args()`、读写 `config.json` |
| `src/core/mod.rs` | 1.2 KB | `Hooks` trait（tick / log / item / cancelled）+ `NoopHooks` |
| `src/core/constants.rs` | 1.5 KB | 共享常量（并发、超时、默认 URL、DOWNLOAD_CONCURRENCY=10、SPEED_SAMPLES=100） |
| `src/core/ip.rs` | 4.6 KB | IP 段加载、CIDR 展开与抽样（/24 随机尾段、大段每 /24 抽 1、IPv6 随机主机位） |
| `src/core/tcping.rs` | 3.2 KB | TCP 握手测延迟，`buffer_unordered(routines)`，1s 超时 |
| `src/core/httping.rs` | 8.4 KB | HTTP HEAD 测延迟 + CDN 地区码识别（cf-ray / x-amz-cf-pop / server / x-served-by / x-id-fe）+ colo 过滤 |
| `src/core/download.rs` | 7.4 KB | 每 IP 独占 Client + `resolve()`，EWMA(alpha=1/3) 采样，`--sl` 为 0 时跳过下载 |
| `src/core/data.rs` | 3.8 KB | `PingData` / `CloudflareIpData` / `IpDataSet`（丢包→延迟排序）/ `DownloadSpeedSet`（速度降序）/ `export_csv` |
| `src/core/pipeline.rs` | 4.0 KB | 编排：加载 → 延迟测速 → 排序筛选 → 下载测速 |
| `src/utils/mod.rs` | 1.1 KB | `data_dir()`（CFST_DATA_DIR > cwd > exe 目录）、`resolve()`、`now_ms()` |
| `frontend/index.html` | 7.8 KB | 页面结构：顶栏 + 左参数面板 + 右结果区 + 日志区 + toast |
| `frontend/style.css` | 8.6 KB | 浅色主题样式（参考 yx-tools 布局） |
| `frontend/app.js` | 12.5 KB | 参数采集/回填、SSE 订阅、表格渲染排序筛选、复制导出、等价命令行预览 |

### 1.3 已验证的逻辑行为（编译通过，未实跑）

- `cargo check` 通过（0 error，0 warning）
- 所有项目 B 的 21 个 CLI 参数已 1:1 映射到 GUI 并保持默认一致
- 界面参数 → `SpeedConfig` → 内核，三者同源

---

## 2. 当前开发阶段

**阶段：上报功能已交付验证，剩真实凭据实测** —— 构建卡点已于 2026-09-28 解除。

已完成的顺序：
1. ✅ 目录结构与 Cargo.toml
2. ✅ core 测速内核（对齐项目 B）
3. ✅ config 参数模型 + CLI 映射
4. ✅ task 任务管理与取消
5. ✅ SSE 实时事件与日志
6. ✅ 前端三件套（布局 / 参数 / 结果 / 日志）
7. ✅ release 构建（需把 MSYS2 binutils 加入 PATH，见 6.2）
8. ✅ 上报 Worker / 上报 GitHub（后端 `src/upload` + 3 个 API + 前端弹窗/按钮，浏览器已验证交互）

下一步应该做什么：
- 用户用自己的 Worker 域名/UUID、GitHub 仓库/Token 实跑一次上报（本机无凭据，且到 Cloudflare 测速网络不通，CLI 实跑结果数为 0 属网络限制）
- 网络通畅的机器上跑一次完整测速验证内核端到端

---

## 3. 未完成任务列表

| # | 任务 | 优先级 | 阻塞原因 |
| --- | --- | --- | --- |
| 1 | 用户凭据实跑上报 Worker / GitHub | P0 | 需用户自己的域名/UUID/仓库/Token |
| 2 | 网络通畅环境跑一次完整测速（CLI + Web） | P1 | 本机到 Cloudflare 段不通，测速结果 0 条 |
| 3 | 可选：恢复 tokio 到最新版 Cargo.lock（当前为修依赖链降到 1.47.1） | P2 | `cargo update -p tokio` 后重新验证 |
| 4 | 可选：Linux / macOS 上验证（本机无法） | P2 | 无环境 |
| 5 | 可选：窄屏（≤900px）单列布局下 sticky 面板盖住结果区的既有体验问题 | P2 | 与上报功能无关，宽屏正常 |

---

## 4. 当前代码状态

### 4.1 是否可以编译

| 检查项 | 结果 |
| --- | --- |
| `cargo check`（类型 / 语法 / 借用检查） | ✅ **通过**，0 error 0 warning |
| `cargo build --release`（含链接） | ✅ **通过**（2026-09-28，Rust 1.98.1；需先把 MSYS2 binutils 加入 PATH，见 6.2） |

### 4.2 release 构建的必要环境（当前有效配方）

```bash
export PATH="/c/Users/24200/.workbuddy/binutils/mingw64/bin:$PATH"
cargo build --release
```

- rustup 自带 `self-contained\dlltool.exe` 仍不可用（调用 `as.exe` 时 CreateProcess 失败）
- `C:\Users\24200\.workbuddy\binutils\mingw64\bin` 的 MSYS2 binutils 2.44（含 as.exe）现在可正常工作，旧文档记录的「缺依赖 DLL」问题已不复现

### 4.3 历史错误链（均已修复或定位）

| 阶段 | 错误 | 处理 |
| --- | --- | --- |
| 初次 check | `ring v0.17` 需要 C 编译器（本机无 gcc / MSVC） | ✅ 改为可选特性：默认 `native-tls`（Windows 走 schannel，零 C 依赖），`--features rustls` 才用 rustls |
| check | `Arc` / `Query` / `Response` / `Html` 导入缺失 | ✅ 修正 imports，`axum` 补 `"query"` feature |
| check | SSE 流 item 类型不匹配（期望 `sse::Event`，实际 `String`） | ✅ 改为 `Ok(sse::Event::default().data(json))` |
| check | `.body(...)` 类型推断歧义 | ✅ 抽出 `index/js/css` 三个具名 handler，用 `Body::from` |
| check | `FnOnce is not general enough`（download.rs 借用跨 await） | ✅ 改为按值迭代 `ip_data.iter().cloned()` |
| release | `error calling dlltool 'dlltool.exe': program not found` | ✅ 找到 rustup 自带 `self-contained\dlltool.exe`，但该 dlltool 缺 `as.exe`（CreateProcess 失败） |
| release | dlltool 生成 ntdll 导入库失败（仅剩这一个问题） | ⏸️ 见 5：已下载 MSYS2 binutils，仍缺依赖 DLL |

### 4.4 已知环境限制

- 本机 Bash（Git Bash shim）缺 coreutils（`ls/head/tail/grep/cd` 报 not found）；PowerShell 工具无回显
  → **可用方式**：用托管 Python 绝对路径 `"C:/Users/24200/.workbuddy/binaries/python/versions/3.13.12/python.exe"` 执行脚本
- 本机无 gcc / clang / MSVC（`cl.exe`），Rust 目标为 `x86_64-pc-windows-gnu`
- curl 访问外部 HTTPS 报 `exit 35`（SSL）；Python `urllib` 可下载但速度慢（约 25 KB/s）

---

## 5. 历史卡点（已解决）

**现象**：Windows-gnu 目标下，`windows-sys` 的 `raw-dylib` 需要外部 `dlltool` 生成导入库；本机 PATH 没有可用 dlltool。

**解决（2026-09-28）**：Rust 升级到 1.98.1 后，`C:\Users\24200\.workbuddy\binutils\mingw64\bin` 里早前解压的 MSYS2 binutils 2.44 已可正常运行（含 `as.exe`，缺 DLL 问题消失）。构建前把它加入 PATH 即可，见 4.2。rustup 自带 `self-contained\dlltool.exe` 依旧不可用（缺 `as.exe`）。历史上尝试过的路线（降级 tokio/windows-sys、llvm-tools、补 DLL 等）全部记录在 git 之外的旧版本文档，已无保留必要。

---

## 6. 下一次继续开发时的操作步骤

### 6.1 构建与运行

```bash
cd /c/Users/24200/Desktop/qoder/CFST
export PATH="/c/Users/24200/.workbuddy/binutils/mingw64/bin:$PATH"
cargo build --release
./target/release/project-c.exe web --listen 127.0.0.1:8080 --no-open
```

### 6.2 上报功能验证（需用户凭据）

- 界面顶栏齿轮 = 上报配置；工具栏「上报 Worker」「GitHub」两个按钮
- 配置存 `<数据目录>/report.json`；GitHub Token 只落盘、GET 接口只回 `has_github_token`
- 缺配置点按钮会自动弹配置窗；无测速结果时报「没有可上报的结果」

---

## 7. 技术方案与重要设计决定

### 7.1 架构

```
浏览器 ──HTTP JSON + SSE──▶ Rust Web Server (axum 0.7 + tokio)
                                    │ 同进程直接调用
                                    ▼
                              Rust SpeedTest Core
```

- 前后端不分进程、不引入 Node/构建工具；前端三件套在编译期用 `include_str!` 内嵌 → 单二进制交付
- SSE（不是 WebSocket）：单向事件推送足够，30 行代码实现 + 20 秒心跳，比 ws 依赖更轻
- 事件字段名与参考项目 A 完全一致，`type` 分 `progress / log / result / done / error`

### 7.2 分层与边界（严格遵守"UI 只调用核心"）

| 层 | 职责 | 明确不做的事 |
| --- | --- | --- |
| `api` | 收参数、发状态 | 不做测速判断 |
| `task` | 串行调度、取消、事件广播 | 不碰测速算法 |
| `config` | 参数单一真源 | 不做业务规则 |
| `core` | 测速算法 | 不感知 Web |

### 7.3 关键设计决定（含原因）

1. **内核 = 按项目 B 的 Rust 实现重新实现**：阶段要求"功能以 B 为准、不得改算法"，所以 tcping/httping/download/data 的 algorithm 细节（超时、排序键、EWMA alpha、采样份数、并发度）逐项对齐，**只新增了一个回调 trait**（`Hooks`）用于 Web 进度推送，不改动任何测速逻辑。
2. **`SpeedConfig` 同时派生 `clap::Args` 与 `serde`**：GUI 表单、命令行参数、config.json 三者共用同一份定义；将来项目 B 新增参数，**只需在这个结构体加一个字段**（带 `#[arg]` 与 `#[serde]` 属性），三端同时生效。
3. **不加任何"额外功能"**：没有评分系统、没有推荐 IP、没有账号/数据库/云同步，这些项目 B 没有，界面也不提供。
4. **IPv4 / IPv6 在界面上做成 IP 版本开关，本质映射到 `-f` 切换 `ip.txt` / `ipv6.txt`**：项目 B 没有 `-ipv6` 参数（IP 版本由输入 IP 段决定），这样既满足界面要求，又不给内核增加新能力。
5. **TLS 后端做成 Cargo feature（默认 native-tls）**：本机无 C 编译器，rustls 依赖的 `ring` 编不过；`--no-default-features --features rustls` 保留给有工具链/偏好纯 Rust 的用户。
6. **依赖数量刻意压缩**：axum / tokio / serde / serde_json / clap + 内核必需的 futures / reqwest / csv / ipnetwork / regex / rand，共 11 个直接依赖；弃用了上游的 `indicatif`、`colored`（CLI 输出改为纯文本）。
7. **release 配置**：`lto = true`、`codegen-units = 1`、`panic = "abort"`、`strip = true`，追求小体积与低资源占用。
8. **任务取消**：`AtomicBool` + 内核每完成一个 IP 检查一次，取消防护点明确；已测得的结果不清空。
9. **结果交付**：下载测速阶段每测完一个 IP 就推一条 `result` 事件，界面逐步填充表格（参考项目 A 的交互）。
10. **上报功能（2026-09-28，按用户要求参照改 yx-tools 移植）**：Worker 走 cfnew 的 `{domain}/{uuid}/api/preferred-ips`（GET 计数 / DELETE 清空 / POST 批量 `{ip,port,name}`）；GitHub 走 contents API（先 GET 取 sha 再 PUT base64 覆盖）。上报逻辑全在 Rust 后端（`src/upload`），前端只发请求；配置独立于测速参数存 `report.json`，Token 不下发给前端。节点备注格式 `地区名-速度MB/s`，地区名用内置机场码中文名表。

### 7.4 需要提醒的后续事项

- `Cargo.lock` 里的 tokio 已被降到 **1.47.1**（当初为砍掉 windows-sys 0.61 的尝试），构建环境修好后建议 `cargo update -p tokio` 回到最新并重新验证一次。
- 已额外安装的东西（不在项目里，属环境）：rustup `llvm-tools` 组件、工具链 `1.75.0-x86_64-pc-windows-gnu`、`C:\Users\24200\.workbuddy\binutils\`（MSYS2 binutils 解压目录）、Python venv `envs/default`（装了 zstandard）。
- 参考源码下载在 `.workbuddy/refs/{A,B}`，仅供分析，**不参与构建**。
