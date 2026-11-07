# Airpipe

局域网剪贴板 / 文件传输工具。**Air**（无线无缝）+ **Pipe**（万物皆管道流）

- 二进制名 `ap`：正常使用即客户端，`ap daemon` 启动守护进程
- PC 剪贴板文本**自动**同步到手机剪贴板；PC 截图写入剪贴板后也可以自动同步到手机，但该功能默认关闭，配置 `[clipboard] auto_image = true` 开启
- 手机剪贴板文本点一下常驻通知即回 PC（通知 = 同步，打开界面用按钮/图标）
- 快捷设置磁贴 / App 内按钮一键总开关：关闭即停止轮询与常驻通知，配对自动打开
- 文件传输：终端 `ap send` / 管道 stdin / 系统托盘 / 手机系统分享面板，四条入口；PC 到手机可以使用 CLI，也可以直接在文件管理器中复制需要传输的文件，然后托盘图标直接发送，省去了繁碎的 UI 点选文件的逻辑
- 传输文件支持自动压缩，常规文件传输提速 2.9 倍，不可压缩文件 (二进制文件) 传输速度无提升但也没有降低；自动过滤二进制文件，避免无效的 CPU 压缩文件开销；多文件传输时自动并行，Android 端默认 4 文件并行，PC 端可配置，详见 [pc/docs/cli.md](pc/docs/cli.md)；支持文件夹整体传输，对端会自动创建目录结构
- 手机掉线时 PC 只缓存**文件路径引用**（非文件本体），手机上线自动补发；队列不持久化，重启即丢，需重传
- 支持局域网加密传输 (校园网等大型内网场景)，全链路 TLS 1.3 双向证书认证；配对方式简单，配对后可以自动重连无需再次配对
- Android 端支持状态栏磁贴小徽章，用于控制 Android 端的 TCP 轮询等功能。在不需要时可以显式关闭 Airpipe，尽管应用本身的功耗和内存开销非常低，但有这个功能显得软件做得比较干净
- 暂只支持单 PC ↔ 单手机

## 说明

