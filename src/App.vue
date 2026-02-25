<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import * as api from './api.js'

const scripts = ref([])
const schedulerRunning = ref(false)
const loading = ref(false)
const error = ref('')
const startupReady = ref(false)
const startupError = ref('')
const isTauri = typeof window !== 'undefined' && (window.__TAURI__ || '__TAURI_INTERNALS__' in window)
const formOpen = ref(false)
const formScript = ref(null)
const form = ref({ name: '', script_path: '', working_dir: '', args: [], schedule: null, scheduleType: 'none', scheduleTime: '08:00', scheduleWeekday: 0, timeout_seconds: null })
const logOpen = ref(false)
const logScriptId = ref('')
const logRuns = ref([])
/** 无日志时由 fetchLogRuns 填充，用于控制台与弹窗调试 */
const logDebugInfo = ref(null)
const runsOpen = ref(false)
const runsList = ref([])
const runsScriptId = ref('')
const settingsOpen = ref(false)
const closeToTray = ref(true)
let pollTimer = null

function scheduleToForm(schedule) {
  if (!schedule || !schedule.type) {
    form.value.scheduleType = 'none'
    form.value.scheduleTime = '08:00'
    form.value.scheduleWeekday = 0
    form.value.scheduleCron = '0 8 * * *'
    return
  }
  form.value.scheduleType = schedule.type
  form.value.scheduleTime = schedule.time || '08:00'
  form.value.scheduleWeekday = schedule.weekday ?? 0
  form.value.scheduleCron = schedule.expression || '0 8 * * *'
}

function formToSchedule() {
  if (form.value.scheduleType === 'none') return null
  if (form.value.scheduleType === 'cron') return { type: 'cron', expression: form.value.scheduleCron }
  if (form.value.scheduleType === 'daily') return { type: 'daily', time: form.value.scheduleTime }
  if (form.value.scheduleType === 'weekly') return { type: 'weekly', weekday: form.value.scheduleWeekday, time: form.value.scheduleTime }
  return null
}

async function loadScripts() {
  loading.value = true
  error.value = ''
  try {
    scripts.value = await api.getScripts()
  } catch (e) {
    error.value = e?.message || '加载失败'
  } finally {
    loading.value = false
  }
}

/** 静默刷新脚本列表（不显示加载中），用于启动/停止后只更新按钮状态 */
async function refreshScriptsSilent() {
  try {
    const list = await api.getScripts()
    scripts.value = list
  } catch (_) {}
}

async function loadScheduler() {
  try {
    const st = await api.getSchedulerStatus()
    schedulerRunning.value = st.running
  } catch (_) {}
}

function openAdd() {
  formScript.value = null
  form.value = { name: '', script_path: '', working_dir: '', args: [], scheduleType: 'none', scheduleTime: '08:00', scheduleWeekday: 0, scheduleCron: '0 8 * * *', timeout_seconds: null }
  formOpen.value = true
}

function openEdit(s) {
  formScript.value = s
  form.value = {
    name: s.name,
    script_path: s.script_path,
    working_dir: s.working_dir,
    args: s.args || [],
    scheduleType: (s.schedule && s.schedule.type) || 'none',
    scheduleTime: (s.schedule && s.schedule.time) || '08:00',
    scheduleWeekday: (s.schedule && s.schedule.weekday) ?? 0,
    scheduleCron: (s.schedule && s.schedule.expression) || '0 8 * * *',
    timeout_seconds: (s.timeout_seconds != null && s.timeout_seconds > 0) ? s.timeout_seconds : null,
  }
  formOpen.value = true
}

async function pickScriptPath() {
  if (!isTauri) {
    error.value = '请在桌面应用中打开以使用文件选择'
    return
  }
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Python 脚本', extensions: ['py'] }],
      title: '选择 Python 脚本',
    })
    if (path && typeof path === 'string') form.value.script_path = path
  } catch (e) {
    error.value = e?.message || '选择文件失败'
  }
}

