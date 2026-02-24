# Script Controller 桌面应用 — 项目说明与开发文档

本文档描述项目整体架构、常用命令，以及开发过程中遇到的问题与解决方法。

---

## 一、项目概述

### 1.1 是什么

**Script Controller 桌面应用** 是一个用于在本机管理、调度、启停 Python 自动化脚本的桌面程序。用户通过图形界面添加脚本、设置定时规则、手动运行/停止、查看日志与运行历史。

### 1.2 技术栈

| 层级 | 技术 |
|------|------|
| 前端 | Vue 3 + Vite |
| 桌面壳 | Tauri 2 |
| 后端 | 独立 Python 项目 `script_controller`（REST API，如 `/api/scripts`、`/api/health` 等） |

### 1.3 运行方式概览

- **浏览器开发**：先单独启动 Python 后端，再 `npm run dev`，前端通过 `http://127.0.0.1:8765`（或配置的端口）直连后端。
- **桌面应用（打包 exe）**：启动 exe 后，Tauri 先检测 Python、再在后台启动 `python -m script_controller.run`，轮询得到 API 端口（8765–8775），通过 Rust 的 `fetch_api` 代理前端请求，避免 CORS 和错误端口问题。关闭窗口时自动结束后端进程。

---

## 二、目录结构（简要）

```
script_controller_app/
├── src/                    # 前端源码
│   ├── App.vue             # 主界面与业务逻辑
│   ├── api.js              # 统一 API 客户端（含 Tauri 代理逻辑）
│   ├── style.css           # 全局样式与 CSS 变量
│   └── main.js
├── src-tauri/              # Tauri 后端
│   ├── src/lib.rs          # 启动流程、后端子进程、fetch_api、open_devtools 等
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── capabilities/
│       └── default.json    # 权限：core、dialog:allow-open
├── dist/                   # 前端构建产物（npm run build）
├── package.json
├── README.md
└── DOCUMENTATION.md        # 本文档
```

---

## 三、常用命令

### 3.1 前端（在项目根 `script_controller_app` 下）

| 命令 | 说明 |
|------|------|
| `npm install` | 安装前端依赖 |
| `npm run dev` | 启动 Vite 开发服务器（默认 http://localhost:5173） |
| `npm run build` | 构建前端到 `dist/` |
| `npm run preview` | 本地预览 `dist/` |

### 3.2 桌面应用（Tauri）

| 命令 | 说明 |
|------|------|
| `npm run tauri dev` | 开发模式：先起 Vite，再开桌面窗口，并自动启动 Python 后端 |
| `npm run tauri build` | 打包：构建前端 + 编译 Rust，产出 exe/msi/nsis 安装包 |

### 3.3 后端（独立 Python 项目）

若需单独调试后端（不通过桌面 exe 启动）：

```bash
# 在 script_controller 所在项目根目录
python -m script_controller.run
```

默认监听 8765–8775 中可用端口，健康检查：`GET /api/health`。

### 3.4 打包产物位置

- 可执行文件：`src-tauri/target/release/app.exe`
- MSI 安装包：`src-tauri/target/release/bundle/msi/`
- NSIS 安装包：`src-tauri/target/release/bundle/nsis/`

---

## 四、配置说明

### 4.1 后端项目根目录（script_controller 所在目录）

桌面应用启动后端时，需要知道「项目根目录」作为 `python -m script_controller.run` 的当前工作目录。优先级：

1. **环境变量** `SCRIPT_CONTROLLER_PROJECT_ROOT`（例如 `E:\project`）
2. **exe 同目录下的文件** `script_controller_root.txt`，内容为一行：项目根路径
3. **代码默认值**：Windows 为 `D:\project`，其他系统为 `/tmp/project`

若你的 `script_controller` 不在默认路径，请设置环境变量或在 exe 旁放置 `script_controller_root.txt`。

### 4.2 前端 API 基址（浏览器开发时）

- 浏览器访问时，若未设置 `window.__API_BASE__`（仅 Tauri 启动后注入），默认使用 `http://127.0.0.1:8765`。
- 若后端端口不同，可在项目下建 `.env.development`，例如：  
  `VITE_API_BASE=http://127.0.0.1:8766`

### 4.3 Tauri 权限（capabilities）

`src-tauri/capabilities/default.json` 中配置了：

