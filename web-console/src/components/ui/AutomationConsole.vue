<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import type { SchedulerEvent, SchedulerStatusResponse } from '@/types/api'

const props = defineProps<{
  autoMatchEnabled: boolean
  rentExtractionEnabled: boolean
}>()

const api = useApi()
const { t } = useI18n()

const expanded = ref(false)
const logs = ref<ConsoleLog[]>([])
const status = ref<SchedulerStatusResponse | null>(null)
const walletUnlocked = ref<boolean | null>(null)
const pollError = ref('')
const logEl = ref<HTMLElement | null>(null)

let pollTimer: ReturnType<typeof setInterval> | null = null
let lastEventId = 0
let prevWalletUnlocked: boolean | null = null
let connected = false

interface ConsoleLog {
  id: number
  ts: number
  level: 'info' | 'warn' | 'error'
  source: string
  message: string
}

let logId = 0
const MAX_LOGS = 300
const POLL_MS = 2000

const SOURCE_LABEL: Record<string, string> = {
  matcher: 'MATCH',
  extractor: 'RENT',
  system: 'SYS',
}

const hasError = computed(() =>
  Boolean(status.value?.matcher.last_error || status.value?.extractor.last_error),
)

const summary = computed(() => {
  const parts: string[] = []
  if (props.autoMatchEnabled) parts.push(t('settings.autoMatch'))
  if (props.rentExtractionEnabled) parts.push(t('settings.rentExtraction'))
  if (!parts.length) return t('settings.consoleAllDisabled')
  return parts.join(' · ')
})

function pushLog(
  level: ConsoleLog['level'],
  source: string,
  message: string,
  ts?: number,
) {
  logs.value.push({
    id: ++logId,
    ts: ts ?? Date.now(),
    level,
    source,
    message,
  })
  if (logs.value.length > MAX_LOGS) {
    logs.value.splice(0, logs.value.length - MAX_LOGS)
  }
  nextTick(() => {
    if (logEl.value) logEl.value.scrollTop = logEl.value.scrollHeight
  })
}

function formatTime(ts: number) {
  return new Date(ts).toLocaleTimeString(undefined, {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  })
}

function sourceLabel(source: string) {
  return SOURCE_LABEL[source] ?? source.toUpperCase()
}

function mapEventLevel(level: string): ConsoleLog['level'] {
  if (level === 'error') return 'error'
  if (level === 'warn') return 'warn'
  return 'info'
}

function ingestEvents(events: SchedulerEvent[]) {
  for (const evt of events) {
    pushLog(mapEventLevel(evt.level), evt.source, evt.message, evt.ts_ms)
    lastEventId = Math.max(lastEventId, evt.id)
  }
}

async function poll() {
  try {
    const [sched, session] = await Promise.all([
      api.getSchedulerStatus(lastEventId),
      api.getWalletSession().catch(() => ({ active: false })),
    ])
    pollError.value = ''
    status.value = sched

    if (!connected) {
      connected = true
      if (sched.events.length) {
        ingestEvents(sched.events)
      } else {
        pushLog('info', 'system', t('settings.consoleConnected'))
        if (!props.autoMatchEnabled && !props.rentExtractionEnabled) {
          pushLog('warn', 'system', t('settings.consoleAllDisabled'))
        }
      }
    } else {
      ingestEvents(sched.events)
    }
    lastEventId = sched.latest_event_id

    if (prevWalletUnlocked !== null && prevWalletUnlocked !== session.active) {
      pushLog(
        session.active ? 'info' : 'warn',
        'system',
        session.active
          ? t('settings.consoleWalletUnlocked')
          : t('settings.consoleWalletLocked'),
      )
    } else if (
      prevWalletUnlocked === null
      && !session.active
      && (props.autoMatchEnabled || props.rentExtractionEnabled)
    ) {
      pushLog('warn', 'system', t('settings.consoleWalletLocked'))
    }
    prevWalletUnlocked = session.active
    walletUnlocked.value = session.active
  } catch (e: any) {
    pollError.value = e.message || t('settings.consolePollFailed')
  }
}

