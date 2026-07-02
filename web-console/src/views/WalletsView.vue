<script setup lang="ts">
import { ref, onMounted, inject, computed, h } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress, formatCKB } from '@/utils/format'
import type { WalletResponse, HdStatusResponse, AddressBalanceItem } from '@/types/api'

const api = useApi()
const { t } = useI18n()
const toast = inject<any>('toast')!
const modal = inject<any>('modal')!

const wallets = ref<WalletResponse[]>([])
const hdStatus = ref<HdStatusResponse | null>(null)
const loading = ref(true)
const error = ref<string | null>(null)
const balanceLoading = ref(false)
const addressBalances = ref<AddressBalanceItem[]>([])
const totalBalance = ref<number | null>(null)

// HD wallet form state
const showCreateHd = ref(false)
const showImportMnemonic = ref(false)
const showMnemonic = ref(false)
const hdPassword = ref('')
const hdLabel = ref('My HD Wallet')
const hdAddressCount = ref(5)
const hdMnemonic = ref('')
const importMnemonicPhrase = ref('')
const unlocked = ref(false)

const hdChildren = computed(() => wallets.value.filter(w => w.wallet_type === 'hd_child'))

async function loadAll() {
  loading.value = true; error.value = null
  try { wallets.value = await api.listWallets(); hdStatus.value = await api.getHdStatus() }
  catch (e: any) { console.error('Failed to load wallets:', e); error.value = e.message || t('wallets.loadFailed') }
  finally { loading.value = false }
}

async function loadBalances() {
  balanceLoading.value = true
  try {
    const [bal, items] = await Promise.all([api.getWalletBalance(), api.getAddressBalances()])
    totalBalance.value = bal.total_balance_shannons
    addressBalances.value = items
  } catch (e: any) {
    console.error('Failed to load wallet balances:', e);
    toast.error(e.message || t('wallets.loadFailed'))
  } finally {
    balanceLoading.value = false
  }
}

function applyRefreshResult(result: { total_balance_shannons: number; address_balances: AddressBalanceItem[] }) {
  totalBalance.value = result.total_balance_shannons
  addressBalances.value = result.address_balances
}

async function doRefreshHd(password?: string) {
  loading.value = true
  balanceLoading.value = true
  error.value = null
  try {
    const result = await api.refreshHdWallet(password ? { password } : {})
    unlocked.value = true
    await loadAll()
    if (hdStatus.value) {
      hdStatus.value = {
        ...hdStatus.value,
        label: result.keystore.label,
        address_count: result.keystore.address_count,
      }
    }
    applyRefreshResult(result)
  } catch (e: any) {
    console.error('Failed to refresh HD wallet:', e);
    error.value = e.message || t('wallets.loadFailed')
    throw e
  } finally {
    loading.value = false
    balanceLoading.value = false
  }
}

async function tryRestoreSession() {
  if (!hdStatus.value?.keystore_exists) return
  try {
    const session = await api.getWalletSession()
    if (!session.active) return
    unlocked.value = true
    await doRefreshHd()
  } catch (e) {
    console.warn('Session restore failed:', e);
  }
}

async function createHdWallet() {
  if (!hdPassword.value || !hdLabel.value) { toast.warning(t('wallets.fillRequired')); return }
  try {
    const result = await api.createHdWallet({ label: hdLabel.value, password: hdPassword.value, address_count: hdAddressCount.value })
    hdMnemonic.value = result.mnemonic; showMnemonic.value = true; showCreateHd.value = false
    unlocked.value = true; hdPassword.value = ''
    toast.success(t('wallets.hdCreated'))
    await doRefreshHd()
  } catch (e: any) { console.error('Failed to create HD wallet:', e); toast.error(e.message || t('wallets.importFailed')) }
}

