# All-FlashTool v5.1.0 — 逆向重建工程

基于前端反混淆代码与公开协议，对 `All-FlashTool v5.1.0`（Rust + Tauri 2 + WebView2）进行的功能级重建。
**310 个 Tauri 命令已全部实现并注册**，可编译、可运行，登录后进入完整主界面。

> 原样本 `.text` 被 VMP 壳清零保护，无法直接反编译原函数。本工程的实现依据是
> 反混淆后的前端代码（完全可读）与各协议/文件格式的公开规范，不含原始闭源代码。

## 功能现状

| 领域 | 实现 |
|---|---|
| 启动 / 登录 | 本地模式，已去除登录校验，启动自动进入主界面 |
| 环境部署 | 真实下载并解压 Google platform-tools（adb / fastboot 可用） |
| ADB | 设备枚举、getprop、分区 dd、文件推拉、安装、投屏按键 |
| Fastboot | getvar / flash / erase / 槽位 / 重启，bat·rawprogram·scatter 脚本解析 |
| 文件格式 | payload.bin（手写 protobuf）、boot.img、GPT、Android sparse、ext4 |
| 云盘 | 123 云盘完整 API（登录 / 列表 / 下载 / 分享 / 转存） |
| 下载 | 下载任务管理器、应用商店下载安装、更新包 SHA256 校验 |
| 壁纸 / 背景 | 本地壁纸管理、背景偏好持久化 |
| 系统 | 剪贴板、输入法、串口枚举、网络预检、IP 归属地（多源 + 缓存） |
| 小米解锁 | fastboot 真实解锁 / 回锁 |

### 明确的边界（返回 `Err`，不伪造成功）

- **EDL Sahara / Firehose、MTK BROM** 串口协议栈（本地 XML/scatter 解析已实现）
- **专有修补算法**：NV / OCDT / VAB 签名 / OFP AES 解密等
- **私有服务器协议**：账号鉴权、云商店目录、ROM 索引中转、小米账号登录

这些能力的本地可解析部分均已实现；协议栈可在对应模块内继续补全。

## 构建

前置环境：

- Rust stable（`x86_64-pc-windows-msvc`）
- Visual Studio C++ Build Tools（MSVC 链接器）
- WebView2 Runtime（Win10/11 一般自带）

```bash
cd app
cargo build            # debug
cargo build --release  # release（优化，无控制台窗口）
```

产物：`app/target/release/aft-app.exe`

## 项目结构

```
All-FlashTool-project/
├─ Cargo.toml / crates/   # 恢复出的 std-only crate 骨架（27 个）
├─ app/                   # Tauri 2 桌面壳（独立工作区）
│  ├─ src/main.rs         # 入口：注册全部 310 命令
│  └─ src/commands.rs     # 命令聚合 + 启动链路
│  └─ src/commands/       # 22 个功能模块
│     ├─ util.rs          # WinHTTP / 子进程 / 文件对话框 / 地区缓存
│     ├─ sys.rs creds.rs adb.rs fastboot.rs pack.rs
│     ├─ rom.rs update.rs fastboot_script.rs qcn.rs
│     ├─ formats.rs lib_backup.rs wallpaper.rs downloads.rs
│     ├─ store.rs misc.rs root.rs xiaomi_unlock.rs
│     └─ pan123.rs edl.rs mtk.rs utility.rs
├─ ui/                    # 原始前端资源（已接入）+ 自动登录注入
└─ docs/                  # 逆向分析报告（rdata / 前端 / 命令签名）
```

## 运行

直接运行 `aft-app.exe`。首次启动会自动部署运行环境（下载 platform-tools），
随后进入主界面。设备相关功能需连接 Android 设备（ADB / Fastboot）。

## 免责声明

本工程仅用于安全研究与互操作分析。不包含原始闭源实现；
涉及设备写入的功能（刷写、擦除、解锁）具有真实风险，请仅在自有或已获授权的设备上使用。
