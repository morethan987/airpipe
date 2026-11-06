# Airpipe 通信协议

**版本 v2（已实现校准）** · 本文档是 PC 端与 Android 端的共同契约，实现过程中如有变动必须同步更新此处并记录到文末变更日志。

角色固定：**手机 = TCP 客户端（主动出站）**，**PC = 监听端**。依据：HyperOS 静默丢弃 App 的一切入站连接（含 loopback），出站畅通；校园网大平面网对 client↔client 大流量限杀属环境问题，与本协议无关。

---

## 1. 常量

| 常量 | 值 | 说明 |
| :--- | :--- | :--- |
| `PROTO_VER` | 2 | HELLO 中携带，不匹配即 BYE 断开（v2：D 帧加 flags、窗口化传输，与 v1 不兼容） |
| `TCP_PORT` | 17580 | PC 监听端口，config 可改 |
| `DISCOVER_PORT` | 17580 | UDP 发现端口（与 TCP 同号，不同协议） |
| `CHUNK` | 131072 | FILE_DATA 单帧数据上限 128 KiB |
| `MAX_TEXT` | 4 MiB | CLIP_TEXT 载荷上限 |
| `MAX_IMAGE` | 24 MiB | CLIP_IMAGE 载荷上限（PC 侧 config 可调 `max_image_mb`） |
| `PROGRESS_EVERY` | 4 MiB | 接收方每收满此字节数回一帧 FILE_PROGRESS |
| `PING_EVERY` | 15 s | PC → 手机心跳周期 |
| `SILENCE_DEAD` | 45 s | 任一侧超过此时长未收到对方任何字节 → 判死链断开 |
| `BACKOFF_MIN / MAX` | 2 s / 60 s | 手机重连退避：2s 起指数翻倍封顶 60s；新任务 nudge 立即全速重拨 |
| `PAIR_WINDOW` | 60 s | 配对码有效窗口 |
| `PAIR_MAX_TRIES` | 5 | 单窗口内验证失败上限 |

## 2. 局域网发现（UDP 广播）

沿用探针验证过的模型，仅换名并携带证书指纹：

```
手机（退避唤醒且无已知 PC 地址时，最多 3 次，每次等 800ms）
  ── "AIRPIPE_DISCOVER <PROTO_VER>" → 255.255.255.255:DISCOVER_PORT

PC daemon（UDP 应答线程，常驻）
  ── "AIRPIPE_PC <PROTO_VER> <tcp_port> <fp16>" 单播回
```

- `fp16` = PC 身份证书指纹（见 §3）的前 16 个 hex 字符。手机用它区分「已配对过的 PC」（直接连）与「陌生 PC」（仅配对流程中会连）。
- 应答包源地址即 PC IP，手机随后 TCP 直连 `<源地址>:<tcp_port>`。
- 连接健康时不广播；连接失败置空缓存的 PC 地址，下轮退避重新发现 → 天然容忍 DHCP 变更。
- 兜底：手机配对页可手动填 PC IP（校园网/广播被禁环境用）。广播域即边界，跨 VLAN 不可达——这是刻意接受的限制。

## 3. 加密与身份

**TLS 1.3，双向证书认证，配对即指纹互存（TOFU + 固定）。** 模型与 KDE Connect 相同：

- 每台设备首次运行生成一张**长期自签证书**（ECDSA P-256，CN=`airpipe/<设备名>`，有效期 10 年），之后永不再换。
  - PC：`~/.local/state/airpipe/identity.pem`（key + cert）
  - Android：应用私有目录 `files/identity.p12`
- **指纹** = SHA-256(证书 DER) 的 hex（64 字符）。
- 会话验证规则（双端对称）：
  - 对端证书指纹 ∈ 本地已配对列表 → 通过；
  - 不在列表 → 仅当本端处于配对窗口（PC 侧 `ap pair` 生效中 / 手机侧用户发起了配对）才放行本次 TLS（机会式加密），并在 PAIR_ACCEPT 后落库；否则握手后立即断开并记日志。
- 未配对陌生设备既拿不到会话，也拿不到任何数据帧。
- 威胁模型：不可信局域网。配对完成后全部流量加密且双向认证；唯一暴露面是 60 秒配对窗口内的机会式 TLS（理论上可被同网段主动 MITM），用 6 位码挡住「不知码的陌生设备」。此残余风险与 KDE Connect 一致，接受。

