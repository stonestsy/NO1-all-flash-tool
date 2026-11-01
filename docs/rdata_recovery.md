# `.rdata` 补齐实验报告

## 目标
尽可能恢复壳加密的 `.rdata`（原始 19 MB，未解密时不可读）。

## 方法
在本机以管理员（UAC）运行样本，按时间点对主进程做内存取证：

| 快照 | 时机 | 方式 | `.rdata` 页统计（共 4603 页，4KB/页） |
|---|---|---|---|
| dumps2 | 1.8 s（挂起） | PE-sieve `/minidmp` | zero=797，高熵(未解密)=3710，已解密≈92 |
| dumps3 | 3.6 s（挂起） | PE-sieve `/minidmp` | zero=0，高熵=3985，已解密≈583 |
| dumps6 | 4.5 s（挂起） | PE-sieve `/minidmp` | zero=0，高熵=3985，已解密≈583 |
| dumps7 | ≥5 s（非提权） | RUNASINVOKER 运行 | 进程被壳提前终止，未取得有效快照 |

`.text` 在 1.8 s 即已 **100% 解密**（2784 页，0 页高熵，熵 ≈ 6.0）。
`.rdata` 到 3.6 s 后不再增加（583 页 ≈ 2.3 MB 可读），其余约 15 MB 始终高熵。

## 结论
- 该壳对 `.rdata` 采用**按页惰性解密**（访问时解密，之后可能重新加密/释放），因此**内存 dump 只能拿到运行中被访问过的页**，无法“补齐”全部 19 MB。
- 想进一步恢复 `.rdata`，可行路径只有：
  1. **逆向导壳的页解密例程**（位于 `.h4` 的明文引导区，`.text` 已解密可供参考），离线解密整段；
  2. 让程序**持续运行并交互**（登录、切页、刷机流程），同时对进程做**高频多次快照**并按页合并（取每页熵最低的版本）；
  3. 通过内核/用户态 hook（如 PageGuard、`NtProtectVirtualMemory`）在解密后立即转储对应页。
- 已有快照中能读到的最有价值 `.rdata` 内容已提取到：
  `../All-FlashTool-unpacked/strings_ascii.txt`、`strings_cjk_utf8.txt`、`urls_endpoints.txt`。

## 追加实验：交互式高频采样（方案 1）
在用户以管理员运行样本并尝试登录期间，使用提权采样器按 6 秒周期执行
“**NtSuspendProcess → PE-sieve `/minidmp` → NtResumeProcess**”（必须挂起，否则壳会杀掉读取其内存的工具）。

结果：
- 登录尝试期间 `.rdata` 可读页从 **583 → 618 页**（+35 页，约 140 KB），高熵页仍为 3985
- 两次快照内容一致（登录被“无效密钥”拦截，未进入功能页，因此解密增量有限）
- 合并（取各快照中每页最优版本）后仍为 **618 / 4603 页**（`.rdata` 仍约 15.5 MB 未解密）

合并产物：
- `../All-FlashTool-unpacked/merged/All-FlashTool-v5.1.0.merged.exe`（57.9 MB，618 页可读）
- `../All-FlashTool-unpacked/merged/strings_ascii.txt`（9853 条）
- `../All-FlashTool-unpacked/merged/strings_cjk_utf8.txt`（5593 条）
- `../All-FlashTool-unpacked/merged/urls.txt`（118 条）

结论：`.rdata` 的进一步恢复依赖**成功登录并实际使用功能页**（触发更多页面解密），
或逆向壳的页解密例程做离线解密；单靠登录界面交互增量很小。

## 方案 3（NtProtectVirtualMemory hook）尝试与结论：受阻
目标：在样本进程内 hook `ntdll!NtProtectVirtualMemory`，在页面保护变更（解密完成）瞬间
由目标进程自己用 `CreateFileW/WriteFile` 把页面写到磁盘。

两次尝试：
1. **附加运行中的样本**（提权 `svcmon.exe` + frida）：宿主启动后数秒内被壳终止，
   日志仅有 `host start`，未进入 `agent loaded`；未产生任何页面文件。