async function importFromMnemonic() {
  if (!importMnemonicPhrase.value.trim() || !hdPassword.value || !hdLabel.value) {
    toast.warning(t('wallets.fillRequired')); return
  }
  try {
    await api.importMnemonic({
      mnemonic: importMnemonicPhrase.value.trim(),
      label: hdLabel.value,
      password: hdPassword.value,
      address_count: hdAddressCount.value,
    })
    toast.success(t('wallets.importSuccess'))
    showImportMnemonic.value = false; unlocked.value = true
    hdPassword.value = ''; importMnemonicPhrase.value = ''
    await doRefreshHd()
  } catch (e: any) { console.error('Failed to import from mnemonic:', e); toast.error(e.message || t('wallets.importFailed')) }
}

async function refreshWallet() {
  if (unlocked.value) {
    try {
      await doRefreshHd()
      toast.success(t('wallets.refreshed'))
    } catch (e) {
      console.error('Failed to refresh wallet (unlocked path):', e);
    }
    return
  }
  try {
    const session = await api.getWalletSession()
    if (session.active) {
      unlocked.value = true
      await doRefreshHd()
      toast.success(t('wallets.refreshed'))
      return
    }
  } catch (e) {
    console.warn('Session check failed, falling through to password modal:', e);
  }
  showRefreshModal()
}

function showRefreshModal() {
  const pw = ref('')
  const err = ref('')
  modal.show({
    title: t('wallets.refresh'),
    content: {
      setup() {
        return () => h('div', { class: 'unlock-dialog' }, [
          h('p', { class: 'unlock-hint' }, t('wallets.refreshHint')),
          h('input', {
            type: 'password',
            class: 'input unlock-input',
            value: pw.value,
            onInput: (e: Event) => { pw.value = (e.target as HTMLInputElement).value; err.value = '' },
            placeholder: t('wallets.passwordPlaceholder'),
            autofocus: true,
          }),
          h('p', { class: 'unlock-error', style: { display: err.value ? 'block' : 'none' } }, err.value),
        ])
      },
    },
    confirmText: t('wallets.refresh'),
    onConfirm: async () => {
      if (!pw.value) {
        err.value = t('wallets.fillRequired')
        return
      }
      try {
        await doRefreshHd(pw.value)
        toast.success(t('wallets.refreshed'))
      } catch (e: any) {
        console.error('Failed to refresh wallet:', e);
        err.value = e.message || t('wallets.loadFailed')
      }
    },
    onCancel: () => modal.hide(),
  })
}

function showDeriveModalFn() {
  if (unlocked.value) {
    void deriveMoreAddresses()
    return
  }
  const pw = ref('')
  const err = ref('')
  modal.show({
    title: t('wallets.deriveMore'),
    content: {
      setup() {
        return () => h('div', { class: 'unlock-dialog' }, [
          h('div', { class: 'unlock-icon-wrap' }, [
            h('span', { class: 'unlock-icon' }, '➕'),
          ]),
          h('p', { class: 'unlock-hint' }, t('wallets.deriveMoreHint')),
          h('input', {
            type: 'password',
            class: 'input unlock-input',
            value: pw.value,
            onInput: (e: Event) => { pw.value = (e.target as HTMLInputElement).value; err.value = '' },
            placeholder: t('wallets.passwordPlaceholder'),
          }),
          h('p', { class: 'unlock-error', style: { display: err.value ? 'block' : 'none' } }, err.value),
        ])
      },
    },
    confirmText: t('common.confirm'),
    onConfirm: async () => {
      if (!pw.value) { err.value = t('wallets.fillRequired'); return }
      try {
        await deriveMoreAddresses(pw.value)
      } catch (e: any) { console.error('Failed to derive addresses:', e); err.value = e.message || t('wallets.importFailed') }
    },
    onCancel: () => modal.hide(),
  })
}

async function deriveMoreAddresses(password?: string) {
  await api.deriveMoreAddresses(password ? { password, count: 3 } : { count: 3 })
  toast.success(t('wallets.importSuccess'))
  await doRefreshHd(password)
}