function startPolling() {
  stopPolling()
  void poll()
  pollTimer = setInterval(() => void poll(), POLL_MS)
}

function stopPolling() {
  if (pollTimer) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

function clearLogs() {
  logs.value = []
}

watch(expanded, (open) => {
  if (open) startPolling()
  else stopPolling()
})

watch(
  () => [props.autoMatchEnabled, props.rentExtractionEnabled] as const,
  ([am, re], [prevAm, prevRe]) => {
    if (am !== prevAm) {
      pushLog(
        am ? 'info' : 'warn',
        'system',
        am ? t('settings.consoleAutoMatchEnabled') : t('settings.consoleAutoMatchDisabled'),
      )
    }
    if (re !== prevRe) {
      pushLog(
        re ? 'info' : 'warn',
        'system',
        re ? t('settings.consoleRentEnabled') : t('settings.consoleRentDisabled'),
      )
    }
  },
)

onUnmounted(stopPolling)
</script>

<template>
  <div
    class="automation-console"
    :class="{ 'automation-console--expanded': expanded }"
  >
    <button
      type="button"
      class="automation-console__header"
      :aria-expanded="expanded"
      @click="expanded = !expanded"
    >
      <div class="automation-console__title-wrap">
        <span
          class="automation-console__dot"
          :class="{
            'automation-console__dot--error': hasError,
            'automation-console__dot--live':
              (autoMatchEnabled || rentExtractionEnabled) && !hasError,
          }"
        />
        <span class="automation-console__title">{{ t('settings.consoleTitle') }}</span>
        <span class="automation-console__summary">{{ summary }}</span>
      </div>
      <span
        class="automation-console__chevron"
        :class="{ 'automation-console__chevron--open': expanded }"
        aria-hidden="true"
      >▾</span>
    </button>

    <div
      v-show="expanded"
      class="automation-console__body"
    >
      <div class="automation-console__stats">
        <div class="stat-chip">
          <span class="stat-chip__label">{{ t('settings.consoleTipBlock') }}</span>
          <span class="stat-chip__value">{{ status?.tip_block?.toLocaleString() ?? '—' }}</span>
        </div>
        <div class="stat-chip">
          <span class="stat-chip__label">{{ t('settings.autoMatch') }}</span>
          <span class="stat-chip__value">
            {{ autoMatchEnabled ? t('settings.enabledLabel') : t('settings.disabledLabel') }}
            · {{ status?.matcher.cycles?.toLocaleString() ?? 0 }} {{ t('settings.consoleCycles') }}
          </span>
        </div>
        <div class="stat-chip">
          <span class="stat-chip__label">{{ t('settings.rentExtraction') }}</span>
          <span class="stat-chip__value">
            {{ rentExtractionEnabled ? t('settings.enabledLabel') : t('settings.disabledLabel') }}
            · {{ status?.extractor.cycles?.toLocaleString() ?? 0 }} {{ t('settings.consoleCycles') }}
          </span>
        </div>
        <div class="stat-chip">
          <span class="stat-chip__label">{{ t('settings.consoleWallet') }}</span>
          <span
            class="stat-chip__value"
            :class="{ 'stat-chip__value--warn': walletUnlocked === false }"
          >
            {{
              walletUnlocked === null
                ? '—'
                : walletUnlocked
                  ? t('settings.consoleWalletUnlockedShort')
                  : t('settings.consoleWalletLockedShort')
            }}
          </span>
        </div>
        <button
          type="button"
          class="btn-clear"
          @click="clearLogs"
        >
          {{ t('settings.consoleClear') }}
        </button>
      </div>

      <div
        ref="logEl"
        class="automation-console__log"
        role="log"
        aria-live="polite"
      >
        <p
          v-if="pollError"
          class="log-line log-line--error"
        >
          <span class="log-line__time">{{ formatTime(Date.now()) }}</span>
          <span class="log-line__tag log-line__tag--sys">SYS</span>
          {{ pollError }}
        </p>
        <p
          v-if="!logs.length && !pollError"
          class="log-line log-line--muted"
        >
          {{ t('settings.consoleNoActivity') }}
        </p>
        <p
          v-for="entry in logs"
          :key="entry.id"
          class="log-line"
          :class="`log-line--${entry.level}`"
        >
          <span class="log-line__time">{{ formatTime(entry.ts) }}</span>
          <span
            class="log-line__tag"
            :class="`log-line__tag--${entry.source}`"
          >{{ sourceLabel(entry.source) }}</span>
          <span class="log-line__msg">{{ entry.message }}</span>
        </p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.automation-console {
  margin-top: var(--space-xl);
  border: 1px solid var(--border-light);
  border-radius: var(--radius-lg);
  background: var(--bg-card);
  box-shadow: var(--shadow-base);
  overflow: hidden;
}

