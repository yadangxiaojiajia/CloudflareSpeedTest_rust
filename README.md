<<<<<<< HEAD
# project-c · CloudflareSpeedTest Web 控制台

Rust 实现的 Cloudflare CDN IP 延迟 / 下载速度测速工具，自带浏览器图形界面，单文件交付。

```
浏览器 (http://127.0.0.1:8080)
        │ HTTP JSON + SSE
        ▼
   Rust Web Server (axum + tokio)
        │ 直接调用（同进程）
        ▼
   Rust SpeedTest Core
```

---

## 1. 开发目的

Cloudflare 用 anycast 广播同一批 IP 段：同一个域名，从不同地区、不同运营商连到不同 IP，延迟和速度可以差出十倍。所谓「优选 IP」就是先批量测速、再挑出对自己网络最好的那几个，供代理、订阅、回源等场景使用。

现有工具的两个痛点，是本项目立项的原因：

| 痛点 | 表现 | 本项目的回答 |
| --- | --- | --- |
| 命令行门槛高 | 原版 CloudflareSpeedTest 系列只有终端界面，参数靠背，非程序员用户难以上手 | 自带 Web 控制台：参数全部表单化、带默认值提示，进度 / 日志 / 结果表格实时可见 |
| 测完之后的路是断的 | 测出好 IP 后还要手动复制、手动粘贴到 Worker / 仓库 / 订阅里，步骤散落在多个工具中 | 界面内闭环：一键复制成 `IP:端口#地区-速度` 节点格式、一键上报 Cloudflare Worker、一键上传 GitHub 仓库 |

一句话概括开发目的：**把「测速 → 筛选 → 使用 / 上报」整条链路装进一个双击即用的单文件程序里，让不懂命令行的人也能完成优选 IP 的全流程。**

---

## 2. 开发逻辑

五条原则，贯穿所有代码决定：

1. **内核算法不创新，只对齐。** 测速内核按参考 Rust 实现逐项移植：超时、排序键、EWMA 平滑、抽样份数、并发度全部与上游一致，只新增了一个进度回调 trait（`Hooks`）供界面推送。优选工具的价值在于结果可信，算法自创只会让结果不可比。
2. **界面不做任何测速判断。** 浏览器只是「遥控器」：收参数、发状态、画表格。所有筛选 / 排序 / 达标规则都在内核里，保证「界面上看到的」永远等于「内核跑出来的」。
3. **参数单一真源。** `SpeedConfig` 一个结构体同时派生命令行参数（clap）与序列化（serde）：GUI 表单、CLI 参数、`config.json` 三者共用同一份定义。将来新增参数只需加一个字段，三端同时生效，不存在「界面能改但内核不认」。
4. **不加参考项目没有的「额外功能」。** 没有评分系统、没有推荐 IP、没有账号 / 数据库 / 云同步。唯一的扩展是结果出口（复制格式、Worker / GitHub 上报），因为那是测速的真正目的，而不是新功能。
5. **随时可以回到命令行。** 界面左下角实时显示当前参数等价的命令行字符串；`project-c test` 子命令与界面参数完全同构。GUI 与 CLI 互为备份，谁都不锁死用户。

---

## 3. 开发方案

### 3.1 总体架构

前后端不分进程、不引入 Node / Nginx / 数据库：前端三件套（原生 HTML/CSS/JS，无构建步骤）在编译期用 `include_str!` 内嵌进二进制，运行时由 axum 直接吐出。推送用 SSE 而非 WebSocket——事件是单向的（服务器 → 浏览器），SSE 三十行代码实现 + 20 秒心跳，比维护 ws 依赖更轻。

### 3.2 分层与边界

| 层 | 职责 | 明确不做的事 |
| --- | --- | --- |
| `api` | 收参数、发状态、静态资源 | 不做测速判断 |
| `task` | 串行调度、取消、事件广播、结果行 | 不碰测速算法 |
| `config` | 参数模型 + 持久化（GUI/CLI/文件同源） | 不做业务规则 |
| `core` | 测速算法（ip / tcping / httping / download / pipeline） | 不感知 Web 存在 |
| `upload` | Worker / GitHub 上报、地区码中文名表 | 不碰测速与调度 |

任务模型：同一时刻只允许一个测速任务；取消用 `AtomicBool`，内核每完成一个 IP 检查一次，已测得的结果不清空。下载测速阶段每测完一个 IP 推一条 `result` 事件，表格逐步填充，而不是等全部结束才刷新。

### 3.3 上报方案