function showMnemonicRevealModal() {
  const pw = ref('')
  const err = ref('')
  modal.show({
    title: t('wallets.showMnemonicTitle'),
    content: {
      setup() {
        return () => h('div', { class: 'unlock-dialog' }, [
          h('div', { class: 'unlock-icon-wrap' }, [
            h('span', { class: 'unlock-icon' }, '🔑'),
          ]),
          h('p', { class: 'unlock-hint' }, t('wallets.showMnemonicHint')),
          h('input', {
            type: 'password',
            class: 'input unlock-input',
            value: pw.value,
            onInput: (e: Event) => { pw.value = (e.target as HTMLInputElement).value; err.value = '' },
            placeholder: t('wallets.passwordPlaceholder'),
            autofocus: true,
          }),
          h('p', { class: 'unlock-error', style: { display: err.value ? 'block' : 'none' } }, err.value),
        ])
      },
    },
    confirmText: t('common.confirm'),
    onConfirm: async () => {
      if (!pw.value) {
        err.value = t('wallets.fillRequired')
        return
      }
      try {
        const result = await api.revealMnemonic(pw.value)
        hdMnemonic.value = result.mnemonic
        showMnemonic.value = true
        modal.hide()
      } catch (e: any) {
        console.error('Failed to reveal mnemonic:', e);
        err.value = e.message || t('wallets.revealMnemonicFailed')
      }
    },
    onCancel: () => modal.hide(),
  })
}

async function deleteHdWallet() {
  const ok = await modal.confirm(t('wallets.deleteHdWarning'), {
    title: t('wallets.deleteHdTitle'),
    danger: true,
    confirmText: t('wallets.deleteHdConfirm'),
  })
  if (!ok) return
  try {
    await api.deleteHdWallet()
    toast.success(t('wallets.deleteSuccess'))
    unlocked.value = false
    totalBalance.value = null
    addressBalances.value = []
    await loadAll()
  } catch (e: any) { console.error('Failed to delete HD wallet:', e); toast.error(e.message || t('wallets.deleteFailed')) }
}

function balanceFor(id: number): number | null {
  if (balanceLoading.value) return null
  return addressBalances.value.find(a => a.wallet.id === id)?.balance_shannons ?? 0
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text)
    toast.success(t('common.copied'))
  } catch {
    console.warn('Clipboard API unavailable, using fallback');
    const ta = document.createElement('textarea')
    ta.value = text; ta.style.position = 'fixed'; ta.style.opacity = '0'
    document.body.appendChild(ta); ta.select(); document.execCommand('copy'); document.body.removeChild(ta)
    toast.success(t('common.copied'))
  }
}

onMounted(async () => {
  await loadAll()
  await tryRestoreSession()
  await loadBalances()
})
</script>

