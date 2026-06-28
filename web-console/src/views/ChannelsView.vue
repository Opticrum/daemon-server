<script setup lang="ts">
import { ref, inject } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress, formatCKB } from '@/utils/format'
import DataTable, { type ColumnDef } from '@/components/ui/DataTable.vue'
import StatusTag from '@/components/ui/StatusTag.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import type { FiberChannelInfo } from '@/types/api'

const api = useApi()
const { t } = useI18n()
const toast = inject<any>('toast')!

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
.search-bar { display: flex; gap: var(--space-sm); margin-bottom: var(--space-xl); }
.search-input { flex: 1; height: 36px; padding: 0 var(--space-sm); border: 1px solid var(--border-dark); border-radius: var(--radius-md); font-size: var(--fs-caption); color: var(--text-primary); background: var(--bg-card); outline: none; transition: border-color var(--transition-base), box-shadow var(--transition-base); }
.search-input::placeholder { color: var(--text-disabled); font-family: var(--font-family); font-size: var(--fs-body); }
.search-input:focus { border-color: var(--primary-500); box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2); }
.btn { display: inline-flex; align-items: center; gap: var(--space-xs); padding: 0 var(--space-md); height: 36px; border: none; border-radius: var(--radius-md); font-size: var(--fs-body); font-family: inherit; cursor: pointer; transition: all var(--transition-base); font-weight: 500; white-space: nowrap; }
.btn-primary { background: var(--primary-500); color: #fff; } .btn-primary:hover:not(:disabled) { background: var(--primary-400); } .btn-primary:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
