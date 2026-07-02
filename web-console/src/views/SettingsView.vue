<script setup lang="ts">
import { ref, onMounted, inject } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import StatusTag from '@/components/ui/StatusTag.vue'
import type { RuntimeConfig, ServerInfo, SchedulerStatusResponse } from '@/types/api'

const api = useApi()
const { t } = useI18n()
const toast = inject<any>('toast')!

const activeTab = ref<'auto-match' | 'rent-extraction' | 'network'>('auto-match')

// Runtime Config (shared by auto-match and rent-extraction tabs)
const config = ref<RuntimeConfig | null>(null)
const configLoading = ref(true)
const editingMatch = ref(false)
const editingRent = ref(false)
const editForm = ref<RuntimeConfig>({
  fee_rate: 0,
  rent_extraction_enabled: true,
  scheduler_interval_secs: 0,
  min_extraction_amount_shannons: 0,
  auto_match_enabled: false,
  auto_match_min_capacity: 0,
  auto_match_max_escrow_blocks: 0,
  auto_match_interval_secs: 0,
})

async function loadConfig() {
  configLoading.value = true
  try { config.value = await api.getRuntimeConfig(); editForm.value = { ...config.value } }
  catch (e: any) { console.error('Failed to load runtime config:', e); toast.error(e.message || t('settings.configLoadFailed')) }
  finally { configLoading.value = false }
}

function startEditingMatch() { if (config.value) editForm.value = { ...config.value }; editingMatch.value = true }
function cancelEditingMatch() { editForm.value = { ...config.value! }; editingMatch.value = false }

function startEditingRent() { if (config.value) editForm.value = { ...config.value }; editingRent.value = true }
function cancelEditingRent() { editForm.value = { ...config.value! }; editingRent.value = false }

async function saveConfig() {
  try {
    const updated = await api.updateRuntimeConfig(editForm.value)
    toast.success(t('settings.configSaved'))
    config.value = updated
    editForm.value = { ...updated }
    editingMatch.value = false
    editingRent.value = false
  }
  catch (e: any) { console.error('Failed to save config:', e); toast.error(e.message || t('settings.configSaveFailed')) }
}

async function resetConfig() {
  try {
    const defaults = await api.resetRuntimeConfig()
    config.value = defaults
    if (editingMatch.value || editingRent.value) editForm.value = { ...defaults }
    toast.success(t('settings.configSaved'))
  }
  catch (e: any) { console.error('Failed to reset config:', e); toast.error(e.message || t('settings.configResetFailed')) }
}

// Scheduler status
const schedulerStatus = ref<SchedulerStatusResponse | null>(null)
const statusLoading = ref(false)

async function loadSchedulerStatus() {
  statusLoading.value = true
  try { schedulerStatus.value = await api.getSchedulerStatus() }
  catch (e) { console.warn('Failed to load scheduler status:', e) }
  finally { statusLoading.value = false }
}

// Network info
const serverInfo = ref<ServerInfo | null>(null)

async function loadServerInfo() {
  try { serverInfo.value = await api.getServerInfo() }
  catch (e) { console.warn('Failed to load server info:', e) }
}

onMounted(() => { loadConfig(); loadServerInfo(); loadSchedulerStatus() })
</script>