- **Cloudflare Worker**：对接 cfnew 的 preferred-ips 接口 `{scheme}://{domain}/{uuid}/api/preferred-ips`。勾选「上报前清空」时先 `DELETE {"all":true}`，再 `POST` 一个 `[{ip, port, name}]` 数组，`name` 为「地区中文名-速度MB/s」。
- **GitHub**：走 contents API。先 `GET` 拿文件 sha（文件已存在时必须带 sha 才能覆盖），再 `PUT` base64 内容；每行格式 `IP:端口#地区-速度`。GitHub 强制要求 User-Agent 头，缺失直接 403，已内置。
- **凭据安全**：上报配置存数据目录 `report.json`；GitHub Token 只落盘，查询接口只回 `has_github_token` 布尔值，永不下发给浏览器；Token 输入框留空 = 沿用已保存的。

### 3.4 构建方案

- TLS 后端做成 Cargo feature：默认 `native-tls`（不需要 C 编译器），`--no-default-features --features rustls` 留给偏好纯 Rust 的环境。
- release 配置 `lto = true`、`codegen-units = 1`、`panic = "abort"`、`strip = true`，追求小体积与低资源占用。
- 依赖刻意压缩到 11 个直接依赖（axum / tokio / serde / serde_json / clap / reqwest / futures / csv / ipnetwork / regex / rand + base64）。

### 3.5 目录结构

```
project-c/
├── Cargo.toml              # 依赖与构建配置
├── README.md
├── ip.txt                  # 默认 IPv4 段（Cloudflare 官方）
├── ipv6.txt                # 默认 IPv6 段
├── frontend/               # 前端（原生 HTML/CSS/JS，无构建步骤）
│   ├── index.html
│   ├── app.js
│   └── style.css
└── src/                    # 后端
    ├── main.rs             # CLI 入口：web / test 两个子命令
    ├── web/                # 前端资源服务（include_str! 内嵌）
    ├── api/                # HTTP 路由与处理函数
    ├── task/               # 测速任务调度、事件广播、结果行
    ├── config/             # 参数模型（GUI / CLI / 配置文件同源）+ 持久化
    ├── upload/             # Worker / GitHub 上报 + 地区码中文名表
    ├── core/               # 测速内核
    │   ├── constants.rs    #   共享常量
    │   ├── ip.rs           #   IP 段加载与 CIDR 展开
    │   ├── tcping.rs       #   TCPing 延迟测速
    │   ├── httping.rs      #   HTTPing 延迟测速 + CDN 地区码识别
    │   ├── download.rs     #   下载测速（EWMA）
    │   ├── data.rs         #   数据结构 / 排序过滤 / CSV 导出
    │   └── pipeline.rs     #   流程编排：加载 → 延迟 → 筛选 → 下载
    └── utils/              # 运行目录解析、时间工具
```

---

## 4. 已经实现的功能

### 4.1 测速内核

- TCPing / HTTPing 双模式延迟测速，含丢包率统计；HTTPing 可从响应头识别 CDN 地区码（机场码）
- CIDR 段展开 + 随机抽样（`--allip` 可穷举网段内每个 IP）；支持直接指定 IP 段或从文件载入
- EWMA 平滑的下载测速，可设数量 / 时长 / 速度下限
- 多级筛选（延迟上下限、丢包上限、速度下限）与排序
- 结果导出 CSV

### 4.2 Web 控制台

- 全部内核参数表单化，每个字段标注对应命令行参数与默认值；参数自动持久化、下次打开回填
- 进度条（有确切进度显示百分比，否则滚动动画）+ 分阶段实时日志（load / ping / filter / download / done）
- 结果表格：点表头排序、按 IP 或地区码筛选、开始 / 停止（停止后已测得结果不丢）
- 界面左下角实时显示等价命令行参数，可复制给定时任务
- 「选择文件」读入本地 IP 段 txt 到输入框，可再编辑；「清空」一键还原

### 4.3 结果使用与上报

- 每行「复制」与工具栏「复制节点」：格式 `IP:端口#地区中文名-速度MB/s`（如 `45.192.206.31:443#东京成田-37.79MB/s`），可直接喂给支持节点备注的工具
- 「导出 CSV」下载结果文件
- 上报配置弹窗：Worker 域名、UUID / 自定义路径、GitHub 仓库、Token、仓库内文件路径、上报数量、上报前清空 Worker 已有 IP
- 「上报 Worker」：一键推送优选 IP 到 cfnew preferred-ips 接口
- 「GitHub」：一键写入 / 覆盖 GitHub 仓库内的 IP 列表文件（自动处理 sha）
- 命令行 `test` 子命令：参数与界面完全一致，输出表格 + CSV

