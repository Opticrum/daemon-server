<script setup lang="ts">
import { ref, onMounted, inject } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress } from '@/utils/format'
import WalletImportForm from '@/components/ui/WalletImportForm.vue'
import Skeleton from '@/components/ui/Skeleton.vue'
import EmptyState from '@/components/ui/EmptyState.vue'
import type { WalletResponse, ImportWalletRequest } from '@/types/api'

const api = useApi()
const { t } = useI18n()
const toast = inject<any>('toast')!
const modal = inject<any>('modal')!

const wallets = ref<WalletResponse[]>([])
const loading = ref(true)
const error = ref<string | null>(null)
const importForm = ref<ImportWalletRequest>({ label: '', private_key_hex: '', password: '' })

async function loadWallets() {
  loading.value = true
  error.value = null
  try { wallets.value = await api.listWallets() }
  catch (e: any) { error.value = e.message || t('wallets.loadFailed') }
  finally { loading.value = false }
}

async function importWallet() {
  if (!importForm.value.label || !importForm.value.private_key_hex) {
    toast.warning(t('wallets.fillRequired'))
    return
  }
  try {
    await api.importWallet(importForm.value)
    toast.success(t('wallets.importSuccess'))
    importForm.value = { label: '', private_key_hex: '', password: '' }
    modal.hide()
    await loadWallets()
  } catch (e: any) { toast.error(e.message || t('wallets.importFailed')) }
}

async function deleteWallet(id: number, label: string) {
  const ok = await modal.confirm(t('wallets.deleteConfirm', { label }), {
    title: t('wallets.deleteTitle'), danger: true, confirmText: t('common.deleteConfirm'),
  })
  if (!ok) return
  try { await api.deleteWallet(id); toast.success(t('wallets.deleteSuccess')); await loadWallets() }
  catch (e: any) { toast.error(e.message || t('wallets.deleteFailed')) }
}

function showImportModal() {
  importForm.value = { label: '', private_key_hex: '', password: '' }
  modal.show({
    title: t('wallets.importTitle'),
    content: WalletImportForm,
    contentProps: { modelValue: importForm.value, 'onUpdate:modelValue': (v: ImportWalletRequest) => { importForm.value = v } },
    confirmText: t('common.import'),
    onConfirm: importWallet,
    onCancel: () => modal.hide(),
  })
}

onMounted(loadWallets)
</script>

<template>
  <div class="page-wallets">
    <div class="page-header">
      <h2 class="page-title">{{ t('wallets.title') }}</h2>
      <button class="btn btn-primary" @click="showImportModal">+ {{ t('wallets.import') }}</button>
    </div>

    <Skeleton v-if="loading" type="card" :cols="3" />
    <EmptyState v-else-if="error" icon="⚠️" :message="error" :action-label="t('common.retry')" @action="loadWallets" />
    <EmptyState v-else-if="!wallets.length" icon="💼" :message="t('wallets.noWallets')" :action-label="t('wallets.import')" @action="showImportModal" />
    <div v-else class="wallet-grid">
      <div v-for="w in wallets" :key="w.id" class="wallet-card card">
        <div class="wallet-card-header">
          <span class="wallet-label">{{ w.label }}</span>
          <button class="btn-delete" @click="deleteWallet(w.id, w.label)" title="Delete">🗑️</button>
        </div>
        <div class="wallet-addr font-mono">{{ truncateAddress(w.ckb_address, 12, 10) }}</div>
        <div class="wallet-meta"><span class="text-muted">{{ t('wallets.lockHash') }}</span><code class="font-mono">{{ truncateAddress(w.lock_hash, 8, 6) }}</code></div>
        <div class="wallet-meta"><span class="text-muted">{{ t('wallets.createdAt') }}</span><span>{{ w.created_at }}</span></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.page-wallets { max-width: 1200px; margin: 0 auto; }
.page-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-xl); }
.page-title { font-size: var(--fs-h2); font-weight: var(--fw-h2); line-height: var(--lh-h2); color: var(--text-primary); }
.btn { display: inline-flex; align-items: center; gap: var(--space-xs); padding: 0 var(--space-md); height: 32px; border: none; border-radius: var(--radius-md); font-size: var(--fs-body); font-family: inherit; cursor: pointer; transition: all var(--transition-base); font-weight: 500; }
.btn-primary { background: var(--primary-500); color: #fff; } .btn-primary:hover { background: var(--primary-400); }
.wallet-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: var(--space-md); }
.wallet-card { background: var(--bg-card); border-radius: var(--radius-lg); border: 1px solid var(--border-light); box-shadow: var(--shadow-base); padding: var(--space-xl); transition: all var(--transition-base); }
.wallet-card:hover { box-shadow: var(--shadow-lg); transform: translateY(-2px); }
.wallet-card-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-sm); }
.wallet-label { font-size: var(--fs-h3); font-weight: var(--fw-h3); color: var(--text-primary); }
.btn-delete { background: none; border: none; cursor: pointer; font-size: 16px; opacity: 0.5; transition: opacity var(--transition-base); } .btn-delete:hover { opacity: 1; }
.wallet-addr { font-size: var(--fs-caption); color: var(--text-secondary); margin-bottom: var(--space-md); padding: var(--space-xs) var(--space-sm); background: var(--gray-50); border-radius: var(--radius-sm); word-break: break-all; }
.wallet-meta { display: flex; justify-content: space-between; font-size: var(--fs-caption); margin-bottom: var(--space-xs); }
.wallet-meta code { font-size: var(--fs-small); color: var(--primary-600); }
</style>