<template>
  <div class="page-settings">
    <h2 class="page-title">
      {{ t('settings.title') }}
    </h2>
    <div class="sub-tabs">
      <button
        class="sub-tab"
        :class="{ active: activeTab === 'auto-match' }"
        @click="activeTab = 'auto-match'"
      >
        {{ t('settings.autoMatch') }}
      </button>
      <button
        class="sub-tab"
        :class="{ active: activeTab === 'rent-extraction' }"
        @click="activeTab = 'rent-extraction'; loadSchedulerStatus()"
      >
        {{ t('settings.rentExtraction') }}
      </button>
      <button
        class="sub-tab"
        :class="{ active: activeTab === 'network' }"
        @click="activeTab = 'network'"
      >
        {{ t('settings.network') }}
      </button>
    </div>

    <!-- Auto-Match Tab -->
    <div
      v-if="activeTab === 'auto-match'"
      class="card config-card"
    >
      <div class="card-header">
        <h3>{{ t('settings.autoMatchConfig') }}</h3>
        <div class="card-header-actions">
          <button
            v-if="!editingMatch"
            class="btn btn-default btn-sm"
            @click="resetConfig"
          >
            {{ t('settings.reset') }}
          </button>
          <button
            v-if="!editingMatch"
            class="btn btn-default btn-sm"
            @click="startEditingMatch"
          >
            {{ t('settings.edit') }}
          </button>
        </div>
      </div>
      <div
        v-if="configLoading"
        class="text-muted"
      >
        {{ t('common.loading') }}
      </div>
      <template v-else-if="config">
        <div
          v-if="!editingMatch"
          class="config-display"
        >
          <div class="config-row">
            <span class="config-label">{{ t('settings.enabled') }}</span><StatusTag
              :status="config.auto_match_enabled ? 'live' : 'destroyed'"
              :label="config.auto_match_enabled ? t('settings.enabledLabel') : t('settings.disabledLabel')"
            />
          </div>
          <div class="config-row">
            <span class="config-label">{{ t('settings.minCapacity') }}</span><span>{{ (config.auto_match_min_capacity / 100_000_000).toFixed(0) }} {{ t('common.CKB') }}</span>
          </div>
          <div class="config-row">
            <span class="config-label">{{ t('settings.maxEscrow') }}</span><span>{{ config.auto_match_max_escrow_blocks.toLocaleString() }} {{ t('settings.blocks') }}</span>
          </div>
          <div class="config-row">
            <span class="config-label">{{ t('settings.interval') }}</span><span>{{ config.auto_match_interval_secs }} {{ t('settings.seconds') }}</span>
          </div>
        </div>
        <div
          v-else
          class="config-form"
        >
          <div class="form-group">
            <label class="form-label"><input
              v-model="editForm.auto_match_enabled"
              type="checkbox"
            > {{ t('settings.enableAutoMatch') }}</label>
          </div>
          <div class="form-group">
            <label class="form-label">{{ t('settings.minCapacity') }} (shannons)</label><input
              v-model.number="editForm.auto_match_min_capacity"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-group">
            <label class="form-label">{{ t('settings.maxEscrow') }}</label><input
              v-model.number="editForm.auto_match_max_escrow_blocks"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-group">
            <label class="form-label">{{ t('settings.interval') }} ({{ t('settings.seconds') }})</label><input
              v-model.number="editForm.auto_match_interval_secs"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-actions">
            <button
              class="btn btn-default"
              @click="cancelEditingMatch"
            >
              {{ t('settings.cancel') }}
            </button><button
              class="btn btn-primary"
              @click="saveConfig"
            >
              {{ t('settings.save') }}
            </button>
          </div>
        </div>
      </template>
    </div>

    <!-- Rent Extraction Tab -->
    <div
      v-if="activeTab === 'rent-extraction'"
      class="card config-card"
    >
      <div class="card-header">
        <h3>{{ t('settings.rentExtractionConfig') }}</h3>
        <div class="card-header-actions">
          <button
            v-if="!editingRent"
            class="btn btn-default btn-sm"
            @click="resetConfig"
          >
            {{ t('settings.reset') }}
          </button>
          <button
            v-if="!editingRent"
            class="btn btn-default btn-sm"
            @click="startEditingRent"
          >
            {{ t('settings.edit') }}
          </button>
        </div>
      </div>
      <div
        v-if="configLoading"
        class="text-muted"
      >
        {{ t('common.loading') }}
      </div>
      <template v-else-if="config">
        <!-- Display mode -->
        <div
          v-if="!editingRent"
          class="config-display"
        >
          <div class="config-row">
            <span class="config-label">{{ t('settings.enabled') }}</span>
            <StatusTag
              :status="config.rent_extraction_enabled ? 'live' : 'destroyed'"
              :label="config.rent_extraction_enabled ? t('settings.enabledLabel') : t('settings.disabledLabel')"
            />
          </div>
          <div class="config-row">
            <span class="config-label">{{ t('settings.feeRate') }}</span><span>{{ config.fee_rate.toLocaleString() }} shannons/KB</span>
          </div>
          <div class="config-row">
            <span class="config-label">{{ t('settings.schedulerInterval') }}</span><span>{{ config.scheduler_interval_secs }} {{ t('settings.seconds') }}</span>
          </div>
          <div class="config-row">
            <span class="config-label">{{ t('settings.minExtraction') }}</span><span>{{ (config.min_extraction_amount_shannons / 100_000_000).toFixed(2) }} {{ t('common.CKB') }}</span>
          </div>
        </div>
        <!-- Edit mode -->
        <div
          v-else
          class="config-form"
        >
          <div class="form-group">
            <label class="form-label">
              <input
                v-model="editForm.rent_extraction_enabled"
                type="checkbox"
              >
              {{ t('settings.enableRentExtraction') }}
            </label>
          </div>
          <div class="form-group">
            <label class="form-label">{{ t('settings.feeRate') }}</label><input
              v-model.number="editForm.fee_rate"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-group">
            <label class="form-label">{{ t('settings.schedulerInterval') }} ({{ t('settings.seconds') }})</label><input
              v-model.number="editForm.scheduler_interval_secs"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-group">
            <label class="form-label">{{ t('settings.minExtraction') }} (shannons)</label><input
              v-model.number="editForm.min_extraction_amount_shannons"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-actions">
            <button
              class="btn btn-default"
              @click="cancelEditingRent"
            >
              {{ t('settings.cancel') }}
            </button><button
              class="btn btn-primary"
              @click="saveConfig"
            >
              {{ t('settings.save') }}
            </button>
          </div>
        </div>

        <!-- Scheduler Status -->
        <div
          style="margin-top: var(--space-xl); border-top: 1px solid var(--border-light); padding-top: var(--space-lg);"
        >
          <div
            class="card-header"
            style="margin-bottom: var(--space-md);"
          >
            <h3>{{ t('settings.extractionStatus') }}</h3>
            <button
              class="btn btn-default btn-sm"
              :disabled="statusLoading"
              @click="loadSchedulerStatus"
            >
              {{ t('settings.refresh') }}
            </button>
          </div>
          <div
            v-if="statusLoading"
            class="text-muted"
          >
            {{ t('common.loading') }}
          </div>
          <div
            v-else-if="schedulerStatus"
            class="config-display"
          >
            <div class="config-row">
              <span class="config-label">{{ t('settings.lastRun') }}</span>
              <span>{{ schedulerStatus.extractor.last_run || t('settings.never') }}</span>
            </div>
            <div class="config-row">
              <span class="config-label">{{ t('settings.lastDuration') }}</span>
              <span>{{ schedulerStatus.extractor.last_duration_ms }} {{ t('settings.ms') }}</span>
            </div>
            <div class="config-row">
              <span class="config-label">{{ t('settings.totalCycles') }}</span>
              <span>{{ schedulerStatus.extractor.cycles }}</span>
            </div>
            <div class="config-row">
              <span class="config-label">{{ t('settings.totalExtracted') }}</span>
              <span>{{ schedulerStatus.extractor.total_processed }} shannons</span>
            </div>
            <div class="config-row">
              <span class="config-label">{{ t('settings.lastError') }}</span>
              <span :class="{ 'text-danger': schedulerStatus.extractor.last_error }">{{ schedulerStatus.extractor.last_error || t('common.none') }}</span>
            </div>
          </div>
        </div>
      </template>
    </div>

    <div
      v-if="activeTab === 'network'"
      class="card config-card"
    >
      <div class="card-header">
        <h3>{{ t('settings.networkInfo') }}</h3>
      </div>
      <div
        v-if="serverInfo"
        class="config-display"
      >
        <div class="config-row">
          <span class="config-label">{{ t('settings.network') }}</span>
          <StatusTag
            :status="serverInfo.network === 'mainnet' ? 'destroyed' : 'live'"
            :label="serverInfo.network === 'mainnet' ? 'Mainnet' : 'Testnet'"
          />
        </div>
        <div class="config-row">
          <span class="config-label">CKB RPC</span><code
            class="font-mono"
            style="font-size:var(--fs-caption)"
          >{{ serverInfo.ckb_rpc_url }}</code>
        </div>
        <div class="config-row">
          <span class="config-label">CKB Indexer</span><code
            class="font-mono"
            style="font-size:var(--fs-caption)"
          >{{ serverInfo.ckb_indexer_url }}</code>
        </div>
        <div class="config-row">
          <span class="config-label">Fiber RPC</span><code
            class="font-mono"
            style="font-size:var(--fs-caption)"
          >{{ serverInfo.fiber_rpc_url }}</code>
        </div>
        <div class="config-row">
          <span class="config-label">{{ t('settings.version') }}</span><span>{{ serverInfo.version }}</span>
        </div>
        <p
          class="form-hint text-muted"
          style="margin-top:var(--space-md)"
        >
          {{ t('settings.networkSwitchNote') }}
        </p>
      </div>
      <div
        v-else
        class="text-muted"
      >
        {{ t('common.loading') }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.page-settings { max-width: 900px; margin: 0 auto; }
.page-title { font-size: var(--fs-h2); font-weight: var(--fw-h2); line-height: var(--lh-h2); color: var(--text-primary); margin-bottom: var(--space-lg); }
.sub-tabs { display: flex; gap: var(--space-xs); margin-bottom: var(--space-xl); border-bottom: 1px solid var(--border-light); }
.sub-tab { padding: var(--space-xs) var(--space-md); border: none; border-bottom: 2px solid transparent; background: transparent; color: var(--text-secondary); font-size: var(--fs-body); cursor: pointer; transition: all var(--transition-base); margin-bottom: -1px; }
.sub-tab:hover { color: var(--primary-500); } .sub-tab.active { color: var(--primary-500); border-bottom-color: var(--primary-500); font-weight: 500; }
.card { background: var(--bg-card); border-radius: var(--radius-lg); border: 1px solid var(--border-light); box-shadow: var(--shadow-base); padding: var(--space-xl); }
.card-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-lg); } .card-header h3 { font-size: var(--fs-h3); font-weight: var(--fw-h3); }
.card-header-actions { display: flex; gap: var(--space-xs); }
.config-display { display: flex; flex-direction: column; gap: var(--space-md); }
.config-row { display: flex; justify-content: space-between; align-items: center; padding: var(--space-sm) 0; border-bottom: 1px solid var(--border-light); font-size: var(--fs-body); }
.config-row:last-child { border-bottom: none; }
.config-label { color: var(--text-secondary); }
.config-form { display: flex; flex-direction: column; gap: var(--space-md); }
.form-group { display: flex; flex-direction: column; gap: var(--space-xs); }
.form-label { font-size: var(--fs-body); color: var(--text-secondary); display: flex; align-items: center; gap: var(--space-xs); }
.form-input { height: 32px; padding: 0 var(--space-sm); border: 1px solid var(--border-dark); border-radius: var(--radius-md); font-size: var(--fs-body); color: var(--text-primary); background: var(--bg-card); outline: none; transition: border-color var(--transition-base), box-shadow var(--transition-base); }
.form-input:focus { border-color: var(--primary-500); box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2); }
.form-actions { display: flex; gap: var(--space-sm); justify-content: flex-end; margin-top: var(--space-sm); }
.form-hint { font-size: var(--fs-small); }
.btn { display: inline-flex; align-items: center; gap: var(--space-xs); padding: 0 var(--space-md); height: 32px; border: none; border-radius: var(--radius-md); font-size: var(--fs-body); font-family: inherit; cursor: pointer; transition: all var(--transition-base); font-weight: 500; }
.btn-primary { background: var(--primary-500); color: #fff; } .btn-primary:hover { background: var(--primary-400); }
.btn-default { background: var(--bg-card); color: var(--text-primary); border: 1px solid var(--border-dark); } .btn-default:hover { color: var(--primary-500); border-color: var(--primary-500); }
.btn-sm { height: 28px; font-size: var(--fs-caption); padding: 0 var(--space-sm); }
</style>