## 4. 配对流程

```
PC                                       手机
ap pair ──► 生成/复用身份证书
            开 60s 窗口 + 随机 6 位码
            终端显示码
                                           用户在配对页输入码
                                           TCP+TLS（机会式，携带本机身份证书）
            ──► H HELLO{ver,name,caps} ──►    （会话建立先于配对交换，PC 侧 TLS 后立即发）
            ◄── Q PAIR_REQUEST{code, name} ─
            常数时间比对 code（≤5 次）
            通过：记录手机指纹+name ──► A PAIR_ACCEPT{name} ─►
                                           记录 PC 指纹
            双方持久化，此后永久免配对
```

- 码由 CSPRNG 生成 6 位十进制；验证失败计数达 5 次即关窗。
- 配对不改变连接：手机在配对等待期遇到 `H` 就当场消化（校验 ver、记下会话状态，不得当异常），收到 `A` 后补发自己的 HELLO，同一连接直接进入正常会话（PC 侧不再二次等待对端 HELLO）。
- 取消配对：PC `ap unpair <name>|--all`；手机配对页「取消配对」。单侧删除即可拒收（对端证书不再被信任），双侧删除才干净。

### 持久化格式（双端同构）

`paired.json`：

```json
{
  "devices": [
    {
      "name": "mi-13p",
      "fingerprint": "<64 hex>",
      "paired_at": "2026-11-04T12:00:00+08:00",
      "last_ip": "192.168.0.23",
      "last_seen": "2026-11-04T20:00:00+08:00"
    }
  ]
}
```

## 5. 帧格式（TLS 之内，单连接双向复用）

所有整数**大端**。帧 = `1B 类型 + 4B 载荷长度 + 载荷`。读取方发现长度 > `MAX_IMAGE + 64` 判协议错误断开。类型字节取可打印字符便于抓包调试。

| 类型 | 帧 | 载荷布局 | 方向 | 说明 |
| :--- | :--- | :--- | :--- | :--- |
| `H` | HELLO | `ver u16, nameLen u8, name, caps u32` | 双向 | TLS 后立即互发；`ver` 不符收 BYE。caps 位掩码：bit0 文本同步 / bit1 图片同步 / bit2 文件传输；未知位忽略 |
| `P` / `p` | PING / PONG | 空 | PC→A / A→PC | 15s 一发；双方 45s 无任何对端字节判死 |
| `T` | CLIP_TEXT | `mimeLen u8, mime, data` | 双向 | mime 如 `text/plain;charset=utf-8`；≤ MAX_TEXT |
| `I` | CLIP_IMAGE | `mimeLen u8, mime, data` | 双向 | png/jpeg/webp 等原始字节；≤ MAX_IMAGE |
| `B` | FILE_BEGIN | `id u32, flags u8, pathLen u16, relPath, size u64` | 双向 | flags：bit0 size 未知（流式源，size 字段填 0）/ bit1 目录项（建目录，size=0）。relPath 以 `/` 分隔的相对路径，接收方消毒（见 §7） |
| `D` | FILE_DATA | `id u32, flags u8, data` | 双向 | 数据 1 B ~ CHUNK 字节。flags：bit0 DF_ZSTD=1 → data 为该块独立的 zstd 帧（level 1）；压缩后不小于原文则置 0 发原始字节（逐块回退）。sha256 恒对解压后字节计算 |
| `E` | FILE_END | `id u32` | 双向 | 该 id 数据发完 |
| `R` | FILE_PROGRESS | `id u32, bytes u64` | 双向 | 接收方每 PROGRESS_EVERY 回一帧（进度显示 + 存活探针） |
| `F` | FILE_RESULT | `id u32, status u8, sha256[32]` | 双向 | status：0 成功 / 1 哈希不符 / 2 IO 错误 / 3 取消。成功时附整文件 sha256 |
| `X` | CANCEL | `id u32` | 双向 | 接收方 IO 失败时通知发送方停止（如磁盘满） |
| `Q` | PAIR_REQUEST | `codeLen u8, code, nameLen u8, name` | A→PC | 仅配对流程 |
| `A` | PAIR_ACCEPT | `nameLen u8, name` | PC→A | 配对成功 |
| `Y` | BYE | `reasonLen u8, reason` | 双向 | 主动告别（版本不符/正常停机等）后断开 |
| `G` | BATCH_BEGIN | `items u32, files u32, bytes u64` | 双向 | 一次用户发送动作（一批文件）的首个 `B` 帧前发一次；接收方据此把批内全部文件的进度与结果聚合成单一事件（总量/总字节供进度条）。仅含目录的批不发（无 F 帧，接收方按目录帧各自成事件） |