2. **Frida spawn 门控注入**（先 taskkill 旧实例 → `frida.spawn` → attach → load agent → resume）：
   `spawned pid=19544` 成功，但宿主在 agent 加载前再次被终止；样本停留在挂起态，
   最终被清理脚本强制结束；页面文件数 = 0。

结论：该壳对“访问其进程的进程”有极强的主动查杀（含 spawn 门控下的注入宿主），
**用户态 Frida/Detours 路线不可行**。要在此壳下实现页解密瞬时转储，需要：
- 内核驱动级 hook/页保护监控（需测试签名或禁用驱动签名强制），或
- 离线逆转向量：定位壳的页解密例程/页表/密钥，在样本外解密 `.rdata`。

## 方案 2（离线逆向壳页解密例程）：静态取证完成，判定不可行（VMProtect 类保护）
本节记录“不运行样本、纯离线逆向壳解密例程”的全部结果。结论先说：
**剩余 `.rdata` 的主要部分不是壳密文，而是 App 自加密资源；且壳为 VMProtect 级虚拟化 + 按页惰性解密，
离线无密钥无法还原。** 证据如下。

### 1. 壳结构与入口
- PE 头中 `.text`(0xadf1f0)、`.rdata`(0x11fabba)、`.data`、`.pdata` 的 `SizeOfRawData` 均为 0；
  全部内容位于 `.\h4`（原始 0x17f4e00 ≈ 25.1 MB，熵 7.92）。
- 入口点 RVA `0x35f06b8` 在 `.h4` 内且为**诱饵**（后续字节为随机数据）。真正入口是 TLS 回调：
  `0x35da6f5`（cmp edx,1 / jne / sub rsp,0x28 / call 0x35f7f9b）→ `0x35f7f9b`（push r9;pushfq;movabs;call 0x35dcb4f）。
  `.h4` 末尾 157 个低熵页是明文引导器（含上述 stub、EP stub `0x35efb48` 等），其余为整体密文。
- `.:V~` 段（RVA 0x1d35000，0x204f01）为壳代码区；内存中 133/517 页可读。运行时在堆/线程栈中可见
  连续指向 `.:V~` 的 handler 表（0x143390 起 ≥43 项，步长 0x30 的表在 0x2364e00）。
- 壳 hook 了 `ntdll!NtProtectVirtualMemory`：桩 `7ffaff930015` → 槽 `0x141d3ab80`(jmp) →
  `0x141f2de54`（参数重排 trampoline：rcx=全局ctx, rdx=ProcessHandle, r8=&Base, r9=&Size, [rsp+0x20]=NewProtect,
  `call 0x141eb5e2b`）→ `0x141eb5e2b` 混淆分发器（5 个 VM 入口调用：0x1edcc93 / 0x1e4bcd3 / 0x1df73e5 /
  0x1d51a42 / 0x1d8ee79）。另 hook 了 `ntdll!DbgUiRemoteBreakin`（cave2 `7ffaff94001f`）。
- 指纹判断：TLS 回调形态 + `movabs/pushfq/popfq` 栈混淆 + handler 表 + PEB/Ldr 动态解析 + `PAGE_GUARD`
  惰性解密，属于 VMProtect 3.x 系（区段名被自定义为 `.:V~` / `.\h4` / `.[sh`）。

### 2. `.rdata` 确为“按页惰性解密”，且密钥随构建静态
- `.pdata` 79 页全部熵 7.92~7.94（未访问的保护页保持密文）→ 说明保护覆盖其它段，按页解密。
- `.rdata` 4603 页中：618 页熵 <7.6（已解密，整页明文，含 IAT、Tauri 命令名等）；3920 页熵 >7.9（密文）。
- 早期快照（1.8 s）有 797 个零页；3.6 s 时全部落盘：523 页写成明文、274 页写成密文。
- 4 份快照（3 个进程：pid 8420@1.8s、22480@3.6s、22828@4.5s、16580×2）的 `.rdata` **逐字节一致**
  → 密文/密钥是构建期静态的，不是每次运行随机。
- (密文, 明文) 配对搜索结果：**0 对**。唯一“明文→密文”变化页 3805 是页内 256 B 明文头 + 后续写入的
  密文载荷，密文并非该页早期明文所加密（K = C^P 仅前 256 B 为 0，其余随机），不能作为已知明文对。