<template>
  <div class="page-wallets">
    <div class="page-header">
      <h2 class="page-title">
        {{ t('wallets.title') }}
      </h2>
    </div>

    <!-- Mnemonic display (shown once after creation) -->
    <div
      v-if="showMnemonic"
      class="card mnemonic-card"
    >
      <div class="card-header">
        <h3>{{ t('wallets.mnemonicTitle') }}</h3>
        <button
          class="btn btn-primary btn-sm"
          @click="showMnemonic = false"
        >
          {{ t('common.confirm') }}
        </button>
      </div>
      <p class="mnemonic-warning">
        {{ t('wallets.mnemonicWarning') }}
      </p>
      <pre class="mnemonic-words">{{ hdMnemonic }}</pre>
    </div>

    <!-- HD Wallet Section -->
    <div class="card hd-card">
      <div class="card-header">
        <h3>{{ t('wallets.hdWallet') }}</h3>
        <div class="card-actions">
          <!-- No wallet: create / import mnemonic -->
          <template v-if="!hdStatus?.keystore_exists">
            <button
              class="btn btn-primary btn-sm"
              @click="showCreateHd = !showCreateHd; showImportMnemonic = false"
            >
              {{ t('wallets.createHD') }}
            </button>
            <button
              class="btn btn-outline btn-sm"
              @click="showImportMnemonic = !showImportMnemonic; showCreateHd = false"
            >
              {{ t('wallets.importMnemonic') }}
            </button>
          </template>
          <!-- Wallet exists: Refresh and Show Mnemonic always available -->
          <template v-else>
            <button
              class="btn btn-outline btn-sm"
              @click="refreshWallet"
            >
              {{ t('wallets.refresh') }}
            </button>
            <button
              class="btn btn-outline btn-sm"
              @click="showMnemonicRevealModal"
            >
              {{ t('wallets.showMnemonic') }}
            </button>
          </template>
          <!-- Delete (danger) — always visible when wallet exists -->
          <button
            v-if="hdStatus?.keystore_exists"
            class="btn btn-danger btn-sm"
            @click="deleteHdWallet"
          >
            {{ t('wallets.deleteHd') }}
          </button>
        </div>
      </div>

      <!-- Create HD Wallet form -->
      <div
        v-if="showCreateHd"
        class="form-import"
      >
        <div class="field">
          <label class="caption">{{ t('wallets.label') }}</label>
          <input
            v-model="hdLabel"
            class="input"
            :placeholder="t('wallets.labelPlaceholder')"
          >
        </div>
        <div class="field">
          <label class="caption">{{ t('wallets.password') }}</label>
          <input
            v-model="hdPassword"
            type="password"
            class="input"
            :placeholder="t('wallets.passwordPlaceholder')"
          >
        </div>
        <div class="field field-count">
          <label class="caption">{{ t('wallets.addressCount') }}</label>
          <input
            v-model.number="hdAddressCount"
            type="number"
            min="1"
            max="50"
            class="input"
          >
        </div>
        <button
          class="btn btn-primary btn-sm"
          @click="createHdWallet"
        >
          {{ t('wallets.createHD') }}
        </button>
      </div>

      <!-- Import Mnemonic form -->
      <div
        v-if="showImportMnemonic"
        class="form-import form-import-grid"
      >
        <div class="form-import-col">
          <div class="field">
            <label class="caption">{{ t('wallets.label') }}</label>
            <input
              v-model="hdLabel"
              class="input"
              :placeholder="t('wallets.labelPlaceholder')"
            >
          </div>
          <div class="field">
            <label class="caption">{{ t('wallets.password') }}</label>
            <input
              v-model="hdPassword"
              type="password"
              class="input"
              :placeholder="t('wallets.passwordPlaceholder')"
            >
          </div>
          <div class="field field-count">
            <label class="caption">{{ t('wallets.addressCount') }}</label>
            <input
              v-model.number="hdAddressCount"
              type="number"
              min="1"
              max="50"
              class="input"
            >
          </div>
        </div>
        <div class="form-import-col">
          <div class="field">
            <label class="caption">{{ t('wallets.mnemonicTitle') }}</label>
            <textarea
              v-model="importMnemonicPhrase"
              class="input textarea"
              :placeholder="t('wallets.mnemonicPlaceholder')"
              rows="3"
            />
          </div>
        </div>
        <button
          class="btn btn-primary btn-sm form-import-submit"
          @click="importFromMnemonic"
        >
          {{ t('wallets.importMnemonic') }}
        </button>
      </div>

      <!-- HD summary -->
      <div
        v-if="hdStatus?.keystore_exists"
        class="hd-summary"
      >
        <div class="summary-item">
          <span class="summary-label">{{ t('wallets.totalBalance') }}</span>
          <span class="summary-value">{{ balanceLoading ? '...' : totalBalance !== null ? formatCKB(totalBalance) : '--' }}</span>
        </div>
        <div class="summary-item">
          <span class="summary-label">{{ t('wallets.addressCount') }}</span><span class="summary-value">{{ hdStatus.address_count }}</span>
        </div>
        <div class="summary-item">
          <span class="summary-label">{{ t('wallets.label') }}</span><span class="summary-value">{{ hdStatus.label || '--' }}</span>
        </div>
      </div>
      <div
        v-else-if="!showCreateHd && !showImportMnemonic"
        class="text-muted"
        style="padding:var(--space-md)"
      >
        {{ t('wallets.noHDWallet') }}
      </div>
    </div>

    <!-- Address Balances -->
    <div
      v-if="hdChildren.length"
      class="card addr-card"
      style="margin-top:var(--space-xl)"
    >
      <div class="card-header">
        <h3>{{ t('wallets.perAddress') }}</h3>
      </div>
      <div class="address-list">
        <div
          v-for="w in hdChildren"
          :key="w.id"
          class="address-row"
        >
          <span class="addr-index">#{{ w.derivation_index }}</span>
          <code
            class="font-mono addr-address copyable"
            :title="w.ckb_address"
            @click="copyToClipboard(w.ckb_address)"
          >{{ truncateAddress(w.ckb_address, 30, 20) }}</code>
          <span class="addr-derivation">m/44'/309'/0'/0/{{ w.derivation_index }}</span>
          <span class="addr-balance">{{ balanceFor(w.id) === null ? '...' : formatCKB(balanceFor(w.id)!) }}</span>
        </div>
      </div>
      <div class="derive-more-bar">
        <button
          class="btn-add"
          :title="t('wallets.deriveMore')"
          @click="showDeriveModalFn"
        >
          +
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.page-wallets { max-width: 1200px; margin: 0 auto; }
.page-header { margin-bottom: var(--space-xl); }
.page-title { font-size: var(--fs-h2); font-weight: var(--fw-h2); color: var(--text-primary); }

