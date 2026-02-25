/**
 * 统一 API 客户端：baseURL、指数退避重试（最多 3 次）
 * 在 Tauri 桌面中由 get_startup_status 设置 window.__API_BASE__
 * 打包后的 exe 使用 Rust 代理请求（fetch_api），避免 tauri://localhost 的 CORS 问题
 */
const DEFAULT_API_PORT = 8765

function getBase() {
  if (typeof window !== 'undefined' && window.__API_BASE__) return window.__API_BASE__
  if (typeof import.meta !== 'undefined' && import.meta.env?.VITE_API_BASE) return import.meta.env.VITE_API_BASE
  if (typeof window !== 'undefined' && (window.__TAURI__ || '__TAURI_INTERNALS__' in window)) {
    throw new Error('后端未连接，请确认 Python 后端已启动。')
  }
  if (typeof window !== 'undefined' && window.location?.hostname === 'localhost') {
    return `http://127.0.0.1:${DEFAULT_API_PORT}`
  }
  return ''
}

const delays = [1000, 2000, 4000]

/** 解析字符串为 JSON；非 JSON 时抛出明确错误并带上响应摘要 */
function parseJsonFromString(text) {
  try {
    return text ? JSON.parse(text) : null
  } catch (_) {
    const summary = typeof text === 'string' && text.length > 0
      ? ' 响应摘要：' + text.trim().slice(0, 200).replace(/\s+/g, ' ')
      : ' （无响应体）'
    const hint = typeof text === 'string' && text.includes('vite.svg')
      ? ' 当前可能请求到了前端页面(如 5173)而非后端，请用打包的 app.exe 运行，或先启动后端并配置 CORS。'
      : ''
    throw new Error(
      '后端返回了非 JSON 数据，请确认脚本控制器后端已正确启动（端口未被其他程序占用）。' + summary + hint
    )
  }
}

/** Tauri 下通过 Rust 代理请求，不走浏览器 fetch，无 CORS 问题 */
async function tauriRequest(path, method, body, retries = 3) {
  const { invoke } = await import('@tauri-apps/api/core')
  let lastErr
  for (let i = 0; i < retries; i++) {
    try {
      const pathNorm = path.startsWith('/') ? path : '/' + path
      const text = await invoke('fetch_api', {
        path: pathNorm,
        method: method || 'GET',
        body: body || null,
      })
      return parseJsonFromString(text)
    } catch (e) {
      lastErr = e
    }
    if (i < retries - 1) await new Promise(r => setTimeout(r, delays[i]))
  }
  throw lastErr instanceof Error ? lastErr : new Error(String(lastErr))
}

async function fetchWithRetry(url, options = {}, retries = 3) {
  const base = getBase()
  const fullUrl = base ? base.replace(/\/$/, '') + url : url
  let lastErr
  for (let i = 0; i < retries; i++) {
    try {
      const res = await fetch(fullUrl, { ...options })
      if (res.ok || res.status < 500) return res
      lastErr = new Error(`HTTP ${res.status}`)
    } catch (e) {
      lastErr = e
    }
    if (i < retries - 1) await new Promise(r => setTimeout(r, delays[i]))
  }
  throw lastErr
}

/** 解析 Response 为 JSON */
async function parseJson(res) {
  const text = await res.text()
  return parseJsonFromString(text)
}

/** 统一发起 API 请求：已通过 Tauri 拿到端口时走 Rust 代理，否则走 fetch */
async function request(path, options = {}) {
  const method = (options.method || 'GET').toUpperCase()
  const body = options.body
  if (typeof window !== 'undefined' && window.__API_BASE__) {
    return tauriRequest(path, method, body)
  }
  const r = await fetchWithRetry(path, options)
  return parseJson(r)
}

export async function getScripts() {
  return request('/api/scripts')
}

export async function getScript(id) {
  return request(`/api/scripts/${id}`)
}

export async function createScript(body) {
  return request('/api/scripts', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export async function updateScript(id, body) {
  return request(`/api/scripts/${id}`, {
    method: 'PUT',
    body: JSON.stringify(body),
  })
}

export async function deleteScript(id) {
  await request(`/api/scripts/${id}`, { method: 'DELETE' })
}

export async function startScript(id, body = {}) {
  return request(`/api/scripts/${id}/start`, {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export async function stopScript(id) {
  return request(`/api/scripts/${id}/stop`, { method: 'POST' })
}

export async function getScriptStatus(id) {
  return request(`/api/scripts/${id}/status`)
}

export async function getScriptLogs(id, lines = 500) {
  return request(`/api/scripts/${id}/logs?lines=${lines}`)
}

export async function getScriptRuns(id, limit = 50) {
  return request(`/api/scripts/${id}/runs?limit=${limit}`)
}

/** 调试：后端数据根、DB 路径、指定脚本在 DB 中的运行条数（用于排查无日志） */
export async function getDebugDataRoot(scriptId = null) {
  const q = scriptId ? `?script_id=${encodeURIComponent(scriptId)}` : ''
  return request(`/api/debug/data-root${q}`)
}

/** 清空所有运行记录与日志文件 */
export async function clearAllLogs() {
  return request('/api/debug/clear-all-logs', { method: 'POST' })
}

export async function getSchedulerStatus() {
  return request('/api/scheduler/status')
}

export async function schedulerStart() {
  return request('/api/scheduler/start', { method: 'POST' })
}

export async function schedulerStop() {
  return request('/api/scheduler/stop', { method: 'POST' })
}