### 3. 关键新发现：剩余 15.5 MB 主要是 **App 自加密容器**，而非壳密文
- 在 `.rdata` 中定位到 6 个 `AFT1` 容器：`"AFT1"` + 偏移 +0x10 处 `uint32` 长度，长度与相邻容器起始
  精确吻合（间隔仅 0x4f~0x92 B 的头部/字符串）。
  | 容器 | RVA | 长度 | 覆盖页 |
  |---|---|---|---|
  | AFT1_0 | 0xaf7cb7 | 0xf32a | 22-37 |
  | AFT1_1 | 0xb073bd | 0xf6e2 | 38-53 |
  | AFT1_2 | 0xb16e40 | 0x456b70 (4.34 MB) | 53-1164 |
  | AFT1_3 | 0xf6da42 | 0x3798e8 (3.47 MB) | 1164-2054 |
  | AFT1_4 | 0x12e7381 | 0xe898 | 2054-2068 |
  | AFT1_5 | 0x12f5c68 | 0x1225e8 (1.13 MB) | 2068-2359 |
  | AFT1_6 | 0x14568a0 | 0x12a4 | 2421-2422 |
  合计 **9.13 MB / 2340 页**。容器头所在页是明文（被 App 访问过），可见 `crates\aft-jailbreak\src\payload.rs`、
  `offsets.bin`、`offsets.json`、`不是数组`、`解压失败` 等 Rust 字符串与 `AFT1` magic。
- 页 2431（RVA 0x1460000）前 0x19b 字节为明文资产元数据（`init DownloadManager`、`APPDATA`、
  `/assets/app-logo-BnL8-74-.png`），其后为密文；页 4008 的 0xd30~0x1000 为明文 Tauri 命令串，
  其前为密文。**同一页内明文/密文交界不在页边界（页 4008 交界 0xd30、页 2431 交界 0x19b）**，
  说明这些页已被壳整页解密，随机部分是 **App 级密文**。
- 因此，页 2431-4007 的连续 1577 页（≈6.3 MB）几乎可以确定是 App 加密的前端/资源包，
  与 AFT1 一样属于“应用层加密”，**即使拿到壳的页密钥也仍是随机密文**。
- 结论：剩余 3920 个高熵页中，约 2003 页落在 AFT1 容器内、约 1577 页为大资源包、
  零散约 70 页（如 4095、4098-4100、4424-4425、4522-4527、4530-4566 等）。
  即：**壳层“普通 .rdata 文本/表数据”残留已非常少（≲0.3 MB）**。

### 4. 已尝试的离线手段与失败原因
1. 固定/页无关 XOR 与周期分析：模式字节计数仅 23~39（随机期望 ~15），无强峰；16 B 块跨页几乎无重复
   → 排除固定 keystream / ECB / 页无关流。
2. 跨快照 (C,P) 配对：4 份快照 + 1.8 s 早期快照交叉，0 对（唯一 1 页见 §2）。
3. 内存密文与 `.\h4` 比对：取 60 页 × 48 B needle，0 命中；`.\h4` 是整体压缩/加密，不是逐页密文。
4. 页表/密钥表扫描：全内存（160 MB，422 区间）未发现按页递增 RVA 的 qword/dword 表；trampoline 读取的
   全局 ctx 指针 `[0x141d71c8]` 所在页在 minidump 中为 0（壳清零/隐藏），无法从快照取得。
5. 反汇编：`.:V~` 中 5 个被调用函数均为 VM 入口（`jmp` 到虚拟化分发器，夹杂 movabs/pushfq 混淆），
   静态反编译得不到算法；要还原需 VMP3 反虚拟化（NoVmp/VTIL 级别）或内核级监控。
6. 环境内无 Ghidra/IDA；即使安装 Ghidra，对 VMProtect 虚拟化代码的收益也有限（函数体是 VM 字节码）。

### 5. 建议的下一步（按收益排序）
1. **转向 App 层解密（推荐）**：`AFT1` 与资源包的解密/解压逻辑在 `.text` 中（已 100% 恢复，11.4 MB 明文）。
   用字符串 `AFT1文件长度无效` / `解压失败` / `不是数组` 的 xref 定位 Rust 函数，重建其流密码 + 解压
   （导入表无 BCryptDecrypt，说明是纯 Rust 实现的自定义算法/RC4/AES crate）。这条路线可离线解开
   9.13 MB AFT1 + ~6.3 MB 资源包，是剩余数据的真正来源。
