<script setup lang="ts">
import { ref, onMounted, inject, h } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress, formatCKB, explorerTxUrl } from '@/utils/format'
import DataTable, { type ColumnDef } from '@/components/ui/DataTable.vue'
import StatusTag from '@/components/ui/StatusTag.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import MatchDetailPanel, { type DetailSection } from '@/components/ui/MatchDetailPanel.vue'
import type { TrackedMatch, MatchDetail } from '@/types/api'

const api = useApi()
const { t } = useI18n()
const toast = inject<any>('toast')!
const modal = inject<any>('modal')!

const matches = ref<TrackedMatch[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const filterStatus = ref<string>('')
const network = ref('testnet')
const signerLockHashes = ref<string[]>([])

const filters = [
  { labelKey: 'matches.all', value: '' },
  { labelKey: 'matches.live', value: 'live' },
  { labelKey: 'matches.exhausted', value: 'exhausted' },
  { labelKey: 'matches.destroyed', value: 'destroyed' },
]

const columns: ColumnDef[] = [
  { key: 'tx_hash', label: t('common.txHash'), align: 'center' },
  { key: 'created_at', label: t('matches.matchTime'), align: 'center' },
  { key: 'rate', label: t('common.rate'), sortable: true, align: 'center' },
  { key: 'rent', label: t('matches.rent'), align: 'center' },
  { key: 'status', label: t('common.status'), align: 'center' },
  { key: 'actions', label: t('common.actions'), align: 'center' },
]

const totalRent = (m: TrackedMatch) => m.ckb_capacity || 0
const extractedRent = (m: TrackedMatch) => m.extracted_total ?? 0

async function loadSignerLockHashes() {
  try {
    const wallets = await api.getSignerWallets()
    signerLockHashes.value = wallets.map(w => w.lock_hash).filter(Boolean)
  } catch {
    signerLockHashes.value = []
  }
}

async function loadMatches() {
  loading.value = true; error.value = null
  try {
    const hashes = signerLockHashes.value.length > 0 ? signerLockHashes.value : undefined
    matches.value = await api.listMatches(filterStatus.value || undefined, hashes)
  }
  catch (e: any) { console.error('Failed to load matches:', e); error.value = e.message || t('matches.loadFailed') }
  finally { loading.value = false }
}

function setFilter(status: string) { filterStatus.value = status; loadMatches() }

async function extractRent(match: TrackedMatch) {
  try { await api.extractRent(match.tx_hash, match.output_index); toast.success(t('matches.extractSuccess')); await loadMatches() }
  catch (e: any) { toast.error(e.message || 'Extract failed') }
}

async function doDestroyMatch(match: TrackedMatch) {
  const label = truncateAddress(match.tx_hash, 8, 6)
  const ok = await modal.confirm(t('matches.destroyConfirm', { id: label }), { title: t('matches.destroyTitle'), danger: true, confirmText: t('matches.destroy') })
  if (!ok) return
  try { await api.destroyMatch(match.tx_hash, match.output_index); toast.success(t('matches.destroySuccess')); await loadMatches() }
  catch (e: any) { toast.error(e.message || 'Destroy failed') }
}

// ── Match Detail Modal ──

async function showMatchDetail(match: TrackedMatch) {
  let detail: MatchDetail | null
  try {
    detail = await api.getMatchDetail(match.tx_hash, match.output_index)
  } catch (e: any) {
    toast.error(e.message || 'Failed to load match detail')
    return
  }

  if (!detail) return

  const createdStr = detail.created_at
    ? new Date(detail.created_at).toLocaleString()
    : '—'

  const sections: DetailSection[] = [
    {
      title: t('channels.matchEconomics'),
      fields: [
        { label: t('matches.matchTime'), value: createdStr },
        { label: t('common.status'), value: detail.status, type: 'status' },
        { label: t('common.capacity'), value: formatCKB(detail.ckb_capacity) },
        { label: t('common.ratePerBlock'), value: `${detail.shannons_per_block} ${t('common.feeRateUnit')}` },
        { label: 'xUDT', value: detail.xudt_amount ? `${detail.xudt_amount}` : t('common.none') },
        { label: t('matches.lastExtractionBlock'), value: String(detail.last_extraction_block) },
        { label: t('matches.rent'), value: `${formatCKB(detail.extracted_total_shannons)} / ${formatCKB(detail.ckb_capacity)}` },
      ],
    },
  ]

  const extractionHistory = {
    title: t('matches.extractionHistory'),
    headers: [
      t('matches.rent').split('/')[0].trim(),
      t('matches.lastExtractionBlock'),
      t('common.txHash'),
      t('common.createdAt'),
    ] as [string, string, string, string],
    rows: detail.extraction_history.map(ex => ({
      amount: formatCKB(ex.extracted_amount),
      block: String(ex.tip_block),
      txHash: ex.tx_hash,
      timestamp: ex.timestamp ? new Date(ex.timestamp).toLocaleString() : '—',
    })),
    emptyText: t('matches.noExtractions'),
  }

  modal.show({
    title: t('matches.detailTitle'),
    wide: true,
    content: {
      components: { MatchDetailPanel },
      setup() {
        return () => h(MatchDetailPanel, { sections, extractionHistory })
      },
    },
    confirmText: null,
    cancelText: t('common.close'),
    onCancel: () => modal.hide(),
  })
}

onMounted(async () => {
  try { const info = await api.getServerInfo(); network.value = info.network } catch { /* ignore */ }
  await loadSignerLockHashes()
  await loadMatches()
})
</script>

<template>
  <div class="page-matches">
    <div class="page-header">
      <h2 class="page-title">
        {{ t('matches.title') }}
      </h2>
      <button
        class="btn btn-default"
        @click="loadMatches"
      >
        {{ t('matches.refresh') }}
      </button>
    </div>
    <div class="filter-tabs">
      <button
        v-for="f in filters"
        :key="f.value"
        class="filter-tab"
        :class="{ active: filterStatus === f.value }"
        @click="setFilter(f.value)"
      >
        {{ t(f.labelKey) }}
      </button>
    </div>
    <EmptyState
      v-if="error"
      icon="⚠️"
      :message="error"
      :action-label="t('common.retry')"
      @action="loadMatches"
    />
    <EmptyState
      v-else-if="!loading && !matches.length"
      icon="🔗"
      :message="filterStatus ? t('matches.noStatusMatches', { status: t(filters.find(f => f.value === filterStatus)?.labelKey || '') }) : t('matches.noMatches')"
    />
    <DataTable
      v-else
      :columns="columns"
      :rows="matches"
      :loading="loading"
    >
      <template #cell-tx_hash="{ value }">
        <a
          :href="explorerTxUrl(String(value), network)"
          target="_blank"
          rel="noopener noreferrer"
          class="tx-link font-mono"
        >{{ truncateAddress(String(value), 12, 8) }}</a>
      </template>
      <template #cell-created_at="{ value }">
        <span class="match-time">{{ value ? new Date(Number(value)).toLocaleString() : '—' }}</span>
      </template>
      <template #cell-rate="{ row }">
        {{ row.shannons_per_block }} {{ t('common.feeRateUnit') }}
      </template>
      <template #cell-rent="{ row }">
        {{ formatCKB(extractedRent(row)) }} / {{ formatCKB(totalRent(row)) }}
      </template>
      <template #cell-status="{ value }">
        <StatusTag :status="String(value)" />
      </template>
      <template #cell-actions="{ row }">
        <div class="actions-group">
          <button
            class="btn btn-sm btn-default"
            @click="showMatchDetail(row)"
          >
            {{ t('matches.detail') }}
          </button>
          <button
            v-if="row.status === 'live'"
            class="btn btn-sm btn-primary"
            @click="extractRent(row)"
          >
            {{ t('matches.extract') }}
          </button>
          <button
            v-else-if="row.status === 'exhausted'"
            class="btn btn-sm btn-danger"
            @click="doDestroyMatch(row)"
          >
            {{ t('matches.destroy') }}
          </button>
        </div>
      </template>
    </DataTable>
  </div>
</template>

<style scoped>
.page-matches { max-width: 1200px; margin: 0 auto; }
.page-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-md); }
.page-title { font-size: var(--fs-h2); font-weight: var(--fw-h2); line-height: var(--lh-h2); color: var(--text-primary); }
.filter-tabs { display: flex; gap: var(--space-xs); margin-bottom: var(--space-lg); }
.filter-tab { padding: var(--space-xs) var(--space-md); border: 1px solid var(--border-dark); border-radius: var(--radius-md); background: var(--bg-card); color: var(--text-secondary); font-size: var(--fs-body); cursor: pointer; transition: all var(--transition-base); }
.filter-tab:hover { color: var(--primary-500); border-color: var(--primary-500); } .filter-tab.active { background: var(--primary-500); border-color: var(--primary-500); color: #fff; }
.tx-link { color: var(--primary-500); text-decoration: none; } .tx-link:hover { text-decoration: underline; }
.copyable { cursor: pointer; transition: color var(--transition-base); }
.copyable:hover { color: var(--primary-500); }
.actions-group { display: flex; gap: 4px; justify-content: center; }
.btn { display: inline-flex; align-items: center; gap: var(--space-xs); padding: 0 var(--space-md); height: 32px; border: none; border-radius: var(--radius-md); font-size: var(--fs-body); font-family: inherit; cursor: pointer; transition: all var(--transition-base); font-weight: 500; }
.btn-default { background: var(--bg-card); color: var(--text-primary); border: 1px solid var(--border-dark); } .btn-default:hover { color: var(--primary-500); border-color: var(--primary-500); }
.btn-primary { background: var(--primary-500); color: #fff; } .btn-primary:hover { background: var(--primary-400); }
.btn-danger { background: var(--danger); color: #fff; } .btn-danger:hover { background: #ff7875; }
.btn-sm { height: 28px; font-size: var(--fs-caption); padding: 0 var(--space-sm); }
.md-footer-actions { display: flex; gap: 8px; }
</style>