## 6. 会话与可靠性语义

- **单连接双向复用**：剪贴板小帧可插队在文件 `D` 帧之间（帧原子性由长度前缀保证），文件传输不阻塞文本同步。
- **传输不等待 ACK**：发送方连续写 `D` 帧靠 TCP 流控；`R` 仅作进度/存活信号。**可靠性锚点是 `F` 的 sha256**：发送方边发边算哈希（v2 起对解压后字节），收 `F(status=0)` 后比对，不符则整文件重发一次（新 id），再不符报错并通知用户。
- **文件级流水线窗口（v2）**：同方向最多 `window` 个文件的 `E` 已发而 `F` 未回（id 单调递增，可交错；接收端本就按 id 归属数据，天然支持）。窗口消除逐文件「RTT + 对端落盘 + 回 F」的串行等待（transfer-perf.md §4：小文件约一半耗在等 F）。PC 侧可配（1..8，默认 4；`window = 1` 即 v1 停等行为），Android 固定 4。批内失败时该批未送达文件整批重排队（v1 语义保持）。
- **传输压缩（v2）**：发送侧逐块 zstd（level 1），按扩展名白名单跳过已熵编码格式（zip/png/mp4/apk 等，双端同表）；`B.size` 与 `F.sha256` 均为解压后语义，接收端解压后落盘，解压失败按该文件 IO 错误处理（作废单文件，不断连接）。
- **断线**：传输中连接断开 → 在途文件作废；手机重连、双方 HELLO 后，PC 出站队列（§8）与手机出站队列自动从头重发整个文件。不做断点续传（刻意从简，与「队列不持久化」的哲学一致）。
- **批量事件聚合（v2.1）**：发送方以 `G` 帧划界一次用户动作（PC 侧一次 `send_paths` 调用 = 一条队列项 = 一批）；接收方在批内不逐文件撤/建传输通知、不逐文件发完成事件，批收尾（G 声明的文件数全部收到 E/X 且无在途）时发一条聚合事件（如「收到文件夹：docs（17 个文件 · 45MB）」）。批被新 `G` 顶替（如整批重发）时已落盘部分立即结算。未见 `G` 的 B 帧流落入隐式批（兼容降级，批界按在途清空推断）。
- **心跳与死链**：探针实测教训——adb/网络抖动会产生无 FIN/RST 的静默半开连接，应用层双向看门狗是唯一可靠手段，双端都必须实现「45s 无字节即断」。
- **重连**：手机退避重拨；用户产生新任务（分享 / 立即同步 / 配对）时 nudge 立即全速。

## 7. 路径与文件规则

- relPath 消毒（接收方强制）：剥首部 `/`，丢弃 `..` 与空段，其余保留（含中文与空格）。递归目录的层级结构原样重建。
- 覆盖策略：目标已存在 → 追加 ` (1)`、` (2)` 序号（不改内容，不覆盖）。
- 落盘位置：
  - PC 收件：`~/Airpipe/from_phone/`（config 可改）
  - Android 收件：`/storage/emulated/0/Airpipe/`
- stdin 源（`ap send -`）：size 未知，flags.bit0=1，命名 `stdin-<YYYYmmdd-HHMMSS>.bin`。
- 空目录：flags.bit1=1 的 `B` 帧，用于递归发送时保留目录骨架。

## 8. PC 出站队列（掉线缓存）

- 队列元素只有**引用**：文本为字节本体（小），文件为本地路径列表；绝不缓存文件内容。
- 文本条目**合并**：多次文本同步只保留最新一条（剪贴板语义天然如此）。
- 文件条目 FIFO；单次入队内部去重，**跨条目不去重**（重复 `send` 同一路径 = 用户显式要求再送一份，接收端按 §7 追加序号，不覆盖）。
- 队列**不持久化**：daemon / 系统重启即丢，用户重传——刻意设计，防止状态文件与磁盘实态漂移。
- 手机上线（HELLO 完成）即按序清空队列；在线期间新入队条目即时唤醒泵发送（泵空闲时在队列条件变量上等待，不退出）。