2. 若坚持壳层：需 VMP3 反虚拟化，或运行样本并以内核页保护监控/高频快照触发更多页面（用户已排除运行）。

### 6. 本次新增产物
- `../All-FlashTool-unpacked/merged/rdata_pages.csv`：4603 页的 RVA / 熵 / 分类（plaintext / AFT1_x / protected_or_blob）。
- `../All-FlashTool-unpacked/merged/containers.csv`：6 个 AFT1 容器的偏移、RVA、长度、页范围。
- `../All-FlashTool-unpacked/heap_strings_ascii.txt`（66,371 条，含系统 DLL 字符串）、`heap_urls.txt`（92 条）。
- 离线分析脚本：`_work/offline/`（pair/pairs/coverage/printable/entropy/h4match/vptrs/vehfind/aft1/…）。

### 7. 后续：App 层容器已离线解开（2026-10-31 续）
既然壳层的剩余内容主要是 App 加密容器，就直接逆 App 层。结果：**7 个 `AFT1` 容器全部离线解密成功**。

**容器格式（`AFT1`）**
```
+0x00  "AFT1"                    4 字节 magic
+0x04  nonce                     12 字节（AES-GCM nonce）
+0x10  uint32 plaintext_len
+0x14  AES-256-GCM 密文          plaintext_len 字节
+0x14+len  GCM tag               16 字节
总长 = 0x24 + plaintext_len
```
AAD = 字符串 `aft-jailbreak-blob-v1`（21 字节，位于 RVA 0xaea394）。

**密钥派生**
- 代码在 `.text`：KDF 函数 VA `0x1400375a1`（另有 4 个变体 `0x140036b2c / 0x140037471 / 0x140037998 / 0x140037acc`）。
- 实现为 HKDF-SHA256：静态 seed → Extract → Expand(info=域名字符串, 32 字节)。
- 关键域名字符串（均可在 .rdata 明文页读取）：
  | KDF 函数 | info 字符串 | 用途 |
  |---|---|---|
  | 0x1400375a1 | `aft-jailbreak-blob-v1` | AFT1 容器（本组） |
  | 0x140036b2c | `aft-local-login-v1-v1` | 本地登录凭据 |
  | 0x140037471 | `aft-pan123-cloud-v1v1` | 123 云盘会话 |
  | 0x140037998 | `aft-local-login-v31v1` | 登录 v3 |
  | 0x140037acc | `aoc-encrypt-key-v31v1` | AOC 加密 |
- 由于 SHA-256 走 SHA-NI 指令（Unicorn 不支持），在单步仿真器中自行实现了
  `sha256rnds2 / sha256msg1 / sha256msg2`，并用标准 SHA-256 压缩函数测试向量校验通过
  （IV + 单块压缩结果与参考实现逐字节一致），再执行 KDF 得到 32 字节密钥：
  `3383e51c6eb080b3537648ecceb134ad2ea0351bcfab74fd77cff0291f663c67`（blob-v1）。

**解密产物（`../All-FlashTool-unpacked/aft1_decrypted/`）**
| RVA | 大小 | 类型 | 内容 |
|---|---|---|---|
| 0xaf7cb7 | 62 KB | ZIP | 78 个 KSU/GhostLock 内核配置（`index.conf`、`kernelsnitch-6x.conf`、5.15/6.1/6.6/6.12 各内核版本 .conf） |
| 0xb073bd | 63 KB | JSON | `bundled-payloads/1` 清单：119 个厂商/kernel 对应的 `preload.so`（含 sha256、命令 `LD_PRELOAD=<payload> /system/bin/id`） |
| 0xb16e40 | 4.34 MB | ELF ARM64 | **ksud**（KernelSU 守护进程，含 metamodule/su/prop 区域） |
| 0xf6da42 | 3.47 MB | ELF ARM64 | **ghostlock-extract**（从 boot.img/Image 提取内核 offsets 的工具） |
| 0x12e7381 | 58 KB | JSON | 67 条内核 offset 记录（`task_prio`/`task_pi_lock`/`task_cred` 等） |
| 0x12f5c68 | 1.13 MB | ELF ARM64 | **ghostlock_oplus** payload（W3 seccomp、KernelSU 加载、root 脚本） |
| 0x14568a0 | 4.7 KB | Shell | `#!/system/bin/sh` root 激活脚本（KSU 管理器/ksud 定位、SELinux、日志） |

