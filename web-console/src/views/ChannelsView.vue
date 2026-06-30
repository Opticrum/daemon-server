<script setup lang="ts">
import { ref, onMounted, inject, computed } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import { useFiber } from '@/composables/useFiber'
import { truncateAddress, formatCKB } from '@/utils/format'
import DataTable, { type ColumnDef } from '@/components/ui/DataTable.vue'
import StatusTag from '@/components/ui/StatusTag.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import type { ChannelWithMatch } from '@/types/api'

const api = useApi()
const { t } = useI18n()
const toast = inject<any>('toast')!
const modal = inject<any>('modal')!

const { rpcUrl, nodeInfo, connected, loading: nodeLoading, fetchNodeInfo } = useFiber()

const channels = ref<ChannelWithMatch[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const fiberKeyFilter = ref('')
const nodeExpanded = ref(false)

// Auto-load everything on mount
onMounted(async () => {
  fetchNodeInfo()
  await loadChannels()
})

async function loadChannels() {
  loading.value = true
  error.value = null
  try {
    channels.value = await api.scanChannels()
  } catch (e: any) {
    console.error('Failed to load channels:', e);
    error.value = e.message || t('channels.loadFailed')
    toast.error(error.value!)
  } finally {
    loading.value = false
  }
}

// Filter channels by counterparty fiber key substring
const filteredChannels = computed(() => {
  if (!fiberKeyFilter.value.trim()) return channels.value
  const q = fiberKeyFilter.value.trim().toLowerCase()
  return channels.value.filter(c =>
    c.counterparty_fiber_key.toLowerCase().includes(q) ||
    c.tx_hash.toLowerCase().includes(q)
  )
})

const columns: ColumnDef[] = [
  { key: 'channel_id', label: t('channels.channelId'), align: 'center' },
  { key: 'counterparty_fiber_key', label: t('channels.counterpartyFiberKey'), align: 'center' },
  { key: 'capacity', label: t('channels.totalCapacity'), sortable: true, align: 'center' },
  { key: 'state_name', label: t('channels.stateName'), align: 'center' },
  { key: 'match_status', label: t('channels.matchStatus'), align: 'center' },
  { key: 'actions', label: t('common.actions'), align: 'center' },
]

function hexToNum(hex: string): number {
  return parseInt(hex, 16) || 0
}

function channelStateVariant(name: string): string {
  switch (name) {
    case 'ChannelReady': return 'live'
    case 'ShuttingDown': return 'warning'
    case 'Closed': return 'destroyed'
    default: return 'pending'
  }
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    toast.success(t('common.copied'))
  } catch {
    console.warn('Clipboard API unavailable, using fallback');
    // Fallback for older browsers
    const ta = document.createElement('textarea')
    ta.value = text
    ta.style.position = 'fixed'; ta.style.opacity = '0'
    document.body.appendChild(ta)
    ta.select()
    document.execCommand('copy')
    document.body.removeChild(ta)
    toast.success(t('common.copied'))
  }
}

function formatMatchRate(shannons: number): string {
  return `${shannons} sh/block`
}

async function closeChannel(channel: ChannelWithMatch) {
  const channelId = channel.channel_id
  const confirmed = await modal.confirm(
    t('channels.closeChannelWarning', { id: truncateAddress(channelId, 8, 8) }),
    {
      title: t('channels.closeChannelTitle'),
      confirmText: t('channels.closeChannelConfirm'),
      danger: true,
    },
  )
  if (!confirmed) return
  try {
    await api.closeChannel(channelId)
    toast.success(t('channels.closeSuccess'))
    await loadChannels()
  } catch (e: any) {
    console.error('Failed to close channel:', e);
    toast.error(e.message || t('channels.closeFailed'))
  }
}
</script>