async function pickWorkingDir() {
  if (!isTauri) {
    error.value = '请在桌面应用中打开以使用目录选择'
    return
  }
  try {
    const { open } = await import('@tauri-apps/plugin-dialog')
    const path = await open({
      multiple: false,
      directory: true,
      title: '选择工作目录',
    })
    if (path && typeof path === 'string') form.value.working_dir = path
  } catch (e) {
    error.value = e?.message || '选择目录失败'
  }
}

async function saveForm() {
  const body = {
    name: form.value.name,
    script_path: form.value.script_path,
    working_dir: form.value.working_dir,
    args: Array.isArray(form.value.args) ? form.value.args : [],
    schedule: formToSchedule(),
    timeout_seconds: (form.value.timeout_seconds != null && form.value.timeout_seconds > 0) ? form.value.timeout_seconds : null,
  }
  try {
    if (formScript.value) {
      await api.updateScript(formScript.value.id, body)
    } else {
      await api.createScript(body)
    }
    formOpen.value = false
    await loadScripts()
    await loadScheduler()
  } catch (e) {
    error.value = e?.message || '保存失败'
  }
}

async function doDelete(s) {
  if (!confirm('确定删除？')) return
  try {
    await api.deleteScript(s.id)
    await loadScripts()
  } catch (e) {
    error.value = e?.message || '删除失败'
  }
}

async function doStart(s) {
  try {
    await api.startScript(s.id)
    await refreshScriptsSilent()
  } catch (e) {
    error.value = e?.message || '启动失败'
  }
}

async function doStop(s) {
  try {
    await api.stopScript(s.id)
    await refreshScriptsSilent()
  } catch (e) {
    error.value = e?.message || '停止失败'
  }
}

async function fetchLogRuns(scriptId) {
  logDebugInfo.value = null
  const LOG_TAG = '[日志]'
  console.log(LOG_TAG, '请求 scriptId=', scriptId)
  try {
    const data = await api.getScriptLogs(scriptId)
    const runs = data?.runs ?? []
    console.log(LOG_TAG, '响应', {
      script_id: data?.script_id,
      runsCount: runs.length,
      runsSample: runs[0] ? { id: runs[0].id, started_at: runs[0].started_at, contentLen: (runs[0].content || '').length } : null,
    })
    if (runs.length === 0) {
      const debug = await api.getDebugDataRoot(scriptId).catch(() => ({}))
      logDebugInfo.value = debug
      console.warn(LOG_TAG, '无运行记录 — 调试信息', {
        后端数据根: debug?.data_root,
        数据库路径: debug?.db_path,
        该脚本在DB中的运行条数: debug?.runs_count,
        说明: debug?.reason,
      })
    }
    return runs
  } catch (e) {
    console.error(LOG_TAG, '请求失败', e)
    logDebugInfo.value = { error: e?.message || String(e) }
    return [{ id: 0, started_at: '', ended_at: '', exit_code: null, timeout: false, content: '获取失败: ' + (e?.message || '') }]
  }
}

async function openLogs(s) {
  logScriptId.value = s.id
  logOpen.value = true
  logRuns.value = []
  logRuns.value = await fetchLogRuns(s.id)
  if (s.running && logRuns.value.length === 0) {
    setTimeout(async () => {
      if (logScriptId.value === s.id && logOpen.value) logRuns.value = await fetchLogRuns(s.id)
    }, 800)
  }
}

async function refreshLogRuns() {
  if (!logScriptId.value) return
  logRuns.value = await fetchLogRuns(logScriptId.value)
}

function formatRunTime(r) {
  const start = r.started_at ? r.started_at.replace('T', ' ').slice(0, 19) : '—'
  const end = r.ended_at ? r.ended_at.replace('T', ' ').slice(0, 19) : '—'
  const code = r.exit_code != null ? r.exit_code : '—'
  const extra = r.timeout ? ' (超时)' : ''
  return `${start} → ${end}  退出码 ${code}${extra}`
}

async function openRuns(s) {
  runsScriptId.value = s.id
  runsOpen.value = true
  try {
    const data = await api.getScriptRuns(s.id)
    runsList.value = data.runs || []
  } catch (e) {
    runsList.value = []
  }
}

