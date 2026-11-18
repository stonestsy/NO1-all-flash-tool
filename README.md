# All-FlashTool v5.1.0 — 逆向重建工程

对 `All-FlashTool v5.1.0`（Rust + Tauri 2 + WebView2）的功能级重建，可编译、可运行。
实现依据为反混淆后的前端代码与公开协议/格式规范，不包含原始闭源代码。

> 原样本 `.text` 被 VMP 壳清零保护，无法直接反编译原函数，因此工程为行为级重建而非二进制还原。

## 快速开始

前置环境：

- Rust stable（`x86_64-pc-windows-msvc`）
- Visual Studio C++ Build Tools（MSVC 链接器）
- WebView2 Runtime（Win10/11 一般自带）

```bash
cd app
cargo build --release   # 产物 app/target/release/aft-app.exe
cargo test              # 运行回归测试
```

首次启动会自动下载并解压 Google platform-tools，随后进入主界面。

## 功能现状

| 模块 | 状态 |
|---|---|
| 启动 / 登录 | 本地模式，免登录直接进入主界面 |
| 环境部署 | 真实下载并解压 platform-tools（adb / fastboot 可用） |
| ADB | 设备枚举、getprop、分区读写/擦除、文件推拉、APK 安装 |
| Fastboot | getvar / flash / erase / 槽位 / 重启，脚本解析与批量刷写 |
| 镜像格式 | payload 元数据、boot.img、GPT、Android sparse、ext4 解析 |
| 123 云盘 | 登录、列表、下载、分享、转存 |
| 下载 | 任务队列（暂停 / 取消）、商店下载安装、更新包校验 |
| 系统 | 剪贴板、输入法、串口枚举、网络预检、IP 归属地 |
| 解锁 | 小米 fastboot 解锁 / 回锁 |

### 近期安全与可靠性修复

- **退出码传播**：adb / fastboot 等子进程非零退出码现在作为失败返回，不再误报成功。
- **真实取消**：`fastboot_cancel` 会终止当前刷写进程，并停止后续批量任务。
- **分区名校验**：ADB 擦除、读回、写入、备份前校验分区名，阻断设备端 root shell 注入。
- **回归测试**：新增退出码、进程取消、分区名校验 3 个测试。

### 已知限制

以下能力会返回明确错误或空结果，不伪造成功：

- **协议栈**：EDL Sahara / Firehose、MTK BROM（本地 XML / scatter 解析已实现）。
- **专有算法**：NV / OCDT / VAB 签名 / OFP 解密、payload 解包、super 合并等。
- **私有服务**：账号鉴权、云商店目录、小米账号登录。
- **下载器**：当前为单连接，线程数设置仅记录；暂停后恢复会重新下载。
- **商店**：目录为空，等待云端接入。

## 项目结构

```
All-FlashTool-project/
├─ Cargo.toml / crates/   # 从二进制恢复的 std-only crate 骨架（27 个）
├─ app/                   # Tauri 2 桌面壳（独立工作区）
│  ├─ src/main.rs         # 入口：注册 310 个命令
│  ├─ src/commands.rs     # 命令聚合 + 启动链路
│  └─ src/commands/       # 22 个功能模块（util / adb / fastboot / formats / ...）
├─ ui/                    # 原始前端资源（已接入）
└─ docs/                  # 逆向分析报告（rdata / 前端 / 命令签名等）
```

## 文档

- [命令签名](docs/command_signatures.md)
- [前端恢复](docs/frontend_recovery.md)
- [.rdata 恢复](docs/rdata_recovery.md)
- [文件格式](docs/file_formats.md)
- [crate 映射](docs/crate_map.md)

## 免责声明

本工程仅用于安全研究与互操作分析。涉及设备写入的功能（刷写、擦除、解锁）具有真实风险，请仅在自有或已获授权的设备上使用。
