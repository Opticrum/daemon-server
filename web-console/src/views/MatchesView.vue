<script setup lang="ts">
import { ref, onMounted, inject } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress } from '@/utils/format'
import DataTable, { type ColumnDef } from '@/components/ui/DataTable.vue'
import StatusTag from '@/components/ui/StatusTag.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import type { TrackedMatch } from '@/types/api'

const api = useApi()
const { t } = useI18n()
const toast = inject<any>('toast')!
const modal = inject<any>('modal')!

const matches = ref<TrackedMatch[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const filterStatus = ref<string>('')

const filters = [
  { labelKey: 'matches.all', value: '' },
  { labelKey: 'matches.live', value: 'live' },
  { labelKey: 'matches.exhausted', value: 'exhausted' },
  { labelKey: 'matches.destroyed', value: 'destroyed' },
]

const columns: ColumnDef[] = [
  { key: 'id', label: t('common.id'), sortable: true, align: 'right' },
  { key: 'tx_hash', label: t('common.txHash') },
  { key: 'seller_address', label: t('common.sellerAddr') },
  { key: 'shannons_per_block', label: t('common.rate'), sortable: true, align: 'right' },
  { key: 'status', label: t('common.status'), align: 'center' },
  { key: 'actions', label: t('common.actions'), align: 'center' },
]

async function loadMatches() {
  loading.value = true; error.value = null
  try { matches.value = await api.listMatches(filterStatus.value || undefined) }
  catch (e: any) { error.value = e.message || t('matches.loadFailed') }
  finally { loading.value = false }
}

function setFilter(status: string) { filterStatus.value = status; loadMatches() }

async function extractRent(match: TrackedMatch) {
  try { await api.extractRent(match.id); toast.success(t('matches.extractSuccess')); await loadMatches() }
  catch (e: any) { toast.error(e.message || 'Extract failed') }
}

async function destroyMatch(match: TrackedMatch) {
  const ok = await modal.confirm(t('matches.destroyConfirm', { id: match.id }), { title: t('matches.destroyTitle'), danger: true, confirmText: t('matches.destroy') })
  if (!ok) return
  try { await api.destroyMatch(match.id); toast.success(t('matches.destroySuccess')); await loadMatches() }
  catch (e: any) { toast.error(e.message || 'Destroy failed') }
}

onMounted(loadMatches)
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
        <code class="font-mono">{{ truncateAddress(String(value), 10, 8) }}</code>
      </template>
      <template #cell-seller_address="{ value }">
        <code class="font-mono">{{ truncateAddress(String(value), 8, 6) }}</code>
      </template>
      <template #cell-shannons_per_block="{ value }">
        {{ value }} {{ t('common.feeRateUnit') }}
      </template>
      <template #cell-status="{ value }">
        <StatusTag :status="String(value)" />
      </template>
      <template #cell-actions="{ row }">
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
          @click="destroyMatch(row)"
        >
          {{ t('matches.destroy') }}
        </button>
        <span
          v-else
          class="text-muted"
        >—</span>
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
.btn { display: inline-flex; align-items: center; gap: var(--space-xs); padding: 0 var(--space-md); height: 32px; border: none; border-radius: var(--radius-md); font-size: var(--fs-body); font-family: inherit; cursor: pointer; transition: all var(--transition-base); font-weight: 500; }
.btn-default { background: var(--bg-card); color: var(--text-primary); border: 1px solid var(--border-dark); } .btn-default:hover { color: var(--primary-500); border-color: var(--primary-500); }
.btn-primary { background: var(--primary-500); color: #fff; } .btn-primary:hover { background: var(--primary-400); }
.btn-danger { background: var(--danger); color: #fff; } .btn-danger:hover { background: #ff7875; }
.btn-sm { height: 28px; font-size: var(--fs-caption); padding: 0 var(--space-sm); }
</style>