- `core:default`：基础 Tauri 能力（invoke 等）
- `dialog:allow-open`：打开文件/目录选择对话框（脚本路径、工作目录的「⋯」按钮）

如需其他能力（如读写文件），需在此添加对应 permission。

---

## 五、开发过程中遇到的问题与解决方法

以下为开发与调试时遇到的典型问题及对应处理方式，便于后续维护或类似项目参考。

### 5.1 打包后请求后端报错（CORS / 非 JSON / 端口错误）

**现象**：桌面 exe 里前端请求后端 API 失败，提示 CORS、或「返回了非 JSON 数据」、或连到错误端口。

**原因**：  
- 打包后页面来源为 `tauri://localhost`，若前端直接用 `fetch` 请求 `http://127.0.0.1:8765`，可能遇到 CORS 或后端未允许该 Origin。  
- 后端端口在启动时动态选择（8765–8775），前端若写死端口或拿到错误端口会请求到 Vite 或别的服务，返回 HTML 等非 JSON。

**解决**：  
- 桌面端**不直接 fetch 后端**，改为通过 Tauri 的 Rust 命令 `fetch_api` 代理：前端 `invoke('fetch_api', { path, method, body })`，Rust 用 `reqwest` 请求本机 `http://127.0.0.1:{port}/...`，再把响应文本返回前端。  
- 前端在启动时先 `invoke('get_startup_status')` 拿到后端实际端口，设置 `window.__API_BASE__`；之后所有 API 请求在存在 `__API_BASE__` 时都走 `fetch_api`，否则走浏览器 fetch（开发时用）。  
- Rust 端 `fetch_api` 仅接受 8765–8775 端口，并校验响应为非 HTML、可解析为 JSON，避免误连到 5173 等。

### 5.2 桌面端「请在桌面应用中打开以使用文件/目录选择」

**现象**：在已打包的 exe 里点击脚本路径/工作目录旁的「⋯」按钮，仍提示「请在桌面应用中打开」。

**原因**：Tauri 2 默认**不会**设置 `window.__TAURI__`，只有开启 `withGlobalTauri` 时才有。前端用 `window.__TAURI__` 判断是否在桌面内，导致在 exe 里也被当成「非桌面」。

**解决**：  
- 增加对 `window.__TAURI_INTERNALS__` 的检测（Tauri 2 在桌面环境中会注入）。  
- 判断改为：`(window.__TAURI__ || '__TAURI_INTERNALS__' in window)`，这样在 exe 内也会正确识别为 Tauri 环境，文件/目录选择可正常使用。

### 5.3 启动后端时 CMD 窗口闪动

**现象**：在 Windows 下通过 Tauri 启动 `python -m script_controller.run` 或执行 `where python`、`tasklist`、`taskkill` 时，会短暂弹出 CMD 窗口。

**解决**：  
- 所有在 Windows 下 spawn 的进程（`Command`）加上：  
  `.creation_flags(0x0800_0000)`（即 `CREATE_NO_WINDOW`）。  
- 对 `where python`、`tasklist`、`taskkill`、以及启动后端的 `python -m script_controller.run` 均使用该 flag，实现静默、无窗口闪动。

### 5.4 单实例与多开

**需求**：只允许一个桌面应用实例，避免重复启动后端和端口冲突。

**实现**：  
- 使用锁文件（如系统 temp 目录下的 `script_controller_app.lock`），内容为当前进程 PID。  
- 启动时若锁文件存在且其中 PID 对应进程仍存在（Windows 用 `tasklist /FI "PID eq ..."`），则直接退出；否则写入当前 PID。  
- 窗口关闭时（`WindowEvent::Destroyed`）删除锁文件。

### 5.5 桌面端开发者工具（F12）不弹出

**现象**：在 exe 里按 F12 或点击「调试」希望打开开发者工具，无反应或报错。

**原因**：  
- 未实现 Rust 端 `open_devtools`，或 Tauri 未启用 devtools 能力。  
- Release 构建默认不包含 DevTools，需开启 Cargo feature。

**解决**：  
- 在 Rust 中实现 `open_devtools` 命令：通过 `app.get_webview_window("main")` 取得主窗口，调用 `window.open_devtools()`。  
- 在 `Cargo.toml` 中为 tauri 启用 feature：`features = ["devtools"]`，这样 release 打包的 exe 也能打开 DevTools。  
- 前端：F12 或顶栏「调试」按钮调用 `invoke('open_devtools')`。

