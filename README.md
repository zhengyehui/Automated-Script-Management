# Script Controller 桌面应用

Vite + Vue 3 前端 + Tauri 2 桌面壳，用于在本机管理、调度、启停 Python 自动化脚本。  
详细说明、常见问题与命令见 **[DOCUMENTATION.md](./DOCUMENTATION.md)**。

## 开发（浏览器访问）

1. 先启动后端：`cd d:\project && python -m script_controller.run`
2. 启动前端：`cd script_controller_app && npm run dev`
3. 打开 http://localhost:5173 ，前端会通过 Vite 代理将 `/api` 请求转发到后端（默认 127.0.0.1:8765）

若后端使用其他端口，在 `script_controller_app` 下创建 `.env.development`：

```
VITE_API_BASE=http://127.0.0.1:8766
```

并确保 Vite 代理目标与之一致（或直接让前端请求 VITE_API_BASE，不经过代理）。

## 构建

- `npm run build` — 产出到 `dist/`

## 桌面版（Tauri）

已集成 Tauri 2.x，可打包为桌面应用。

**环境**：Node 18+、**Rust**（安装见 https://www.rust-lang.org/tools/install ）、Python 3.9+（后端用）。

**开发**：

```bash
# 在 script_controller_app 目录
npm run tauri dev
```

会先启动 Vite 开发服务器，再打开桌面窗口；Tauri 会**自动检测 Python**、**启动后端**（`python -m script_controller.run`，cwd 为 `D:\project` 或环境变量 `SCRIPT_CONTROLLER_PROJECT_ROOT`），轮询得到 API 端口后注入前端。关闭窗口时会**自动结束后端进程**。若已有一个实例在运行，再次启动会直接退出（单实例锁文件在系统 temp 目录）。

**打包**：

```bash
npm run tauri build
```

产出在 `src-tauri/target/release/`，可分发 exe。

**项目根目录**：后端默认从 `D:\project` 启动。若项目不在该路径，可任选其一：设置环境变量 `SCRIPT_CONTROLLER_PROJECT_ROOT`；或在 **exe 同目录** 放置 `script_controller_root.txt`，内容为一行项目根路径（如 `E:\project`）。再运行 `tauri dev` 或打包后的 exe。

**桌面端「返回了非 JSON 数据」**：桌面应用请求会带 Origin（如 `tauri://localhost`），后端必须允许该来源，否则浏览器会拦截或后端返回非 JSON。若后端是 **FastAPI**，在创建 app 后添加：

```python
from fastapi.middleware.cors import CORSMiddleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["tauri://localhost", "http://localhost:5173", "http://127.0.0.1:5173"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)
```

**开发者工具**：桌面运行时点击顶栏 **「调试」** 或按 **F12** 可打开开发者工具，便于调试样式与 Network。

## 环境

- Node 18+
- 后端需已安装 Python 3.9+；桌面版由 Tauri 自动启动后端
- 桌面版构建需安装 Rust