.card { background: var(--bg-card); border-radius: var(--radius-lg); border: 1px solid var(--border-light); box-shadow: var(--shadow-base); padding: var(--space-xl); }
.card-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--space-lg); }
.card-header h3 { font-size: var(--fs-h3); font-weight: var(--fw-h3); margin: 0; }
.card-actions { display: flex; gap: var(--space-xs); }

.mnemonic-card { border-color: var(--warning-500); margin-bottom: var(--space-xl); }
.mnemonic-warning { color: var(--danger); font-weight: 600; margin-bottom: var(--space-md); }
.mnemonic-words { background: var(--bg-surface); border: 1px solid var(--border-light); border-radius: var(--radius-md); padding: var(--space-lg); font-size: var(--fs-h3); font-family: var(--font-mono); word-spacing: var(--space-md); line-height: 1.8; text-align: center; }

.hd-summary { display: flex; gap: var(--space-xl); margin-top: var(--space-md); }
.summary-item { display: flex; flex-direction: column; gap: 2px; }
.summary-label { font-size: var(--fs-small); color: var(--text-secondary); }
.summary-value { font-size: var(--fs-h3); font-weight: var(--fw-h3); color: var(--text-primary); }
.form-import {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
  margin: 0 auto var(--space-lg);
  align-items: center;
  width: 100%;
  max-width: 480px;
}
.form-import-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: var(--space-md) var(--space-xl);
  align-items: start;
  max-width: 960px;
}
.form-import-col {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
  min-width: 0;
}
.form-import-submit {
  grid-column: 1 / -1;
  justify-self: center;
}
.form-import .textarea { resize: none; padding: var(--space-sm); height: auto; min-height: 88px; }
.form-import .field { width: 100%; max-width: none; }
.form-import:not(.form-import-grid) .field { width: 100%; }

@media (max-width: 768px) {
  .form-import-grid {
    grid-template-columns: 1fr;
  }
}

