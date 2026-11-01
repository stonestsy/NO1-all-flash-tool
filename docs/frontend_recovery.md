# 前端资源恢复报告（Tauri / WebView2）

## 已恢复（精确内容）
**`index.html`（541 字节，字节级一致）**，已保存到 `ui/recovered/index.html`：

```html
<!DOCTYPE html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>All-FlashTool</title>
    <link rel="stylesheet"
      href="https://font.sec.miui.com/font/css?family=MiSans:400:Chinese_Simplify,Latin&display=swap" />
    <script type="module" crossorigin src="/assets/index-DknuZDVu.js"></script>
    <link rel="stylesheet" crossorigin href="/assets/index-QyZyyXao.css">
  </head>
  <body>
    <div id="app"></div>
  </body>
</html>
```

由此确认：
- 前端为 **Vue 3 单页应用**（`<div id="app">` + 运行期 `createApp`）
- 字体：**MiSans**（`font.sec.miui.com`）
- 资源：`/assets/index-DknuZDVu.js`（主包）、`/assets/index-QyZyyXao.css`
- 协议：`http://tauri.localhost/`（WebView2 自定义协议）
- WebView2 数据目录：`%LOCALAPPDATA%\online.all-flashtool.app\EBWebView`

## 未恢复（原因）
- **JS 主包源码**：V8 只保留编译产物；渲染进程 `Code Cache\js\2b204c087fdef1d4_0`（3.1 MB）
  中不含 `function(`/`const` 等源码文本，但含 **39,885 个 `_0x…` 标识符**，说明主包经过
  *obfuscator.io* 风格的混淆（字符串与标识符均被改写）。堆内仅见碎片化标识符，
  未发现完整的连续源码。
- **CSS**：渲染进程内存中未检索到 `index-QyZyyXao.css` 的文本内容（仅命中请求 URL 与 MIME `text/css`）。

## 尝试过但不可行的路径
1. **内存裁剪**：按中文字符串/`_0x`/`function(` 定位连续源码 → 均失败（源码非连续留存）。
2. **磁盘缓存**：`EBWebView\Default\Cache` 无该资源响应体（custom protocol 不落 HTTP 缓存）。
3. **WebView2 远程调试（CDP）**：
   - 注入 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223` → 端口未开放
     （Tauri 发布版显式设置了 `AdditionalBrowserArguments` / 未启用 devtools）。
   - 通过 `AppCompatFlags` 的 `RUNASINVOKER` 免提权启动可成功运行时，但壳会因缺少管理员权限
     在数秒内终止进程，无法维持 WebView 完成加载。
4. **修改清单为 asInvoker 的副本**：壳的自校验拒绝运行（窗口不出现）。

## 如需继续
- 在**有效授权/管理员**环境运行，并**与界面交互**（登录后浏览各功能页），同时用 PE-sieve/ProcDump
  对**渲染进程**做多次快照；或者在运行时用调试器在 `WebView2` 的 `AddWebResourceRequestedFilter`/
  Tauri asset resolver 上下断点，直接取得明文字节。
- 若只需要“可构建”的前端占位，本工程 `ui/` 已提供与真实入口一致的 `index.html`（`ui/recovered/`）。

## 2026-10-31 续：前端资源已解出并接入 `ui/`
前端资源并非无法提取——它们以 **brotli 压缩**存放在 `.rdata` 的 Tauri 资源表中
（表位于 RVA 0x1a89cb0，4 个 `(key_ptr, key_len, data_ptr, data_len)` 条目）：

| key | 压缩长度 | 解压后 | 工程内路径 |
|---|---|---|---|
| `/index.html` | 256 B | 542 B | `ui/index.html` |
| `/assets/index-DknuZDVu.js` | 6,412,778 B | 21,409,388 B | `ui/assets/index-DknuZDVu.js` |
| `/assets/index-QyZyyXao.css` | 44,099 B | 288,881 B | `ui/assets/index-QyZyyXao.css` |
| `/assets/app-logo-BnL8-74-.png` | 5,047 B | 5,053 B | `ui/assets/app-logo-BnL8-74-.png` |

- 原始解压产物保存在 `../All-FlashTool-unpacked/embedded_assets/`；`ui/index.html` 已替换为真实入口，
  `ui/` 下占位 `index.js`/`style.css` 已移除。
- 本地静态服务验证：`index.html` 与三个 `/assets/*` 均返回 200，字节数与上表一致。
- 说明：JS 仍是 `_0x…` 混淆形态（39,885 个标识符），可运行但不可读；Tauri 侧 `main.rs` 仍为占位，
  因此当前只能作为“真实前端 + 可编译骨架”的组合。