### 4.4 工程特性

- 跨平台单二进制（Windows / Linux / macOS），前端内嵌、无运行时依赖
- 数据目录优先级：环境变量 `CFST_DATA_DIR` > 当前工作目录 > 可执行文件目录
- 配置与凭据落盘：`config.json`（测速参数）、`report.json`（上报配置，Token 不下发）

---

## 5. Rust 用于本项目的优点

| 优点 | 在本项目里的具体体现 |
| --- | --- |
| 高并发、低开销 | 延迟测速要同时握几百个 TCP 连接并做毫秒级计时；tokio 异步运行时以极小内存承载上千并发任务，远超线程池模型的资源效率 |
| 计时不被 GC 扭曲 | 延迟 / 丢包是测量值，运行时一旦停顿采样就失真；Rust 无垃圾回收，测出来的毫秒数只反映网络，不反映解释器 |
| 内存安全、长跑稳定 | 一次优选可能跑几十分钟、累积上万条结果；所有权与借用检查在编译期消除数据竞争与泄漏，任务长跑不崩不漏 |
| 单二进制交付 | 静态编译 + `include_str!` 内嵌前端 = 一个文件双击即用，目标机器不装运行时、不装 Node、不装依赖；对非程序员用户和 NAS / 服务器场景是最友好的分发形态 |
| 编译期保证三端一致 | `SpeedConfig` 同时派生 clap 与 serde，GUI / CLI / 配置文件同源；改一个字段编译器全程护航，杜绝「界面能改但内核不认」这类漂移 |
| 移植零损耗 | 参考内核本身就是 Rust 实现，算法可原样移植而非跨语言重写；tokio / reqwest / axum / csv 生态成熟，无需造轮子 |
| 体积与占用可控 | `lto + codegen-units=1 + strip` 的 release 配置产物小、占用低，适合长期挂在后台或软路由上跑 |

---

## 附录 A · 快速开始

前置：Rust 1.70+（推荐 1.80+）。

```bash
# 通用（默认特性，不需要 C 编译器）
cargo build --release

# 纯 Rust 的 rustls 后端（需要本机有 gcc / MSVC）
cargo build --release --no-default-features --features rustls
```

Linux 缺 OpenSSL 时：`apt install -y pkg-config libssl-dev`，或改用 rustls 后端。产物：Windows `target\release\project-c.exe`，Linux / macOS `target/release/project-c`（必要时 `chmod +x`）。

```bash
# Web 控制台（默认命令）：监听 127.0.0.1:8080 并自动打开浏览器
project-c

# 局域网 / 服务器
project-c web --listen 0.0.0.0:8080 --no-open

# 命令行测速（参数与界面一致）
project-c test -n 500 -t 6 --tl 300
project-c test --httping --cfcolo "LAX,SJC" --dd
project-c test --ip "104.16.0.0/13,1.1.1.0/24" -o result.csv
```

> 绑定 `0.0.0.0` 会让同网段所有人都能操作测速，请自行确认网络环境。

## 附录 B · 参数说明

界面上的每一项参数都对应内核的一个命令行参数，两者共用同一份 Rust 结构体。

| 界面字段 | 命令行参数 | 说明 | 默认 |
| --- | --- | --- | --- |
| 选择文件（读入 IP 段输入框） | `-f`（命令行指定运行机上的文件） | 每行一条 CIDR 或单个 IP；界面填了 IP 段则优先于文件 | `ip.txt` |
| 直接指定 IP 段 | `--ip` | 逗号 / 换行分隔，填了就优先于文件 | 空 |
| 测速全部 IP | `--allip` | 关掉抽样，穷举网段内每个 IP（IPv4 大段请慎用） | 关 |
| 延迟并发数 | `-n` | 同时测多少个 IP 的延迟，上限 1000 | 200 |
| 每 IP 次数 | `-t` | 每个 IP 连几次取平均，顺带算丢包率 | 4 |
| 测速端口 | `--tp` | TCP / HTTP 连接端口 | 443 |
| 显示数量 | `-p` | 界面 / 终端打印多少条，0 = 全部 | 10 |
| 测速地址 | `--url` | HTTPing 请求地址 + 下载测速地址 | `https://cf.xiu2.xyz/url` |
| 延迟测速模式 | `--httping` | 默认 TCPing（TCP 握手）；开启后走 HTTP 请求 | TCPing |
| 有效状态码 | `--httping-code` | 仅 HTTPing 生效，0 = 接受 100~599 | 0 |
| 地区码过滤 | `--cfcolo` | 仅 HTTPing 生效，如 `HKG,SJC`，留空不过滤 | 空 |
| 延迟上限 | `--tl` | 平均延迟超过就淘汰（ms），9999 = 不限 | 9999 |
| 延迟下限 | `--tll` | 平均延迟低于就淘汰（ms） | 0 |
| 丢包上限 | `--tlr` | 0.00~1.00 | 1.0 |
| 下载测速 | `--dd` | 选「跳过」即禁用下载测速 | 开 |
| 下载数量 | `--dn` | 想要多少个**达标** IP | 10 |
| 下载时间 | `--dt` | 单个 IP 最长测多少秒 | 10 |
| 速度下限 | `--sl` | 低于该速度的 IP 淘汰（MB/s），0 = 不筛选 | 0 |
| 输出文件 | `-o` | 结果 CSV 路径 | `result.csv` |
| 调试输出 | `--debug` | 输出更多内核调试信息 | 关 |