**可复用脚本**
- `_work/offline/aft1_decrypt.py`：扫描 `.rdata` 中所有 AFT1 容器 → AES-GCM 解密 → 按类型保存并解 ZIP。
- `_work/offline/emu_keys.py` / `verify_block2.py`：单步 Unicorn + SHA-NI 实现，用于复算 5 个域的密钥。

**6.3 MB 高熵块 = Tauri 内嵌资源（brotli），已完整解出**
- `.rdata` 0xFA8CB0 处是资源表：4 个 32 字节条目 `(key_ptr, key_len, data_ptr, data_len)`，
  VA 0x141a89cb0（RVA 0x1a89cb0），条目如下：
  | key | 压缩数据 VA | 压缩长度 | 解压后 |
  |---|---|---|---|
  | `/assets/app-logo-BnL8-74-.png` | 0x141460195 | 5047 | 5053 B PNG（128×128） |
  | `/assets/index-DknuZDVu.js` | 0x141461565 | 6,412,778 | **21,409,388 B** 混淆 JS |
  | `/index.html` | 0x141a7ef4f | 256 | 542 B HTML |
  | `/assets/index-QyZyyXao.css` | 0x141a7f069 | 44,099 | 288,881 B CSS |
- 数据均为 **brotli** 流（无外层头），正好从表里的 data_ptr 开始；之前失败是因为把起点错当成
  0x97F19B（资源表第一条数据从 0x97F195 开始，JS 从 0x980565 开始）。
- 解压产物：`../All-FlashTool-unpacked/embedded_assets/`（`index-DknuZDVu.js`、`index.html`、
  `index-QyZyyXao.css`、`app-logo-BnL8-74-.png`）。
- **资源解析代码定位**：`.text` 0x14025b238 `lea rax,[0x141a89cb0]` → 写入 resolver 结构 +0x38，
  紧随 `mov qword [+0x40], 4`（资产数=4）；同段 0x14025b1f8~0x14025b2f9 负责构造该结构并调用
  Tauri 运行时（0x1408f9460 等）。该结构即 Tauri `AssetResolver` 的静态资产表，取用时按 key 匹配后
  brotli 解压。

## 方案 2 补充：剩余壳保护页盘点（2026-10-31 续 2）
把 AFT1 容器范围与 Tauri 资源表数据范围扣除后，重新按页分类（`merged/rdata_pages.csv`）：

| 分类 | 页数 | 说明 |
|---|---|---|
| `plaintext` | 614 | 熵 <7.6 的普通明文页 |
| `app_aft1` | 2340 | 6 个 AFT1 容器覆盖（含明文头页、AES-GCM 密文页） |
| `app_asset` | 1578 | 4 个 Tauri 资源（brotli 数据）覆盖（logo/JS/HTML/CSS） |
| `plaintext_he` | 19 | 熵高但有结构：证书 DER / 指针表 / 重复表（见下） |
| `shell_protected` | 52 | 真·壳密文页（208 KB） |

合计 4603 页，无遗漏。另有 `.pdata` **79 页（316 KB）全部壳密文**（0 条合法 RUNTIME_FUNCTION）。

**壳保护页分布（52 页）**
```
4095(1) 4098-4100(3) 4122(1) 4424-4425(2) 4466(1)
4523-4527(5) 4530-4566(37, 148 KB) 4571(1) 4573(1)
```
判定依据：
1. 熵 7.92~8.00、页内 16 B 块无重复、无指针/ASCII/零串结构；
2. 与已解密明文页逐页交错（4094/4096 明文、4095 密文），符合“按页惰性解密、未访问页保持密文”；
3. 无 AFT1/XMU1 头；按 5 个已知域密钥的 AES-GCM/ChaCha20 多种布局、brotli/zstd/lzma/deflate 均失败；
4. 148 KB 连续块（4530-4566）前后为零页，全镜像无任何 `(ptr,len)` 指向它。

