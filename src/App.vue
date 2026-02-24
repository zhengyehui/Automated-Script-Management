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
const form = ref({ name: '', script_path: '', working_dir: '', args: [], schedule: null, scheduleType: 'none', scheduleTime: '08:00', scheduleWeekday: 0 })
const logOpen = ref(false)
const logScriptId = ref('')
const logContent = ref('')
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

async function loadScheduler() {
  try {
    const st = await api.getSchedulerStatus()
    schedulerRunning.value = st.running
  } catch (_) {}
}

function openAdd() {
  formScript.value = null
  form.value = { name: '', script_path: '', working_dir: '', args: [], scheduleType: 'none', scheduleTime: '08:00', scheduleWeekday: 0, scheduleCron: '0 8 * * *' }
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
    await loadScripts()
  } catch (e) {
    error.value = e?.message || '启动失败'
  }
}

async function doStop(s) {
  try {
    await api.stopScript(s.id)
    await loadScripts()
  } catch (e) {
    error.value = e?.message || '停止失败'
  }
}

async function openLogs(s) {
  logScriptId.value = s.id
  logOpen.value = true
  try {
    const data = await api.getScriptLogs(s.id)
    logContent.value = data.content || ''
  } catch (e) {
    logContent.value = '获取失败: ' + (e?.message || '')
  }
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

onMounted(() => {
  initStartup()
  loadCloseBehavior()
  pollTimer = setInterval(() => {
    if (scripts.value.some(s => s.running)) loadScripts()
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
          <button class="toggle-btn" :class="{ on: schedulerRunning }" @click="toggleScheduler">
            {{ schedulerRunning ? '已开启' : '已关闭' }}
          </button>
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
      <div v-else class="table-wrap card-panel">
        <table class="script-table">
          <thead>
            <tr>
              <th>名称</th>
              <th>路径</th>
              <th>时间规则</th>
              <th>下次运行</th>
              <th>状态</th>
              <th>操作</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="s in scripts" :key="s.id">
              <td class="name-cell">{{ s.name }}</td>
              <td class="path-cell">{{ s.script_path }}</td>
              <td>{{ scheduleLabel(s) }}</td>
              <td class="time-cell">{{ s.next_run_time ? new Date(s.next_run_time).toLocaleString() : '—' }}</td>
              <td>
                <span class="status-badge" :class="{ running: s.running }">
                  {{ s.running ? '运行中' : '已停止' }}
                </span>
              </td>
              <td class="actions-cell">
                <button v-if="!s.running" class="btn-sm btn-start" @click="doStart(s)">启动</button>
                <button v-else class="btn-sm btn-stop" @click="doStop(s)">停止</button>
                <button class="btn-sm btn-ghost" @click="openLogs(s)">日志</button>
                <button class="btn-sm btn-ghost" @click="openRuns(s)">历史</button>
                <button class="btn-sm btn-ghost" @click="openEdit(s)">编辑</button>
                <button class="btn-sm btn-danger" @click="doDelete(s)">删除</button>
              </td>
            </tr>
          </tbody>
        </table>
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
          <div class="form-actions">
            <button class="btn-primary" @click="saveForm">保存</button>
            <button class="btn-secondary" @click="formOpen = false">取消</button>
          </div>
        </div>
      </div>
    </div>
    <div v-if="logOpen" class="modal" @click.self="logOpen = false">
      <div class="modal-content modal-log">
        <h2>日志</h2>
        <pre>{{ logContent }}</pre>
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
          <div class="radio-group">
            <label class="radio-item">
              <input v-model="closeToTray" type="radio" :value="true" @change="saveCloseBehavior(true)" />
              <span>最小化到托盘</span>
            </label>
            <label class="radio-item">
              <input v-model="closeToTray" type="radio" :value="false" @change="saveCloseBehavior(false)" />
              <span>退出程序</span>
            </label>
          </div>
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
  background: var(--bg-header);
  color: #fff;
  padding: 1rem 1.5rem;
  display: flex;
  justify-content: space-between;
  align-items: center;
  box-shadow: var(--shadow);
}
.main-header h1 {
  margin: 0;
  font-size: 1.35rem;
  font-weight: 600;
  letter-spacing: 0.02em;
}
.header-right { display: flex; align-items: center; gap: 1rem; }
.btn-settings,
.btn-devtools {
  padding: 0.3rem 0.6rem;
  font-size: 0.8rem;
  background: rgba(255,255,255,0.2);
  color: rgba(255,255,255,0.9);
  border: 1px solid rgba(255,255,255,0.4);
  border-radius: var(--radius-sm);
}
.btn-settings:hover,
.btn-devtools:hover { background: rgba(255,255,255,0.35); }
.scheduler { display: flex; align-items: center; gap: 0.6rem; }
.scheduler-label { font-size: 0.9rem; opacity: 0.95; }
.toggle-btn {
  padding: 0.35rem 0.9rem;
  border-radius: 999px;
  border: 1px solid rgba(255,255,255,0.4);
  background: rgba(255,255,255,0.15);
  color: #fff;
  font-size: 0.85rem;
}
.toggle-btn:hover { background: rgba(255,255,255,0.25); }
.toggle-btn.on {
  background: rgba(255,255,255,0.95);
  color: #1e3a5f;
  border-color: transparent;
}

.content { padding: 1.25rem 1.5rem; }
.error-banner {
  background: var(--danger-bg);
  color: var(--danger);
  padding: 0.6rem 1rem;
  border-radius: var(--radius-sm);
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
  padding: 0.75rem 1rem;
  margin-bottom: 1rem;
  display: flex;
  gap: 0.5rem;
}
.btn-primary {
  background: var(--accent);
  color: #fff;
  border: none;
  padding: 0.5rem 1rem;
}
.btn-primary:hover { background: var(--accent-hover); box-shadow: var(--shadow); }
.btn-secondary {
  background: var(--bg-page);
  color: var(--text-primary);
  border: 1px solid var(--border);
}
.btn-secondary:hover { background: var(--border); }
.loading-state {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 1.5rem;
  color: var(--text-secondary);
  font-size: 0.95rem;
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

.table-wrap.card-panel {
  overflow: hidden;
  padding: 0;
}
.script-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 0.9rem;
}
.script-table th {
  text-align: left;
  padding: 0.75rem 1rem;
  background: #f8fafc;
  color: var(--text-secondary);
  font-weight: 600;
  font-size: 0.8rem;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  border-bottom: 1px solid var(--border);
}
.script-table td {
  padding: 0.65rem 1rem;
  border-bottom: 1px solid var(--border);
  vertical-align: middle;
}
.script-table tbody tr:hover { background: #fafbfc; }
.script-table tbody tr:last-child td { border-bottom: none; }
.name-cell { font-weight: 500; color: var(--text-primary); }
.path-cell {
  font-size: 0.82rem;
  color: var(--text-secondary);
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.time-cell { color: var(--text-secondary); font-size: 0.88rem; }
.status-badge {
  display: inline-block;
  padding: 0.2rem 0.55rem;
  border-radius: 999px;
  font-size: 0.78rem;
  font-weight: 500;
  background: #f1f5f9;
  color: var(--text-secondary);
}
.status-badge.running {
  background: var(--success-bg);
  color: var(--success);
}
.actions-cell { white-space: nowrap; }
.actions-cell .btn-sm { margin-right: 0.35rem; margin-bottom: 0.15rem; }
.btn-sm {
  padding: 0.3rem 0.6rem;
  font-size: 0.8rem;
}
.btn-start { background: var(--success); color: #fff; border: none; }
.btn-start:hover { filter: brightness(1.08); }
.btn-stop { background: #b91c1c; color: #fff; border: none; }
.btn-stop:hover { filter: brightness(1.1); }
.btn-ghost { background: transparent; color: var(--text-secondary); border: 1px solid var(--border); }
.btn-ghost:hover { background: #f1f5f9; color: var(--text-primary); }
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
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.15);
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
  box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.15);
}
.form-actions { margin-top: 1.25rem; display: flex; gap: 0.5rem; }
.modal-settings .settings-body { margin-bottom: 1.25rem; }
.modal-settings .settings-label {
  display: block;
  margin: 0 0 0.6rem;
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--text-primary);
}
.modal-settings .radio-group {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}
.modal-settings .radio-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  font-weight: normal;
  font-size: 0.95rem;
  color: var(--text-primary);
  min-height: 1.5rem;
}
.modal-settings .radio-item input[type="radio"] {
  margin: 0;
  width: 1rem;
  height: 1rem;
  min-width: 1rem;
  min-height: 1rem;
  flex-shrink: 0;
  accent-color: var(--accent);
  cursor: pointer;
}
.modal-settings .radio-item span { user-select: none; }
.modal-log { max-width: 640px; }
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
}
.startup-error h2 { margin: 0 0 0.5rem; color: var(--danger); font-size: 1.2rem; }
.startup-error .error-msg { color: #991b1b; margin: 0; }
.startup-error .hint { margin-top: 1rem; font-size: 0.85rem; color: var(--text-secondary); line-height: 1.5; }
</style>
