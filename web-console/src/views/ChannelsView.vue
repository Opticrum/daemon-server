<script setup lang="ts">
import { ref, onMounted, inject } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import { useFiber } from '@/composables/useFiber'
import { truncateAddress, formatCKB } from '@/utils/format'
import DataTable, { type ColumnDef } from '@/components/ui/DataTable.vue'
import StatusTag from '@/components/ui/StatusTag.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import type { FiberChannelInfo } from '@/types/api'

const api = useApi()
const { t } = useI18n()
const toast = inject<any>('toast')!

const { rpcUrl, nodeInfo, connected, loading: nodeLoading, error: nodeError, fetchNodeInfo } = useFiber()

const channels = ref<FiberChannelInfo[]>([])
const loading = ref(false)
const scanned = ref(false)
const error = ref<string | null>(null)
const ownerLockHash = ref('')

const columns: ColumnDef[] = [
  { key: 'tx_hash', label: t('common.txHash') },
  { key: 'output_index', label: t('common.outputIndex'), align: 'right' },
  { key: 'capacity', label: t('common.capacity'), sortable: true, align: 'right' },
  { key: 'status', label: t('common.status'), align: 'center' },
]

function hexToNum(hex: string): number {
  return parseInt(hex, 16) || 0
}

onMounted(() => { fetchNodeInfo() })

async function scanChannels() {
  loading.value = true; error.value = null
  try { channels.value = await api.scanChannels(ownerLockHash.value || undefined); scanned.value = true }
  catch (e: any) { error.value = e.message || t('channels.scanFailed'); toast.error(error.value!) }
  finally { loading.value = false }
}
</script>

<template>
  <div class="page-channels">
    <div class="page-header"><h2 class="page-title">{{ t('channels.title') }}</h2></div>

    <!-- Fiber Node Info Card -->
    <div class="card node-info-card">
      <div class="card-header">
        <h3>{{ t('channels.nodeInfo') }}</h3>
        <span v-if="nodeLoading" class="spinner" />
        <StatusTag v-else :status="connected ? 'live' : 'destroyed'" :label="connected ? t('channels.connected') : t('channels.disconnected')" />
      </div>
      <div v-if="nodeLoading" class="text-muted" style="padding: var(--space-md) 0;">{{ t('common.loading') }}</div>
      <div v-else-if="nodeInfo" class="config-display">
        <div class="config-row"><span class="config-label">{{ t('channels.nodeRpcUrl') }}</span><code class="font-mono" style="font-size:var(--fs-caption)">{{ rpcUrl }}</code></div>
        <div class="config-row"><span class="config-label">{{ t('channels.nodeId') }}</span><code class="font-mono value-full" style="font-size:var(--fs-caption)">{{ nodeInfo.pubkey }}</code></div>
        <div class="config-row"><span class="config-label">{{ t('channels.nodeVersion') }}</span><span>{{ nodeInfo.version }}</span></div>
        <div v-if="nodeInfo.addresses.length" class="config-row">
          <span class="config-label">{{ t('channels.nodeAddresses') }}</span>
          <div class="addresses-stack">
            <code v-for="(addr, i) in nodeInfo.addresses" :key="i" class="font-mono" style="font-size:var(--fs-small)">{{ addr }}</code>
          </div>
        </div>
        <div class="config-row"><span class="config-label">{{ t('channels.channelCount') }}</span><span>{{ hexToNum(nodeInfo.channel_count) }}</span></div>
        <div class="config-row"><span class="config-label">{{ t('channels.pendingChannelCount') }}</span><span>{{ hexToNum(nodeInfo.pending_channel_count) }}</span></div>
        <div class="config-row"><span class="config-label">{{ t('channels.peerCount') }}</span><span>{{ hexToNum(nodeInfo.peers_count) }}</span></div>
        <div class="config-row"><span class="config-label">{{ t('channels.chainHash') }}</span><code class="font-mono value-full" style="font-size:var(--fs-caption)">{{ nodeInfo.chain_hash }}</code></div>
      </div>
      <div v-else class="text-muted" style="padding: var(--space-md) 0;">
        {{ t('channels.disconnected') }}
        <code v-if="rpcUrl" class="font-mono" style="font-size:var(--fs-small); display:block; margin-top:var(--space-xs);">{{ rpcUrl }}</code>
      </div>
    </div>

    <div class="search-bar">
      <input v-model="ownerLockHash" type="text" class="search-input font-mono" :placeholder="t('channels.placeholder')" maxlength="64" @keyup.enter="scanChannels" />
      <button class="btn btn-primary" :disabled="loading" @click="scanChannels">
        <span v-if="loading" class="spinner" /> {{ loading ? t('channels.scanning') : t('channels.scan') }}
      </button>
    </div>
    <EmptyState v-if="!scanned" icon="🌐" :message="t('channels.enterHash')" />
    <EmptyState v-else-if="error" icon="⚠️" :message="error" :action-label="t('common.retry')" @action="scanChannels" />
    <EmptyState v-else-if="!channels.length" icon="🌐" :message="t('channels.noChannels')" />
    <DataTable v-else :columns="columns" :rows="channels" :loading="loading">
      <template #cell-tx_hash="{ value }"><code class="font-mono">{{ truncateAddress(String(value), 10, 8) }}</code></template>
      <template #cell-capacity="{ value }">{{ formatCKB(Number(value)) }}</template>
      <template #cell-status="{ value }"><StatusTag :status="String(value)" /></template>
    </DataTable>
  </div>