async function toggleScheduler() {
  try {
    if (schedulerRunning.value) await api.schedulerStop()
    else await api.schedulerStart()
    await loadScheduler()
    await loadScripts()
  } catch (e) {
    error.value = e?.message || '操作失败'
  }
}

function scheduleLabel(s) {
  if (!s.schedule || !s.schedule.type) return '仅手动'
  if (s.schedule.type === 'cron') return s.schedule.expression || 'Cron'
  if (s.schedule.type === 'daily') return `每天 ${s.schedule.time || ''}`
  if (s.schedule.type === 'weekly') {
    const w = ['一','二','三','四','五','六','日'][s.schedule.weekday ?? 0]
    return `周${w} ${s.schedule.time || ''}`
  }
  return '仅手动'
}

async function initStartup() {
  const tryTauri = async () => {
    const { invoke } = await import('@tauri-apps/api/core')
    return await invoke('get_startup_status')
  }
  let status = null
  try {
    status = await tryTauri()
  } catch (e) {
    await new Promise(r => setTimeout(r, 300))
    try {
      status = await tryTauri()
    } catch (e2) {
      if (typeof window !== 'undefined' && (window.__TAURI__ || '__TAURI_INTERNALS__' in window)) {
        startupError.value = e2?.message || '获取启动状态失败'
        startupReady.value = true
        return
      }
      startupReady.value = true
      await loadScripts()
      await loadScheduler()
      return
    }
  }
  const err = status?.error ?? (!status?.python_ok ? '未检测到 Python，请安装 Python 3.9+ 并加入 PATH。' : null)
  if (err) {
    startupError.value = err
    startupReady.value = true
    return
  }
  if (status?.api_port) {
    window.__API_BASE__ = `http://127.0.0.1:${status.api_port}`
  } else {
    startupError.value = startupError.value || '后端未就绪，请检查 Python 与脚本控制器（script_controller）环境。'
    startupReady.value = true
    return
  }
  startupReady.value = true
  await loadScripts()
  await loadScheduler()
}

function openDevtools() {
  if (!isTauri) return
  import('@tauri-apps/api/core').then(({ invoke }) => invoke('open_devtools')).catch(() => {})
}

function onKeydown(e) {
  if (e.key === 'F12') {
    e.preventDefault()
    openDevtools()
  }
}

async function loadCloseBehavior() {
  if (!isTauri) return
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    closeToTray.value = await invoke('get_close_behavior')
  } catch (_) {}
}

async function saveCloseBehavior(value) {
  if (!isTauri) return
  try {
    const { invoke } = await import('@tauri-apps/api/core')
    await invoke('set_close_behavior', { value })
    closeToTray.value = value
  } catch (_) {}
}

async function clearAllLogsConfirm() {
  if (!confirm('确定要清除所有历史运行记录与日志文件吗？此操作不可恢复。')) return
  try {
    const res = await api.clearAllLogs()
    logRuns.value = []
    logDebugInfo.value = null
    if (logOpen.value) logOpen.value = false
    alert(`已清除：${res.deleted_runs ?? 0} 条运行记录，${res.deleted_files ?? 0} 个日志文件。`)
  } catch (e) {
    alert('清除失败：' + (e?.message || ''))
  }
}

