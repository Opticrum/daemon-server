<script setup lang="ts">
import { ref, onMounted, inject } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress } from '@/utils/format'
import DataTable, { type ColumnDef } from '@/components/ui/DataTable.vue'
import StatusTag from '@/components/ui/StatusTag.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import WitnessSubmitModal from '@/components/ui/WitnessSubmitModal.vue'
import type { AutoMatchConfig, UnsignedTx, ServerInfo } from '@/types/api'

const api = useApi()
const { t } = useI18n()
const toast = inject<any>('toast')!
const modal = inject<any>('modal')!

const activeTab = ref<'auto-match' | 'signing' | 'network'>('auto-match')

// Auto-Match Config
const config = ref<AutoMatchConfig | null>(null)
const configLoading = ref(true)
const editing = ref(false)
const editForm = ref<AutoMatchConfig>({ enabled: false, min_capacity_shannons: 0, max_escrow_blocks: 0, interval_secs: 0 })

async function loadConfig() {
  configLoading.value = true
  try { config.value = await api.getAutoMatchConfig(); editForm.value = { ...config.value } }
  catch (e: any) { toast.error(e.message || t('settings.configLoadFailed')) }
  finally { configLoading.value = false }
}
function startEditing() { if (config.value) editForm.value = { ...config.value }; editing.value = true }
async function saveConfig() {
  try { await api.updateAutoMatchConfig(editForm.value); toast.success(t('settings.configSaved')); config.value = { ...editForm.value }; editing.value = false }
  catch (e: any) { toast.error(e.message || t('settings.configSaveFailed')) }
}

// External Signing
const txs = ref<UnsignedTx[]>([])
const txsLoading = ref(false)
const txsError = ref<string | null>(null)
const witnessJson = ref('')

const txColumns: ColumnDef[] = [
  { key: 'id', label: t('common.id') },
  { key: 'operation', label: t('common.operation') },
  { key: 'status', label: t('common.status'), align: 'center' },
  { key: 'created_at', label: t('common.createdAt') },
  { key: 'actions', label: t('common.actions'), align: 'center' },
]

async function loadUnsignedTxs() {
  txsLoading.value = true; txsError.value = null
  try { txs.value = await api.listUnsignedTxs() }
  catch (e: any) { txsError.value = e.message || t('settings.loadSigningFailed') }
  finally { txsLoading.value = false }
}

async function viewTx(tx: UnsignedTx) {
  witnessJson.value = ''
  try {
    const detail = await api.getUnsignedTx(tx.id)
    modal.show({
      title: t('settings.submitWitness'),
      content: WitnessSubmitModal,
      contentProps: { txId: tx.id, txData: detail.tx_data_json ? JSON.parse(detail.tx_data_json) : detail, operation: tx.operation, modelValue: witnessJson.value, 'onUpdate:modelValue': (v: string) => { witnessJson.value = v } },
      confirmText: t('settings.submitWitness'),
      onConfirm: async () => {
        if (!witnessJson.value.trim()) { toast.warning(t('settings.witnessPlaceholder')); throw new Error('empty') }
        try { const parsed = JSON.parse(witnessJson.value); await api.submitWitnesses(tx.id, parsed); toast.success(t('settings.witnessSuccess')); modal.hide(); await loadUnsignedTxs() }
        catch (e: any) { toast.error(e instanceof SyntaxError ? t('common.jsonInvalid') : e.message || t('settings.witnessFailed')); throw e }
      },
      onCancel: () => modal.hide(),
    })
  } catch (e: any) { toast.error(e.message || t('settings.loadTxFailed')) }
}

async function broadcastTx(tx: UnsignedTx) {
  const ok = await modal.confirm(t('settings.broadcastConfirm', { id: truncateAddress(tx.id, 8, 4) }), { title: t('settings.broadcastTitle'), confirmText: t('common.broadcast') })
  if (!ok) return
  try { await api.submitTx(tx.id); toast.success(t('settings.broadcastSuccess')); await loadUnsignedTxs() }
  catch (e: any) { toast.error(e.message || t('settings.broadcastFailed')) }
}

// Network info
const serverInfo = ref<ServerInfo | null>(null)

async function loadServerInfo() {
  try { serverInfo.value = await api.getServerInfo() }
  catch { /* non-critical */ }
}

onMounted(() => { loadConfig(); loadUnsignedTxs(); loadServerInfo() })
</script>