<template>
  <div class="page-channels">
    <div class="page-header">
      <h2 class="page-title">
        {{ t('channels.title') }}
      </h2>
    </div>

    <!-- Fiber Node Info Card (foldable) -->
    <div class="card node-info-card">
      <div
        class="card-header"
        @click="nodeExpanded = !nodeExpanded"
      >
        <div class="card-header-left">
          <span
            class="fold-arrow"
            :class="{ expanded: nodeExpanded }"
          >&#9654;</span>
          <h3>{{ t('channels.nodeInfo') }}</h3>
        </div>
        <div class="card-header-right">
          <code
            v-if="!nodeLoading && nodeInfo"
            class="font-mono node-url-inline"
          >{{ rpcUrl }}</code>
          <span
            v-if="nodeLoading"
            class="spinner"
          />
          <StatusTag
            v-else
            :status="connected ? 'live' : 'destroyed'"
            :label="connected ? t('channels.connected') : t('channels.disconnected')"
          />
        </div>
      </div>
      <!-- Disconnected fallback -->
      <div
        v-if="!nodeLoading && !nodeInfo"
        class="text-muted node-summary"
      >
        {{ t('channels.disconnected') }}
        <code
          v-if="rpcUrl"
          class="font-mono"
          style="font-size:var(--fs-small); display:block; margin-top:var(--space-xs);"
        >{{ rpcUrl }}</code>
      </div>
      <!-- Expanded details -->
      <div
        v-if="nodeExpanded && nodeInfo"
        class="config-display"
      >
        <div class="config-row">
          <span class="config-label">{{ t('channels.nodeId') }}</span><code
            class="font-mono value-full"
            style="font-size:var(--fs-caption)"
          >{{ nodeInfo.pubkey }}</code>
        </div>
        <div class="config-row">
          <span class="config-label">{{ t('channels.nodeVersion') }}</span><span>{{ nodeInfo.version }}</span>
        </div>
        <div
          v-if="nodeInfo.addresses.length"
          class="config-row"
        >
          <span class="config-label">{{ t('channels.nodeAddresses') }}</span>
          <div class="addresses-stack">
            <code
              v-for="(addr, i) in nodeInfo.addresses"
              :key="i"
              class="font-mono"
              style="font-size:var(--fs-small)"
            >{{ addr }}</code>
          </div>
        </div>
        <div class="config-row">
          <span class="config-label">{{ t('channels.channelCount') }}</span><span>{{ hexToNum(nodeInfo.channel_count) }}</span>
        </div>
        <div class="config-row">
          <span class="config-label">{{ t('channels.pendingChannelCount') }}</span><span>{{ hexToNum(nodeInfo.pending_channel_count) }}</span>
        </div>
        <div class="config-row">
          <span class="config-label">{{ t('channels.peerCount') }}</span><span>{{ hexToNum(nodeInfo.peers_count) }}</span>
        </div>
        <div class="config-row">
          <span class="config-label">{{ t('channels.chainHash') }}</span><code
            class="font-mono value-full"
            style="font-size:var(--fs-caption)"
          >{{ nodeInfo.chain_hash }}</code>
        </div>
      </div>
      <div
        v-if="nodeLoading"
        class="text-muted"
        style="padding: var(--space-md) 0;"
      >
        {{ t('common.loading') }}
      </div>
    </div>

    <!-- Toolbar: refresh + filter -->
    <div class="toolbar">
      <div class="search-bar">
        <input
          v-model="fiberKeyFilter"
          type="text"
          class="search-input font-mono"
          :placeholder="t('channels.fiberKeyFilter')"
        >
      </div>
      <button
        class="btn btn-primary"
        :disabled="loading"
        @click="loadChannels"
      >
        <span
          v-if="loading"
          class="spinner"
        />
        {{ loading ? t('channels.refreshing') : t('channels.refresh') }}
      </button>
    </div>

    <!-- States -->
    <EmptyState
      v-if="!loading && error"
      icon="⚠️"
      :message="error"
      :action-label="t('common.retry')"
      @action="loadChannels"
    />
    <EmptyState
      v-else-if="!loading && !channels.length"
      icon="🌐"
      :message="t('channels.noChannels')"
    />
    <DataTable
      v-else
      :columns="columns"
      :rows="filteredChannels"
      :loading="loading"
    >
      <template #cell-channel_id="{ value }">
        <code class="font-mono">{{ truncateAddress(String(value), 8, 6) }}</code>
      </template>
      <template #cell-counterparty_fiber_key="{ value }">
        <code
          class="font-mono copyable fiber-key-cell"
          :title="String(value)"
          @click="copyToClipboard(String(value))"
        >{{ truncateAddress(String(value), 20, 16) }}</code>
      </template>
      <template #cell-capacity="{ value }">
        {{ formatCKB(Number(value)) }}
      </template>
      <template #cell-state_name="{ value }">
        <StatusTag
          :status="channelStateVariant(String(value))"
          :label="String(value)"
        />
      </template>
      <template #cell-match_status="{ row }">
        <div
          v-if="row.match_info"
          class="match-cell"
        >
          <div class="match-tx">
            <code class="font-mono match-hash">{{ truncateAddress(String(row.match_info.match_tx_hash), 10, 6) }}</code>
          </div>
          <div class="match-meta">
            <span class="match-amount">{{ row.match_info.xudt_amount > 0 ? row.match_info.xudt_amount + ' xUDT' : formatCKB(row.match_info.ckb_capacity) }}</span>
            <span class="match-rate">{{ formatMatchRate(row.match_info.shannons_per_block) }}</span>
          </div>
        </div>
        <span
          v-else
          class="match-none"
        >{{ t('channels.matchNotFound') }}</span>
      </template>
      <template #cell-actions="{ row }">
        <button
          v-if="row.state_name !== 'Closed'"
          class="btn btn-sm btn-danger"
          @click="closeChannel(row)"
        >
          {{ t('channels.closeChannel') }}
        </button>
      </template>
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
.card-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-lg); cursor: pointer; user-select: none; }
.card-header:hover { opacity: 0.85; }
.card-header-left { display: flex; align-items: center; gap: var(--space-sm); }
.card-header-right { display: flex; align-items: center; gap: var(--space-sm); }
.card-header h3 { font-size: var(--fs-h3); font-weight: var(--fw-h3); margin: 0; }
.fold-arrow { display: inline-block; font-size: var(--fs-small); transition: transform var(--transition-base); color: var(--text-secondary); }
.fold-arrow.expanded { transform: rotate(90deg); }
.node-url-inline { font-size: var(--fs-caption); color: var(--text-secondary); max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.node-summary { padding-bottom: var(--space-md); border-bottom: 1px solid var(--border-light); font-size: var(--fs-body); }
.config-display { display: flex; flex-direction: column; gap: var(--space-md); }
.config-row { display: flex; justify-content: space-between; align-items: flex-start; padding: var(--space-sm) 0; border-bottom: 1px solid var(--border-light); font-size: var(--fs-body); }
.config-row:last-child { border-bottom: none; }
.config-label { color: var(--text-secondary); flex-shrink: 0; margin-right: var(--space-md); }
.value-full { max-width: 70%; word-break: break-all; text-align: right; }
.addresses-stack { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; max-width: 70%; word-break: break-all; }

/* Toolbar */
.toolbar { display: flex; gap: var(--space-sm); margin-bottom: var(--space-xl); align-items: center; }
.search-bar { flex: 1; display: flex; }
.search-input { flex: 1; height: 36px; padding: 0 var(--space-sm); border: 1px solid var(--border-dark); border-radius: var(--radius-md); font-size: var(--fs-caption); color: var(--text-primary); background: var(--bg-card); outline: none; transition: border-color var(--transition-base), box-shadow var(--transition-base); }
.search-input::placeholder { color: var(--text-disabled); font-family: var(--font-family); font-size: var(--fs-body); }
.search-input:focus { border-color: var(--primary-500); box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2); }
.btn { display: inline-flex; align-items: center; gap: var(--space-xs); padding: 0 var(--space-md); height: 36px; border: none; border-radius: var(--radius-md); font-size: var(--fs-body); font-family: inherit; cursor: pointer; transition: all var(--transition-base); font-weight: 500; white-space: nowrap; }
.btn-primary { background: var(--primary-500); color: #fff; } .btn-primary:hover:not(:disabled) { background: var(--primary-400); } .btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
.spinner { display: inline-block; width: 14px; height: 14px; border: 2px solid rgba(255, 255, 255, 0.3); border-top-color: #fff; border-radius: 50%; animation: spin 0.6s linear infinite; }

/* Match cell column */
.match-cell {
  display: flex;
  flex-direction: column;
  gap: 2px;
  align-items: center;
}
.match-tx { margin-bottom: 1px; }
.match-hash { font-size: var(--fs-caption); color: var(--primary-500); }
.match-meta {
  display: flex;
  gap: var(--space-sm);
  font-size: var(--fs-small);
  color: var(--text-secondary);
}
.match-amount { color: var(--primary-500); }
.match-rate { font-family: var(--font-mono); }
.match-none {
  color: var(--text-disabled);
  font-size: var(--fs-body);
  font-style: italic;
}

/* Channel table — centered like on-chain orders */
.page-channels :deep(.data-table) {
  table-layout: fixed;
  width: 100%;
}
.page-channels :deep(.data-table th:nth-child(1)),
.page-channels :deep(.data-table td:nth-child(1)) {
  width: 14%;
}
.page-channels :deep(.data-table th:nth-child(2)),
.page-channels :deep(.data-table td:nth-child(2)) {
  width: 32%;
  overflow: hidden;
}
.page-channels :deep(.data-table th:nth-child(3)),
.page-channels :deep(.data-table td:nth-child(3)),
.page-channels :deep(.data-table th:nth-child(4)),
.page-channels :deep(.data-table td:nth-child(4)),
.page-channels :deep(.data-table th:nth-child(5)),
.page-channels :deep(.data-table td:nth-child(5)),
.page-channels :deep(.data-table th:nth-child(6)),
.page-channels :deep(.data-table td:nth-child(6)) {
  width: 13%;
}
.page-channels :deep(.data-table th),
.page-channels :deep(.data-table td) {
  text-align: center;
}
.fiber-key-cell {
  display: inline-block;
  max-width: 100%;
  white-space: nowrap;
  font-size: var(--fs-caption);
  vertical-align: middle;
}

/* Click-to-copy */
.copyable {
  cursor: pointer;
  transition: color var(--transition-base);
}
.copyable:hover {
  color: var(--primary-500);
}

@keyframes spin { to { transform: rotate(360deg); } }
</style>