.field { display: flex; flex-direction: column; gap: 4px; }
.field-count {
  flex-shrink: 0;
}
.field-count .caption {
  white-space: nowrap;
}
.field-count .input {
  width: 100%;
  max-width: 120px;
}
.caption { font-size: var(--fs-small); color: var(--text-secondary); font-weight: 500; }

.address-list { display: flex; flex-direction: column; }
.address-row { display: flex; align-items: center; gap: var(--space-md); padding: var(--space-sm) 0; border-bottom: 1px solid var(--border-light); font-size: var(--fs-body); }
.address-row:last-child { border-bottom: none; }
.addr-index { font-weight: 600; color: var(--text-secondary); min-width: 32px; }
.addr-address { flex: 1; font-size: var(--fs-caption); }
.copyable { cursor: pointer; transition: color var(--transition-base); }
.copyable:hover { color: var(--primary-500); }
.addr-derivation { font-size: var(--fs-small); color: var(--text-disabled); min-width: 160px; }

.derive-more-bar { margin-top: var(--space-md); padding-top: var(--space-md); border-top: 1px solid var(--border-light); display: flex; align-items: center; flex-wrap: wrap; gap: var(--space-sm); }
.btn-add { width: 28px; height: 28px; border-radius: 50%; border: 1px dashed var(--border-dark); background: transparent; color: var(--text-secondary); font-size: var(--fs-h3); cursor: pointer; display: flex; align-items: center; justify-content: center; transition: all var(--transition-base); }
.btn-add:hover { border-color: var(--primary-500); color: var(--primary-500); }
.addr-balance { font-weight: 600; color: var(--primary-500); min-width: 100px; text-align: right; }

.btn { display: inline-flex; align-items: center; gap: var(--space-xs); padding: 0 var(--space-md); height: 36px; border: none; border-radius: var(--radius-md); font-size: var(--fs-body); font-family: inherit; cursor: pointer; transition: all var(--transition-base); font-weight: 500; white-space: nowrap; }
.btn-primary { background: var(--primary-500); color: #fff; }
.btn-primary:hover:not(:disabled) { background: var(--primary-400); }
.btn-outline { background: transparent; color: var(--primary-500); border: 1px solid var(--primary-500); }
.btn-outline:hover { background: var(--primary-50); }
.btn-danger { background: var(--danger); color: #fff; }
.btn-danger:hover:not(:disabled) { background: #ff7875; }
.btn-sm { height: 28px; font-size: var(--fs-small); padding: 0 var(--space-sm); }

.input { height: 36px; padding: 0 var(--space-sm); border: 1px solid var(--border-dark); border-radius: var(--radius-md); font-size: var(--fs-body); color: var(--text-primary); background: var(--bg-card); outline: none; }
.input:focus { border-color: var(--primary-500); box-shadow: 0 0 0 2px rgba(24,144,255,0.2); }

.text-muted { color: var(--text-disabled); font-size: var(--fs-body); }
</style>

<!-- Global styles for modal dialogs (teleported to body, scoped won't reach) -->
<style>
.unlock-dialog { display: flex; flex-direction: column; align-items: center; gap: var(--space-sm); }
.unlock-icon-wrap { margin-bottom: 0; }
.unlock-icon { font-size: 40px; line-height: 1; }
.unlock-hint { color: var(--text-secondary); font-size: var(--fs-body); line-height: 1.6; text-align: center; margin: 0; }
.unlock-input { width: 100%; height: 40px; padding: 0 var(--space-md); border: 1px solid var(--border-dark); border-radius: 6px; font-size: var(--fs-body); color: var(--text-primary); background: var(--bg-card); outline: none; box-sizing: border-box; transition: border-color 0.2s, box-shadow 0.2s; }
.unlock-input:focus { border-color: var(--primary-500); box-shadow: 0 0 0 2px rgba(24,144,255,0.2); }
.unlock-error { color: var(--danger); font-size: var(--fs-small); margin: 0; width: 100%; }
</style>