**19 个“高熵但明文”页**
- 4394~4405 一带是 **X.509 根证书 DER 区**：相邻页可见 `QuoVadis Root CA 3 G3`、`Hellenic Academic
  and Research Institute`、`T-Systems Trust Center` 等证书主题串（每页 350~457 B ASCII）；
- 4097：页首是 3 个指向 `.text` 的指针（0x1406a84e0…）+ 表结构；
- 4522：重复 16 B 块 34 个；4572/4599~4601：大量 (u64,0) 成对结构与零填充；
- 4140：熵 7.65，属阈值边缘的普通数据。

**结论**
- 离线可恢复的 `.rdata` 已全部恢复；剩余真正不可读的只有 **52 页 `.rdata`（208 KB）+ 79 页 `.pdata`
  （316 KB）**，内容推测为 Tauri 命令注册表/panic 字符串/回调表所在的普通数据页（未被 App 在启动阶段访问）。
- 进一步恢复仍只有两条路：VMP3 反虚拟化（成本高），或运行样本触发这些页面访问（用户已排除运行）。
- `.pdata` 若后续需要，可由 `.text` 反汇编重建函数边界（不依赖壳）。

## 方案 3 复测（2026-10-31 续 3）：注入/调试路径均被安全校验拦截
- **Frida 注入**：沿用此前结论，宿主在数秒内被壳终止（两次）。
- **WebView2 CDP（无注入变体）**：按官方方式写入
  `HKCU\Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments`
  值 `All-FlashTool-v5.1.0.exe = --remote-debugging-port=9222`，手动双击启动后 App 直接显示
  **“安全校验失败”**、未真正进入登录页（WebView2 进程也未创建）；移除该策略后手动启动恢复正常。
  → 安全校验会检测 WebView2 调试开关本身，CDP 路线不可行。
- **PE-sieve 采样管线本身可用**：把 `dump64.exe` 改名为随机名后，在 `NtSuspendProcess` 挂起期间
  可正常产出 57.9 MB realigned 镜像 + minidump，App 存活（此前“0 文件/被杀”是工具名被查杀 +
  目录未递归统计导致的误判）。
- 结论：剩余 **52 页 `.rdata` + 79 页 `.pdata` 无法在不被检测的前提下从运行期获取**；可行路径只剩
  “合法登录并实际使用功能页”配合上述挂起快照管线，或 VMP3 反虚拟化。
- 环境已回滚：调试策略键、计划任务（AftCdp/AftSnap）、相关进程均已清理；手动启动恢复正常。

### 运行期采样复测（live3）
- 通过计划任务（提权、脱离父进程树）启动每 12 s 一次的“NtSuspendProcess → 改名 PE-sieve 快照 → Resume”，
  用户手动启动 App 并尝试操作，共采集 **15 份**快照。
- 结果：所有快照的 `.rdata` 可读页始终为 **618 页**，`shell_protected` 52 页与 `.pdata` 79 页无任何解密增量；
  唯一“新增”的第 163 页来自首份异常快照（内容为乱码），已丢弃。
- 结论：未成功登录/进入功能页时，壳不会解密这些页，采样管线也无内容可补；管线本身可用，待能实际使用功能页时重跑即可。
- 采样任务与临时镜像已清理（`_work/live3` 仅保留日志）。

## 附件
- `../All-FlashTool-unpacked/rdata_and_frontend/`：从快照裁剪的候选区域与 HTML。
- `../All-FlashTool-unpacked/aft1_decrypted/`：App 层容器解密产物（ZIP/JSON/ELF/脚本）。
- `../All-FlashTool-unpacked/embedded_assets/`：Tauri 前端内嵌资源（JS/HTML/CSS/PNG，brotli 解压后）。
- `../All-FlashTool-unpacked/merged/rdata_pages.csv`：逐页分类（plaintext/app_aft1/app_asset/plaintext_he/shell_protected）。
- 原始全量快照：`_work/dumps3/minidump_*/`、`_work/dumps6/main/`、`_work/dumps2/minidump_8420/`、`_work/live2/`。