.automation-console__header {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-md);
  padding: var(--space-sm) var(--space-md);
  border: none;
  background: var(--gray-50);
  cursor: pointer;
  font-family: inherit;
  text-align: left;
  transition: background var(--transition-base);
}

.automation-console__header:hover {
  background: var(--gray-100);
}

.automation-console__title-wrap {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  min-width: 0;
  flex: 1;
}

.automation-console__dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-disabled);
  flex-shrink: 0;
}

.automation-console__dot--live {
  background: var(--success);
  box-shadow: 0 0 0 2px rgba(82, 196, 26, 0.25);
}

.automation-console__dot--error {
  background: var(--danger);
  box-shadow: 0 0 0 2px rgba(255, 77, 79, 0.25);
}

.automation-console__title {
  font-size: var(--fs-body);
  font-weight: 600;
  color: var(--text-primary);
  flex-shrink: 0;
}

.automation-console__summary {
  font-size: var(--fs-caption);
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.automation-console__chevron {
  color: var(--text-muted);
  transition: transform var(--transition-base);
  flex-shrink: 0;
}

.automation-console__chevron--open {
  transform: rotate(180deg);
}

.automation-console__body {
  border-top: 1px solid var(--border-light);
}

.automation-console__stats {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-xs);
  padding: var(--space-sm) var(--space-md);
  border-bottom: 1px solid var(--border-light);
  align-items: center;
}

.stat-chip {
  display: inline-flex;
  align-items: center;
  gap: var(--space-xs);
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  background: var(--gray-50);
  border: 1px solid var(--border-light);
  font-size: var(--fs-caption);
}

.stat-chip__label {
  color: var(--text-muted);
}

.stat-chip__value {
  color: var(--text-primary);
  font-weight: 500;
}

.stat-chip__value--warn {
  color: var(--warning);
}

.btn-clear {
  margin-left: auto;
  border: 1px solid var(--border-dark);
  background: var(--bg-card);
  color: var(--text-secondary);
  border-radius: var(--radius-sm);
  padding: 2px 8px;
  font-size: var(--fs-caption);
  cursor: pointer;
  font-family: inherit;
}

.btn-clear:hover {
  color: var(--primary-500);
  border-color: var(--primary-500);
}

.automation-console__log {
  height: 300px;
  overflow-y: auto;
  padding: var(--space-sm) var(--space-md);
  background: #1a1a1a;
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.6;
}

.log-line {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  margin: 0 0 3px;
  color: #c8c8c8;
}

.log-line__time {
  color: #5a7a5a;
  flex-shrink: 0;
  min-width: 64px;
}

.log-line__tag {
  flex-shrink: 0;
  min-width: 44px;
  padding: 0 4px;
  border-radius: 3px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.02em;
  text-align: center;
  line-height: 18px;
  color: #bbb;
  background: #2a2a2a;
}

.log-line__tag--matcher {
  color: #79b8ff;
  background: rgba(24, 144, 255, 0.12);
}

.log-line__tag--extractor {
  color: #b392f0;
  background: rgba(114, 46, 209, 0.12);
}

.log-line__tag--system,
.log-line__tag--sys {
  color: #8c8c8c;
  background: #262626;
}

.log-line__msg {
  flex: 1;
  min-width: 0;
  word-break: break-word;
}

.log-line--warn .log-line__msg {
  color: #dcdcaa;
}

.log-line--error .log-line__msg {
  color: #f48771;
}

.log-line--muted {
  color: #666;
  font-style: italic;
}
</style>