## 9. 剪贴板同步规则（PC 侧 watcher）

优先级沿用 clipboard_probe 的分类结论：

1. `x-kde-passwordManagerHint` → **敏感，不读不传**
2. 文件引用（`x-special/gnome-copied-files` / `text/uri-list`）→ **不自动传**（仅托盘「发送剪贴板文件」显式触发）
3. 图片（png > jpeg > webp > bmp > tiff，再退而求其次任意 `image/*`）→ 自动 `I` 帧，超 `MAX_IMAGE` 跳过并记日志
4. 富文本 → 取 `text/plain` 渲染版发 `T`
5. 纯文本 → `T`
6. 其他 MIME → 有 `text/*` 取之，否则跳过记日志

**回声抑制**（防环）：daemon 每次自己写入剪贴板（收到手机的 T/I、收到文件后写入的文件引用）都记录内容哈希；watcher 看到哈希命中即跳过同步。手机侧不会自动读剪贴板（读必须经蹦床），天然无环。

**手机→PC 文本**：蹦床读到文本 → `T` → daemon 直接写入 PC 剪贴板（wlr-data-control 无焦点限制）。
**手机→PC 文件**（分享 / 剪贴板 URI）：按 §5 文件流传输；落盘后 daemon 额外把收到的文件 URI 以 `x-special/gnome-copied-files`（copy）写入 PC 剪贴板 → 用户在文件管理器直接 Ctrl+V。

## 10. IPC（仅 PC 本机，CLI ↔ daemon）

Unix 域套接字：`$XDG_RUNTIME_DIR/airpipe/daemon.sock`（无 XDG_RUNTIME_DIR 时 `/tmp/airpipe-$UID.sock`），**JSON 行协议**（明文，本机信任边界内）：

```jsonc
→ {"cmd":"status"}
← {"ok":true,"proto":1,"uptime_s":123,"phone":{"name":"mi-13p","ip":"192.168.0.23","connected":true,"since":...},
   "queue":{"texts":1,"files":3,"bytes":1073741824},"paired":[...],"recv_dir":"...","port":17580}

→ {"cmd":"send","paths":["a.jpg","docs/"]}
← {"ok":true,"queued":3,"online":true}

→ {"cmd":"text","data":"hello"}
← {"ok":true}

→ {"cmd":"send_clip_files"}          // 托盘同款：读剪贴板文件引用入队
← {"ok":true,"queued":2,"online":false}

→ {"cmd":"pair","timeout":60}        // 挂线等到结果或超时
← {"event":"code","code":"483920"}
← {"event":"paired","name":"mi-13p"}

→ {"cmd":"unpair","name":"mi-13p"}   // 或 "all"
→ {"cmd":"stop"}
```

单实例约束：sock 绑定失败即「daemon 已在运行」，CLI 全部命令经 IPC 走守护进程，自身不做业务。

## 变更日志

- 2026-11-04 v1 初稿（审议中）
- 2026-11-06 v2：D 帧载荷加 `flags u8`（bit0 DF_ZSTD，逐块独立压缩、可逐块回退）；§6 新增文件级流水线窗口与传输压缩语义；PROTO_VER 1→2（破坏性，旧新互连版本拒绝）。动机与实测收益见 `transfer-perf.md`。
- 2026-11-06 修正：§8 文件队列取消跨条目去重（原「同一路径重复入队去重」会在前一份仍在队列/在途时静默丢弃后续 `ap send`，造成文件丢失）；§10 send 系回复加 `online` 字段，CLI 据此显示手机在线/离线提示。
- 2026-11-06 v2.1：新增 `G` BATCH_BEGIN 帧（双向，加性变更不升 PROTO_VER，旧端忽略未知帧）；§6 批量事件聚合语义。动机：PC→手机发文件夹/多文件时，手机端通知逐文件闪烁且常驻通知最终显示内部文件名——用户语义「一次发送 = 一个事件」。