onMounted(() => {
  initStartup()
  loadCloseBehavior()
  pollTimer = setInterval(() => {
    if (scripts.value.some(s => s.running)) refreshScriptsSilent()
  }, 2000)
  document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible') loadScripts(); loadScheduler() })
  document.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  if (pollTimer) clearInterval(pollTimer)
  document.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <div class="app">
    <div v-if="!startupReady" class="startup-loading">
      <span class="spinner"></span>
      <p>正在启动后端…</p>
    </div>
    <div v-else-if="startupError" class="startup-error card-panel">
      <h2>启动失败</h2>
      <p class="error-msg">{{ startupError }}</p>
      <p class="hint">请确保已安装 Python 3.9+ 并加入 PATH；端口 8765–8775 未被占用。若 script_controller 不在默认目录，请在 app.exe 同目录放置 script_controller_root.txt，内容为项目根路径（一行）。</p>
      <p class="hint">若出现「返回了非 JSON 数据」：桌面端请求带 Origin（如 tauri://localhost），后端需配置 CORS 允许该来源。按 F12 或 Ctrl+Shift+I 可打开开发者工具。</p>
    </div>
    <template v-else>
    <header class="main-header">
      <h1>脚本控制</h1>
      <div class="header-right">
        <button v-if="isTauri" type="button" class="btn-settings" title="设置" @click="settingsOpen = true">设置</button>
        <button v-if="isTauri" type="button" class="btn-devtools" title="打开开发者工具 (F12)" @click="openDevtools">调试</button>
        <div class="scheduler">
          <span class="scheduler-label">自动运行</span>
          <div class="pill-group">
            <button type="button" class="pill" :class="{ on: schedulerRunning }" @click="!schedulerRunning && toggleScheduler()">已开启</button>
            <button type="button" class="pill" :class="{ on: !schedulerRunning }" @click="schedulerRunning && toggleScheduler()">已关闭</button>
          </div>
        </div>
      </div>
    </header>
    <div class="content">
      <p v-if="error" class="error-banner">{{ error }}</p>
      <div class="toolbar card-panel">
        <button class="btn-primary" @click="openAdd">添加脚本</button>
        <button class="btn-secondary" @click="loadScripts">刷新</button>
      </div>
      <div v-if="loading" class="loading-state">
        <span class="spinner"></span>
        <span>加载中…</span>
      </div>
      <div v-else-if="scripts.length === 0" class="empty-state">
        <p>暂无脚本，点击「添加脚本」创建</p>
      </div>
      <div v-else class="script-list">
        <article v-for="s in scripts" :key="s.id" class="script-card card-panel">
          <div class="script-card-header">
            <h3 class="script-card-name">{{ s.name }}</h3>
            <span class="status-badge" :class="{ running: s.running }">
              {{ s.running ? '运行中' : '已停止' }}
            </span>
          </div>
          <div class="script-card-meta">
            <span class="script-card-path" :title="s.script_path">{{ s.script_path }}</span>
            <span class="script-card-schedule">{{ scheduleLabel(s) }}</span>
            <span class="script-card-next">{{ s.next_run_time ? new Date(s.next_run_time).toLocaleString() : '—' }}</span>
          </div>
          <div class="script-card-actions">
            <button v-if="!s.running" class="btn-sm btn-start" @click="doStart(s)">启动</button>
            <button v-else class="btn-sm btn-stop" @click="doStop(s)">停止</button>
            <button class="btn-sm btn-ghost" @click="openLogs(s)">日志</button>
            <button class="btn-sm btn-ghost" @click="openRuns(s)">历史</button>
            <button class="btn-sm btn-ghost" @click="openEdit(s)">编辑</button>
            <button class="btn-sm btn-danger" @click="doDelete(s)">删除</button>
          </div>
        </article>
      </div>
      </div>
    <div v-if="formOpen" class="modal" @click.self="formOpen = false">
      <div class="modal-content modal-form">
        <h2>{{ formScript ? '编辑脚本' : '添加脚本' }}</h2>
        <div class="form">
          <label>名称 <input v-model="form.name" placeholder="显示名称" /></label>
          <label>
            脚本路径
            <div class="input-with-browse">
              <input v-model="form.script_path" placeholder="d:\path\to\script.py" />
              <button type="button" class="btn-browse" title="选择文件" @click="pickScriptPath">⋯</button>
            </div>
          </label>
          <label>
            工作目录
            <div class="input-with-browse">
              <input v-model="form.working_dir" placeholder="d:\path\to\dir" />
              <button type="button" class="btn-browse" title="选择目录" @click="pickWorkingDir">⋯</button>
            </div>
          </label>
          <label>时间规则
            <select v-model="form.scheduleType">
              <option value="none">仅手动</option>
              <option value="daily">每天</option>
              <option value="weekly">每周</option>
              <option value="cron">Cron</option>
            </select>
            <template v-if="form.scheduleType === 'daily'">
              <input v-model="form.scheduleTime" type="time" />
            </template>
            <template v-if="form.scheduleType === 'weekly'">
              <select v-model.number="form.scheduleWeekday">
                <option :value="0">周一</option>
                <option :value="1">周二</option>
                <option :value="2">周三</option>
                <option :value="3">周四</option>
                <option :value="4">周五</option>
                <option :value="5">周六</option>
                <option :value="6">周日</option>
              </select>
              <input v-model="form.scheduleTime" type="time" />
            </template>
            <template v-if="form.scheduleType === 'cron'">
              <input v-model="form.scheduleCron" placeholder="0 8 * * *" />
            </template>
          </label>
          <label>
            运行超时（秒）
            <input v-model.number="form.timeout_seconds" type="number" min="1" placeholder="不限制" />
            <span class="form-hint">超过后自动终止该次运行，避免脚本卡住</span>
          </label>
          <div class="form-actions">
            <button class="btn-primary" @click="saveForm">保存</button>
            <button class="btn-secondary" @click="formOpen = false">取消</button>
          </div>
        </div>
      </div>
    </div>
    <div v-if="logOpen" class="modal" @click.self="logOpen = false">
      <div class="modal-content modal-log">
        <div class="modal-log-header">
          <h2>日志</h2>
          <button type="button" class="btn-secondary btn-sm" @click="refreshLogRuns">刷新</button>
        </div>
        <div v-if="logRuns.length === 0" class="log-empty">
          <p>暂无运行记录。请先运行一次该脚本，或点击「刷新」。</p>
          <div v-if="logDebugInfo && !logDebugInfo.error" class="log-debug">
            <p class="log-debug-title">调试信息（便于排查无日志原因）</p>
            <dl class="log-debug-dl">
              <dt>后端数据根</dt><dd>{{ logDebugInfo.data_root || '—' }}</dd>
              <dt>数据库路径</dt><dd>{{ logDebugInfo.db_path || '—' }}</dd>
              <dt>该脚本在 DB 中的运行条数</dt><dd>{{ logDebugInfo.runs_count ?? '—' }}</dd>
              <dt>说明</dt><dd>{{ logDebugInfo.reason || '—' }}</dd>
            </dl>
            <p v-if="logDebugInfo.runs_count === 0" class="log-debug-hint">若运行条数为 0 且你已运行过该脚本，说明当前后端使用的数据库与写入运行记录时的数据库不是同一份（例如从 site-packages 加载的包用了另一套 data 目录）。请完全退出应用后重新打开，让后端从项目目录加载。</p>
          </div>
          <div v-else-if="logDebugInfo?.error" class="log-debug">请求失败: {{ logDebugInfo.error }}</div>
        </div>
        <div v-else class="log-runs">
          <details v-for="r in logRuns" :key="r.id" class="log-run-block">
            <summary class="log-run-summary">{{ formatRunTime(r) }}</summary>
            <pre class="log-run-content">{{ r.content && r.content.trim() ? r.content : '(该次运行无日志记录)' }}</pre>
          </details>
        </div>
        <button class="btn-secondary" @click="logOpen = false">关闭</button>
      </div>
    </div>
    <div v-if="runsOpen" class="modal" @click.self="runsOpen = false">
      <div class="modal-content">
        <h2>运行历史</h2>
        <ul class="runs-list">
          <li v-for="r in runsList" :key="r.id">
            {{ r.started_at }} → {{ r.ended_at || '—' }} 退出码 {{ r.exit_code }} {{ r.timeout ? '(超时)' : '' }}
          </li>
        </ul>
        <button class="btn-secondary" @click="runsOpen = false">关闭</button>
      </div>
    </div>
    <div v-if="settingsOpen" class="modal" @click.self="settingsOpen = false">
      <div class="modal-content modal-settings">
        <h2>设置</h2>
        <div class="settings-body">
          <p class="settings-label">关闭窗口时</p>
          <div class="settings-pill-group">
            <label class="settings-pill" :class="{ on: closeToTray }">
              <input v-model="closeToTray" type="radio" :value="true" @change="saveCloseBehavior(true)" />
              <span>最小化到托盘</span>
            </label>
            <label class="settings-pill" :class="{ on: !closeToTray }">
              <input v-model="closeToTray" type="radio" :value="false" @change="saveCloseBehavior(false)" />
              <span>退出程序</span>
            </label>
          </div>
        </div>
        <p class="settings-label">数据</p>
        <div class="settings-body">
          <button type="button" class="btn-secondary btn-danger" @click="clearAllLogsConfirm">清除所有历史日志与运行记录</button>
          <span class="form-hint">将清空数据库中的运行记录并删除所有日志文件，不可恢复。</span>
        </div>
        <div class="form-actions">
          <button class="btn-secondary" @click="settingsOpen = false">关闭</button>
        </div>
      </div>
    </div>
    </template>
  </div>
</template>

<style scoped>
.app { max-width: 1100px; margin: 0 auto; min-height: 100vh; }

.main-header {
  background: var(--bg-card);
  color: var(--text-primary);
  padding: 1rem 1.5rem;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--border);
  box-shadow: var(--shadow-sm);
}
.main-header h1 {
  margin: 0;
  font-size: 1.35rem;
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--text-primary);
}
.header-right { display: flex; align-items: center; gap: 1rem; }
.btn-settings,
.btn-devtools {
  padding: 0.4rem 0.75rem;
  font-size: 0.85rem;
  background: transparent;
  color: var(--text-secondary);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
.btn-settings:hover,
.btn-devtools:hover { background: var(--bg-page); color: var(--text-primary); }
.scheduler { display: flex; align-items: center; gap: 0.75rem; }
.scheduler-label { font-size: 0.9rem; font-weight: 500; color: var(--text-secondary); }
.pill-group { display: flex; gap: 0.25rem; }
.pill {
  padding: 0.35rem 0.9rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg-page);
  color: var(--text-secondary);
  font-size: 0.85rem;
  font-weight: 500;
}
.pill:hover { background: #edf2f7; color: var(--text-primary); }
.pill.on {
  background: var(--accent);
  color: #fff;
  border-color: var(--accent);
}
.pill.on:hover { background: var(--accent-hover); border-color: var(--accent-hover); }

.content { padding: 1.5rem 2rem; }
.error-banner {
  background: var(--danger-bg);
  color: var(--danger);
  padding: 0.75rem 1rem;
  border-radius: var(--radius);
  margin-bottom: 1rem;
  font-size: 0.9rem;
}
.card-panel {
  background: var(--bg-card);
  border-radius: var(--radius);
  box-shadow: var(--shadow-sm);
  border: 1px solid var(--border);
}
.toolbar.card-panel {
  padding: 1rem 1.25rem;
  margin-bottom: 1.25rem;
  display: flex;
  gap: 0.75rem;
}
.btn-primary {
  background: var(--accent);
  color: #fff;
  border: none;
  padding: 0.6rem 1.25rem;
  border-radius: var(--radius);
  font-weight: 500;
}
.btn-primary:hover { background: var(--accent-hover); box-shadow: var(--shadow); }
.btn-secondary {
  background: var(--bg-card);
  color: var(--text-primary);
  border: 1px solid var(--border);
  border-radius: var(--radius);
}
.btn-secondary:hover { background: var(--bg-page); }
.btn-danger { color: #c53030; border-color: #c53030; }
.btn-danger:hover { background: #fef2f2; }
.loading-state {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 2rem;
  color: var(--text-secondary);
  font-size: 0.95rem;
}
.empty-state {
  padding: 2.5rem;
  text-align: center;
  color: var(--text-secondary);
  font-size: 0.95rem;
  background: var(--bg-card);
  border-radius: var(--radius);
  border: 1px solid var(--border);
}
.spinner {
  width: 18px;
  height: 18px;
  border: 2px solid var(--border);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

.script-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}
.script-card {
  padding: 1.25rem 1.5rem;
  transition: box-shadow 0.15s;
}
.script-card:hover {
  box-shadow: var(--shadow);
}
.script-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.5rem;
}
.script-card-name {
  margin: 0;
  font-size: 1.05rem;
  font-weight: 600;
  color: var(--text-primary);
}
.script-card-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem 1rem;
  margin-bottom: 0.75rem;
  font-size: 0.875rem;
  color: var(--text-secondary);
}
.script-card-path {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1 1 280px;
  min-width: 0;
}
.script-card-schedule { flex-shrink: 0; }
.script-card-next { flex-shrink: 0; }
.script-card-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}
.status-badge {
  display: inline-block;
  padding: 0.25rem 0.6rem;
  border-radius: 999px;
  font-size: 0.78rem;
  font-weight: 500;
  background: #edf2f7;
  color: var(--text-secondary);
}
.status-badge.running {
  background: var(--accent);
  color: #fff;
}
.btn-sm {
  padding: 0.35rem 0.7rem;
  font-size: 0.8rem;
  border-radius: var(--radius-sm);
}
.btn-start { background: var(--accent); color: #fff; border: none; }
.btn-start:hover { background: var(--accent-hover); filter: brightness(1.02); }
.btn-stop { background: #c53030; color: #fff; border: none; }
.btn-stop:hover { filter: brightness(1.1); }
.btn-ghost { background: transparent; color: var(--text-secondary); border: 1px solid var(--border); }
.btn-ghost:hover { background: var(--bg-page); color: var(--text-primary); }
.btn-danger { background: transparent; color: var(--danger); border: 1px solid #fecaca; }
.btn-danger:hover { background: var(--danger-bg); }

.modal {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  backdrop-filter: blur(2px);
}
.modal-content {
  background: var(--bg-card);
  padding: 1.5rem;
  border-radius: var(--radius);
  max-width: 480px;
  width: 90%;
  max-height: 90vh;
  overflow: auto;
  box-shadow: var(--shadow-lg);
}
.modal-content h2 { margin: 0 0 1rem; font-size: 1.15rem; color: var(--text-primary); }
.modal-form .form label { display: block; margin-bottom: 0.9rem; font-size: 0.9rem; }
.modal-form .form label input,
.modal-form .form label select {
  display: block;
  margin-top: 0.35rem;
  padding: 0.5rem 0.65rem;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  width: 100%;
  max-width: none;
}
.input-with-browse {
  position: relative;
  margin-top: 0.35rem;
  display: flex;
  align-items: stretch;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--bg-card);
}
.input-with-browse:focus-within {
  border-color: var(--accent);
  box-shadow: 0 0 0 2px rgba(232, 93, 4, 0.2);
}
/* 仅外层描边，内层 input 不再单独描边，避免双线 */
.modal-form .form .input-with-browse input {
  flex: 1;
  margin-top: 0;
  min-width: 0;
  padding: 0.5rem 2.5rem 0.5rem 0.65rem;
  border: none;
  background: transparent;
  border-radius: var(--radius-sm);
  box-shadow: none;
}
.modal-form .form .input-with-browse input:focus {
  outline: none;
  box-shadow: none;
}
.btn-browse {
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 2.25rem;
  padding: 0;
  font-size: 1.1rem;
  line-height: 1;
  letter-spacing: 0.02em;
  background: transparent;
  color: var(--text-secondary);
  border: none;
  border-left: 1px solid var(--border);
  border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
}
.btn-browse:hover {
  background: var(--border);
  color: var(--text-primary);
}
.modal-form .form label input:focus,
.modal-form .form label select:focus {
  outline: none;
  border-color: var(--accent);
  box-shadow: 0 0 0 2px rgba(232, 93, 4, 0.2);
}
.modal-form .form .form-hint { display: block; margin-top: 0.35rem; font-size: 0.8rem; color: var(--text-secondary); }
.form-actions { margin-top: 1.25rem; display: flex; gap: 0.5rem; }
.modal-settings .settings-body { margin-bottom: 1.25rem; }
.modal-settings .settings-label {
  display: block;
  margin: 0 0 0.75rem;
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--text-primary);
}
.modal-settings .settings-pill-group {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}
.modal-settings .settings-pill {
  display: inline-flex;
  align-items: center;
  padding: 0.5rem 1rem;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg-page);
  color: var(--text-secondary);
  font-size: 0.9rem;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.15s, color 0.15s, border-color 0.15s;
}
.modal-settings .settings-pill:hover { background: #edf2f7; color: var(--text-primary); }
.modal-settings .settings-pill.on {
  background: var(--accent);
  color: #fff;
  border-color: var(--accent);
}
.modal-settings .settings-pill input[type="radio"] {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}
.modal-settings .settings-pill span { user-select: none; }
.modal-log { max-width: 720px; }
.modal-log-header { display: flex; align-items: center; justify-content: space-between; gap: 0.75rem; margin-bottom: 0.75rem; }
.modal-log-header h2 { margin: 0; }
.btn-sm { padding: 0.35rem 0.65rem; font-size: 0.85rem; }
.log-empty { color: var(--text-secondary); padding: 1.5rem 0; font-size: 0.9rem; }
.log-debug { margin-top: 1rem; padding: 0.75rem; background: var(--bg-secondary); border: 1px solid var(--border); border-radius: var(--radius-sm); font-size: 0.8rem; text-align: left; }
.log-debug-title { margin: 0 0 0.5rem; font-weight: 600; color: var(--text); }
.log-debug-dl { margin: 0; }
.log-debug-dl dt { margin-top: 0.35rem; color: var(--text-secondary); }
.log-debug-dl dd { margin: 0 0 0 1rem; word-break: break-all; }
.log-debug-hint { margin: 0.5rem 0 0; padding-top: 0.5rem; border-top: 1px solid var(--border); color: var(--accent); font-size: 0.85rem; }
.log-runs { margin-bottom: 1rem; max-height: 70vh; overflow: auto; }
.log-run-block { margin-bottom: 0.5rem; border: 1px solid var(--border); border-radius: var(--radius-sm); overflow: hidden; }
.log-run-block summary {
  padding: 0.5rem 0.75rem;
  font-size: 0.85rem;
  color: var(--text-secondary);
  background: var(--bg-secondary);
  cursor: pointer;
  list-style: none;
  user-select: none;
}
.log-run-block summary::-webkit-details-marker { display: none; }
.log-run-block[open] summary { border-bottom: 1px solid var(--border); }
.log-run-content {
  white-space: pre-wrap;
  font-size: 0.8rem;
  max-height: 320px;
  overflow: auto;
  background: #f8fafc;
  padding: 1rem;
  margin: 0;
  border-radius: 0;
  border: none;
}
.modal-log pre {
  white-space: pre-wrap;
  font-size: 0.8rem;
  max-height: 380px;
  overflow: auto;
  background: #f8fafc;
  padding: 1rem;
  border-radius: var(--radius-sm);
  border: 1px solid var(--border);
  margin-bottom: 1rem;
}
.runs-list { list-style: none; padding: 0; margin: 0 0 1rem; max-height: 280px; overflow: auto; }
.runs-list li {
  padding: 0.5rem 0;
  border-bottom: 1px solid var(--border);
  font-size: 0.88rem;
  color: var(--text-secondary);
}

.startup-loading {
  padding: 4rem 2rem;
  text-align: center;
  color: var(--text-secondary);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}
.startup-loading .spinner { margin: 0; }
.startup-error.card-panel {
  padding: 1.75rem;
  max-width: 520px;
  margin: 2rem auto;
  background: var(--danger-bg);
  border: 1px solid #fecaca;
  border-radius: var(--radius);
}
.startup-error h2 { margin: 0 0 0.5rem; color: var(--danger); font-size: 1.2rem; }
.startup-error .error-msg { color: #991b1b; margin: 0; }
.startup-error .hint { margin-top: 1rem; font-size: 0.85rem; color: var(--text-secondary); line-height: 1.5; }
</style>