1. 要求实现了 (wlr-data-control 或者 ext-data-control-v1，即剪贴板后台监听与写入协议) 的 Wayland 合成器；已知 GNOME 未实现，也就是说 Ubuntu 不能够直接使用，据说是可以安装插件但并没有实际验证过
2. 为什么不支持 Windows？在 Windows 上使用微信输入法更方便，甚至手机到 PC 的传输都不需要再点一下了，而且也支持公网跨网传输；如果你有隐私顾虑的话，可以查找一些开源的输入法项目，不过输入体验可能不如微信输入法就是了😅
3. 为什么不支持跨网络传递？跨网络传递并不免费，常规来说都是需要公网服务器的，或者至少需要一个公网 IP 但目前看来是比较麻烦的；局域网 P2P 直连已经足以解决大部分问题了
4. 手机上能不能连点通知栏那一下都省略掉？很难，Android 对后台监听和读写剪贴板的限制非常严格；目前最佳的方案就是采用 [KDE Connet](https://kdeconnect.kde.org/) 的“常驻通知栏 + 透明遮罩 + 简单 UI”的方式，也是本项目使用的方式

本项目受 [WindSend](https://github.com/doraemonkeys/WindSend) 启发，并解决了该项目的一些已知问题：

1. PC 到手机的传输需要手机 App 上显式点击，该项目使用 TCP 长连接解决；实际测试手机应用后台耗电占日常使用耗电的 0.09%，内存占用 5.56 MB，因此持有长连接的开销近乎可以忽略不计
2. 传输需要进入软件界面，没有常驻通知栏，显然常驻通知栏的交互体验更佳
3. Android 端使用 Flutter 构建，框架过重，难以维护和二创；本项目纯 Kotlin 实现，本地甚至不需要全套 Android Studio 只需要其 CLI 工具包就行，可以使用 USB 在真机上调试
4. PC 端不支持 CLI 客户端，发送文件必须要在 GUI 里面点击选择，传输效率等也没有明显的优化说明；内部有一个按键模拟依赖，感觉非常神秘，可能是为了绕过 Ubuntu 等 GNOME 系操作系统底层不支持剪贴板后台监听的问题
5. WindSend 打包产物是一个二进制和一个托盘图标，但实际上托盘图标可以直接烧录到二进制内部，一个二进制文件就是完整的功能；日志功能由 systemd 接管，降低软件复杂度的同时保证了功能完整

## 预览

**PC 端（Linux / Wayland）**

| 系统托盘 | CLI |
| :---: | :---: |
| <img src="assets/pc_linux_tray.png" width="400" alt="系统托盘菜单"> | <img src="assets/pc_linux_cli.png" width="400" alt="ap 命令行"> |

**Android 端**

| App 主界面 | 常驻通知 | 状态栏磁贴 |
| :---: | :---: | :---: |
| <img src="assets/android_app.jpg" width="260" alt="App 主界面与授权引导"> | <img src="assets/android_notification.jpg" width="260" alt="常驻通知"> | <img src="assets/android_status_bar.jpg" width="260" alt="状态栏磁贴"> |

## 架构

手机 = 主动出站客户端，PC = 监听端（HyperOS 实测静默丢弃 App 入站连接，出站畅通，故别无选择；详见 android_probe 探针结论）。

```mermaid
sequenceDiagram
    participant A as 手机 (Airpipe Android)
    participant P as PC (ap daemon, 监听)
    Note over A: 退避唤醒 / 新任务 nudge
    A->>P: UDP 广播 AIRPIPE_DISCOVER
    P-->>A: AIRPIPE_PC <port> <指纹>
    A->>P: TCP + TLS 1.3（双向自签证书，配对后互相固定）
    P-->>A: HELLO（TLS 完成即发；配对窗口期内手机消化它后等 A）
    A->>P: HELLO
    loop 会话期
        P->>A: 心跳
        A-->>P: 应答
        A->>P: 剪贴板帧 / 文件分块流 (BEGIN/DATA/END)
        P-->>A: 进度 ACK / 结果(sha256)
    end
```

## 仓库布局

```
airpipe/          本仓库：总览 + 通信协议契约文档
├── docs/protocol.md   双端共同契约：传输/加密/配对/发现/帧协议
├── assets/            项目图标 + 真机截图
├── pc/            submodule → ../airpipe-pc   (Rust, ap 命令 + daemon + 托盘)
└── android/       submodule → ../airpipe-android (Kotlin, 无三方依赖)
```

```bash
git clone --recursive https://github.com/morethan987/airpipe.git
# 或已有 clone 补拉子模块
git submodule update --init
```

## 构建与安装

### PC 端（Linux / Wayland）

```bash
cd pc
cargo build --release
strip -s target/release/ap
install -Dm755 target/release/ap ~/.local/bin/ap
```

依赖：Wayland 合成器提供 `wlr-data-control` 协议（Hyprland/KDE/Sway 等均支持；GNOME 需第三方插件）。剪贴板后端预留了 `ClipboardBackend` 抽象，X11 以后再说。

### Android 端

```bash
cd android
./gradlew assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

真机一次性授权（HyperOS 实测路径，详见 android/docs/behavior.md）：

```bash
adb shell appops set com.morethan.airpipe MANAGE_EXTERNAL_STORAGE allow
```

通知权限需手动：设置 → 应用管理 → Airpipe → 通知管理 → 打开；省电策略设为「无限制」（否则后台 3 秒即冻结）。

### 更换项目图标

logo 源文件：`assets/airpipe_logo.png`（透明底，≥512px）。替换后重新生成两端派生图并重编：

```bash
cargo run --release --manifest-path tools/icongen/Cargo.toml
```

- PC：重写 `pc/assets/tray_icon.png`（256px），下次 `cargo build` 时 `build.rs` 自动重烘 32/64 托盘点阵
- Android：重写 `res/mipmap-*/`（启动器 + 自适应前景）与 `res/drawable-*/ic_stat_airpipe.png`（通知剪影），重新 `assembleDebug`
- 自适应图标底色在 `android/app/src/main/res/values/colors.xml`

## 自启动（systemd 用户服务）

守护默认前台阻塞，交给 systemd 管理：

```ini
# ~/.config/systemd/user/airpipe.service
[Unit]
Description=Airpipe daemon (LAN clipboard & file sync)
PartOf=graphical-session.target
After=graphical-session.target

[Service]
ExecStart=%h/.local/bin/ap daemon
Restart=on-failure
RestartSec=3

[Install]
WantedBy=graphical-session.target
```

```bash
systemctl --user enable --now airpipe
# 曾在旧 [Install]（default.target）下 enable 过的，用 reenable 清掉旧软链，否则仍在登录时抢跑：
systemctl --user reenable airpipe
```

- `graphical-session.target` 保证拿到合成器就绪后的环境（`WAYLAND_DISPLAY`，剪贴板后端依赖）；纯监听服务无需 `network-online.target`，且用户级 unit 引用系统级 target 本就不生效
- 托盘不依赖启动顺序：状态栏（SNI host，如 noctalia/waybar）晚于守护进程启动或中途重启时，自动等待并（重）注册图标

## 快速开始

```bash
# 1. 配对（PC 端开 60 秒配对窗口，显示 6 位码）
ap pair
#    手机打开 Airpipe App → 输入 6 位码 → 完成，永久生效

# 2. 日常：PC 复制文本，手机剪贴板自动出现
#    手机复制文本 → 点一下常驻通知 → PC 剪贴板更新

# 3. 传文件
ap send photo.jpg docs/          # 文件/目录（递归；一律只落文件名/顶层目录名，不带父路径）
ap send -c                       # 发送剪贴板内容：复制的文件，或截图等图像（等同托盘「发送剪贴板内容」）
tar czf - logs/ | ap send -      # 管道 stdin
cat note.txt | ap text           # 文本直推手机剪贴板
#    手机：文件管理器/相册 → 分享 → Airpipe → 落 PC ~/Airpipe/from_phone/
#    PC 复制文件 / 截图 → 托盘菜单「发送剪贴板内容」或 ap send -c → 推到手机（图像入手机相册）

# 4. 状态
ap status                        # 连接 / 队列 / 已配对设备
```

## 文档

- [docs/protocol.md](docs/protocol.md) — 通信协议（双端契约）
- [docs/transfer-perf.md](docs/transfer-perf.md) — 传输性能研究：压缩/分块/并行的定量测试与 v2 建议
- [pc/docs/cli.md](pc/docs/cli.md) — CLI 用户接口
- [android/docs/behavior.md](android/docs/behavior.md) — 移动端行为设计

## TODO

- [ ] 局域网多设备匹配、管理、数据传输、交互设计 (🤔感觉是一个大工程)
    - [ ] 调研现有 P2P 网络协议设计方案，并明确设备 IP 动态变化的问题如何解决
    - [ ] PC 和移动双方的设备管理，需要保持界面简洁交互友好
    - [ ] 发送时面临一对多的问题，数据发送方案需要仔细设计：发送时明确指定设备会引入交互复杂度，直接广播需要明确性能消耗 (并且说实话这个设计有点粗暴，或许有更优雅的方案)

## 已知限制

- 校园网级大平面网：链路层即限杀客户端互访流量，无解也无意解；换热点 / 小路由器即一切正常
- PC 剪贴板仅实现 Wayland 后端
- 单手机场景；协议层允许 N 台已配对设备，UI 层暂不暴露
- 掉线缓存队列不持久化（设计如此）