</template>

<style scoped>
.page-channels { max-width: 1200px; margin: 0 auto; }
.page-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-xl); }
.page-title { font-size: var(--fs-h2); font-weight: var(--fw-h2); line-height: var(--lh-h2); color: var(--text-primary); }

/* Node Info Card */
.node-info-card { margin-bottom: var(--space-xl); }
.node-info-card .spinner { width: 16px; height: 16px; border-width: 2px; }
.card { background: var(--bg-card); border-radius: var(--radius-lg); border: 1px solid var(--border-light); box-shadow: var(--shadow-base); padding: var(--space-xl); }
.card-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-lg); }
.card-header h3 { font-size: var(--fs-h3); font-weight: var(--fw-h3); margin: 0; }
.config-display { display: flex; flex-direction: column; gap: var(--space-md); }
.config-row { display: flex; justify-content: space-between; align-items: flex-start; padding: var(--space-sm) 0; border-bottom: 1px solid var(--border-light); font-size: var(--fs-body); }
.config-label { color: var(--text-secondary); flex-shrink: 0; margin-right: var(--space-md); }
.value-full { max-width: 70%; word-break: break-all; text-align: right; }
.addresses-stack { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; max-width: 70%; word-break: break-all; }

.search-bar { display: flex; gap: var(--space-sm); margin-bottom: var(--space-xl); }
.search-input { flex: 1; height: 36px; padding: 0 var(--space-sm); border: 1px solid var(--border-dark); border-radius: var(--radius-md); font-size: var(--fs-caption); color: var(--text-primary); background: var(--bg-card); outline: none; transition: border-color var(--transition-base), box-shadow var(--transition-base); }
.search-input::placeholder { color: var(--text-disabled); font-family: var(--font-family); font-size: var(--fs-body); }
.search-input:focus { border-color: var(--primary-500); box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2); }
.btn { display: inline-flex; align-items: center; gap: var(--space-xs); padding: 0 var(--space-md); height: 36px; border: none; border-radius: var(--radius-md); font-size: var(--fs-body); font-family: inherit; cursor: pointer; transition: all var(--transition-base); font-weight: 500; white-space: nowrap; }
.btn-primary { background: var(--primary-500); color: #fff; } .btn-primary:hover:not(:disabled) { background: var(--primary-400); } .btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