<template>
  <div class="page-settings">
    <h2 class="page-title">{{ t('settings.title') }}</h2>
    <div class="sub-tabs">
      <button class="sub-tab" :class="{ active: activeTab === 'auto-match' }" @click="activeTab = 'auto-match'">{{ t('settings.autoMatch') }}</button>
      <button class="sub-tab" :class="{ active: activeTab === 'signing' }" @click="activeTab = 'signing'">{{ t('settings.signing') }}</button>
      <button class="sub-tab" :class="{ active: activeTab === 'network' }" @click="activeTab = 'network'">{{ t('settings.network') }}</button>
    </div>

    <div v-if="activeTab === 'auto-match'" class="card config-card">
      <div class="card-header">
        <h3>{{ t('settings.autoMatchConfig') }}</h3>
        <button v-if="!editing" class="btn btn-default btn-sm" @click="startEditing">{{ t('settings.edit') }}</button>
      </div>
      <div v-if="configLoading" class="text-muted">{{ t('common.loading') }}</div>
      <template v-else-if="config">
        <div v-if="!editing" class="config-display">
          <div class="config-row"><span class="config-label">{{ t('settings.enabled') }}</span><StatusTag :status="config.enabled ? 'live' : 'destroyed'" :label="config.enabled ? t('settings.enabledLabel') : t('settings.disabledLabel')" /></div>
          <div class="config-row"><span class="config-label">{{ t('settings.minCapacity') }}</span><span>{{ (config.min_capacity_shannons / 100_000_000).toFixed(0) }} {{ t('common.CKB') }}</span></div>
          <div class="config-row"><span class="config-label">{{ t('settings.maxEscrow') }}</span><span>{{ config.max_escrow_blocks.toLocaleString() }} {{ t('settings.blocks') }}</span></div>
          <div class="config-row"><span class="config-label">{{ t('settings.interval') }}</span><span>{{ config.interval_secs }} {{ t('settings.seconds') }}</span></div>
        </div>
        <div v-else class="config-form">
          <div class="form-group"><label class="form-label"><input v-model="editForm.enabled" type="checkbox" /> {{ t('settings.enableAutoMatch') }}</label></div>
          <div class="form-group"><label class="form-label">{{ t('settings.minCapacity') }} (shannons)</label><input v-model.number="editForm.min_capacity_shannons" type="number" class="form-input" /></div>
          <div class="form-group"><label class="form-label">{{ t('settings.maxEscrow') }}</label><input v-model.number="editForm.max_escrow_blocks" type="number" class="form-input" /></div>
          <div class="form-group"><label class="form-label">{{ t('settings.interval') }} ({{ t('settings.seconds') }})</label><input v-model.number="editForm.interval_secs" type="number" class="form-input" /></div>
          <div class="form-actions"><button class="btn btn-default" @click="editing = false">{{ t('settings.cancel') }}</button><button class="btn btn-primary" @click="saveConfig">{{ t('settings.save') }}</button></div>
          <p class="form-hint text-muted">{{ t('settings.restartNote') }}</p>
        </div>
      </template>
    </div>

    <div v-if="activeTab === 'signing'">
      <div style="display:flex;align-items:center;justify-content:space-between;margin-bottom:var(--space-md);"><h3>{{ t('settings.signing') }}</h3><button class="btn btn-default btn-sm" @click="loadUnsignedTxs">{{ t('matches.refresh') }}</button></div>
      <EmptyState v-if="txsError" icon="⚠️" :message="txsError" :action-label="t('common.retry')" @action="loadUnsignedTxs" />
      <EmptyState v-else-if="!txsLoading && !txs.length" icon="📝" :message="t('settings.noUnsignedTxs')" />
      <DataTable v-else :columns="txColumns" :rows="txs" :loading="txsLoading">
        <template #cell-id="{ value }"><code class="font-mono">{{ truncateAddress(String(value), 8, 4) }}</code></template>
        <template #cell-status="{ value }"><StatusTag :status="String(value)" /></template>
        <template #cell-actions="{ row }">
          <button v-if="row.status === 'pending'" class="btn btn-sm btn-primary" @click="viewTx(row)">{{ t('settings.viewSign') }}</button>
          <button v-else-if="row.status === 'signed'" class="btn btn-sm btn-primary" @click="broadcastTx(row)">{{ t('settings.broadcast') }}</button>
          <span v-else class="text-muted">—</span>
        </template>
      </DataTable>
    </div>

    <div v-if="activeTab === 'network'" class="card config-card">
      <div class="card-header"><h3>{{ t('settings.networkInfo') }}</h3></div>
      <div v-if="serverInfo" class="config-display">
        <div class="config-row">
          <span class="config-label">{{ t('settings.network') }}</span>
          <StatusTag :status="serverInfo.network === 'mainnet' ? 'destroyed' : 'live'" :label="serverInfo.network === 'mainnet' ? 'Mainnet' : 'Testnet'" />
        </div>
        <div class="config-row"><span class="config-label">CKB RPC</span><code class="font-mono" style="font-size:var(--fs-caption)">{{ serverInfo.ckb_rpc_url }}</code></div>
        <div class="config-row"><span class="config-label">CKB Indexer</span><code class="font-mono" style="font-size:var(--fs-caption)">{{ serverInfo.ckb_indexer_url }}</code></div>
        <div class="config-row"><span class="config-label">Fiber RPC</span><code class="font-mono" style="font-size:var(--fs-caption)">{{ serverInfo.fiber_rpc_url }}</code></div>
        <div class="config-row"><span class="config-label">{{ t('settings.version') }}</span><span>{{ serverInfo.version }}</span></div>
        <p class="form-hint text-muted" style="margin-top:var(--space-md)">{{ t('settings.networkSwitchNote') }}</p>
      </div>
      <div v-else class="text-muted">{{ t('common.loading') }}</div>
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
.config-display { display: flex; flex-direction: column; gap: var(--space-md); }
.config-row { display: flex; justify-content: space-between; align-items: center; padding: var(--space-sm) 0; border-bottom: 1px solid var(--border-light); font-size: var(--fs-body); }
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