### 5.6 选择脚本路径/工作目录（文件资源管理器）

**需求**：在添加/编辑脚本时，通过系统文件选择器选脚本路径和工作目录，而不是手输路径。

**实现**：  
- 使用 Tauri 官方插件：`tauri-plugin-dialog`（Rust 依赖 + 前端 `@tauri-apps/plugin-dialog`）。  
- 在 `capabilities/default.json` 中增加权限 `dialog:allow-open`。  
- 脚本路径：`open({ directory: false, filters: [{ name: 'Python 脚本', extensions: ['py'] }] })`，选中的文件路径回填到「脚本路径」。  
- 工作目录：`open({ directory: true })`，选中的目录路径回填到「工作目录」。  
- UI：在输入框右侧放「⋯」按钮（可做成框内按钮样式），仅在 Tauri 环境显示或始终显示、在非 Tauri 下点击时提示「请在桌面应用中打开」。

### 5.7 Capabilities 中不存在的 permission 导致启动报错

**现象**：在 `capabilities/default.json` 里写了未在 Tauri 中注册的 permission（例如自定义的 `allow-fetch-api`），应用启动或打包时报错。

**解决**：  
- capabilities 中的 `permissions` 只能填写 Tauri 及已用插件提供的 permission 标识符。  
- 本项目中只保留：`core:default`、`dialog:allow-open` 等已存在项，不要添加未在 schema/插件中声明的 permission。

### 5.8 后端启动超时或「后端未就绪」

**现象**：桌面启动后一直显示「正在启动后端…」或「后端未就绪」。

**可能原因**：  
- Python 未安装或未加入 PATH。  
- `script_controller` 未安装或项目根目录错误（环境变量/`script_controller_root.txt`）。  
- 端口 8765–8775 被占用或后端启动报错。

**排查**：  
- 确认本机可执行 `python -m script_controller.run`（在正确项目根下）。  
- 检查 8765–8775 端口是否被占用。  
- 若为打包 exe，确认同目录有 `script_controller_root.txt` 或已设置 `SCRIPT_CONTROLLER_PROJECT_ROOT`。  
- Rust 端在超时或失败时会尝试读取子进程 stderr，错误信息会通过 `get_startup_status` 的 `error` 字段返回给前端，可据此进一步排查。

---

## 六、功能速览（用户侧）

- **脚本列表**：展示已配置脚本、状态（空闲/运行中）、上次运行时间等。  
- **添加/编辑脚本**：名称、脚本路径、工作目录、时间规则（仅手动 / 每天 / 每周 / Cron）；脚本路径与工作目录支持「⋯」打开系统选择器（仅桌面端）。  
- **启停**：手动运行、停止；定时由「自动运行」总开关 + 各脚本规则控制。  
- **日志与历史**：查看某脚本的日志、运行历史。  
- **开发者工具**：桌面端顶栏「调试」或 F12 打开 DevTools，便于调试样式与网络。

---

## 七、环境要求小结

- **Node**：18+（前端与 Tauri CLI）  
- **Rust**：用于 Tauri 构建（安装见 https://www.rust-lang.org/tools/install ）  
- **Python**：3.9+，且需能运行 `script_controller`（桌面 exe 会自行启动后端）  
- **系统**：当前开发与打包以 Windows 为主；Rust 侧对 Linux/macOS 有条件编译（如 `where python` vs `which python3`、`taskkill` vs `kill`）

---

## 八、相关文件索引

| 文件 | 作用 |
|------|------|
| `src/App.vue` | 主界面、表单、列表、Tauri 检测与 open_devtools / 文件选择调用 |
| `src/api.js` | 请求封装、Tauri 下走 `fetch_api`、非 Tauri 下走 fetch |
| `src/style.css` | 全局样式与 CSS 变量 |
| `src-tauri/src/lib.rs` | 单实例、Python 检测、后端启动、端口轮询、fetch_api、open_devtools、窗口关闭杀后端 |
| `src-tauri/capabilities/default.json` | 桌面权限配置 |
| `src-tauri/tauri.conf.json` | 窗口、构建、打包配置 |

---

以上为项目整体说明、常用命令以及开发中常见问题与解决方案。后续若增加新功能或遇到新问题，可在此文档中继续补充「问题与解决」一节。