## 附录 C · HTTP 接口

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/` `/app.js` `/style.css` | 前端页面（编译期内嵌） |
| GET / POST | `/api/config` | 读取 / 保存测速配置 |
| GET | `/api/status` | `{running, count}` |
| POST | `/api/start` | 提交 `SpeedConfig` JSON 启动测速 |
| POST | `/api/cancel` | 停止当前任务 |
| GET | `/api/events` | SSE 事件流（20s 心跳） |
| GET | `/api/results` | 当前结果列表 |
| GET | `/api/logs` | 当前日志缓冲 |
| GET | `/api/download?file=result.csv` | 下载结果 CSV |
| GET | `/api/system` | 版本、数据目录、默认参数 |
| GET / POST | `/api/report-config` | 读取 / 保存上报配置（Token 只回 `has_github_token`） |
| POST | `/api/upload/worker` | 上报优选 IP 到 Worker |
| POST | `/api/upload/github` | 上传优选 IP 到 GitHub 仓库 |

SSE 事件体：

```json
{"type":"progress|log|result|done|error","stage":"ping","message":"可用 128",
 "current":300,"total":1000,"result":null,"results":null,"finished":false,"at":1730000000000}
```

## 附录 D · 数据文件

| 文件 | 说明 |
| --- | --- |
| `config.json` | 界面最后一次的测速参数，自动回填用 |
| `result.csv` | 测速结果（列名：`IP 地址,已发送,已接收,丢包率,平均延迟,下载速度(MB/s),地区码`） |
| `report.json` | 上报配置（Worker 域名 / UUID / GitHub 仓库与 Token 等）；**含凭据，不要上传到公开仓库** |

## 附录 E · 常见问题

**Q：结果一直是 0 个？**
多半是 `-n` 太大导致握手超时堆积，或 `--sl` 设太高导致没有 IP 达标。先把 `--sl` 改成 0、`-p` 改成 0 试一次；确认网络能直连 443 端口。

**Q：下载测速没有速度数据？**
`--sl` 默认 0.0 表示「不做速度筛选」，此时内核会跳过下载测速直接沿用延迟结果（与上游逻辑一致）。要真正测下载，把速度下限设成大于 0 的值，例如 `1`。

**Q：HTTPing / TCPing 拿不到地区码？**
地区码来自 CDN 响应头（`cf-ray` / `x-amz-cf-pop` 等），TCPing 模式本身拿不到；HTTPing 下若仍为空，切换测速地址或改用 Cloudflare 官方地址试试。复制 / 上报时空地区码会显示为「未知地区」。

**Q：GitHub 上传报 403？**
GitHub REST API 强制要求 User-Agent 头，本程序已内置；仍报 403 请检查 Token 权限（需要 repo scope）与仓库名是否为 `owner/repo` 格式。

**Q：编译报 ring / gcc 相关错误？**
说明用到了 rustls 后端。默认已经改成系统 TLS，重新 `cargo build --release` 即可；确实需要 rustls 请先装 C 编译器。

## 附录 F · 协议

测速内核设计与参数体系取自 [aspnmy/CloudflareSpeedTest](https://github.com/aspnmy/CloudflareSpeedTest)（GPL-3.0），本项目沿用 GPL-3.0。Web UI 交互参考 [byJoey/yx-tools](https://github.com/byJoey/yx-tools)（MIT）。
=======
# CloudflareSpeedTest_rust
>>>>>>> faf08a7816609c5e1c2f19f827edc095f911a38f
