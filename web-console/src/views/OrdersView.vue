<script setup lang="ts">
import { ref, onMounted, inject, h, watch } from "vue";
import { useApi } from "@/composables/useApi";
import { useI18n } from "@/composables/useI18n";
import { truncateAddress, formatCKB, formatAPY, explorerTxUrl } from "@/utils/format";
import MatchOrderForm from "@/components/ui/MatchOrderForm.vue";
import DataTable, { type ColumnDef } from "@/components/ui/DataTable.vue";
import EmptyState from "@/components/ui/EmptyState.vue";
import type { OrderScanItem, MatchOrderRequest, SignerWalletItem } from "@/types/api";

const api = useApi();
const { t } = useI18n();
const toast = inject<any>("toast")!;
const modal = inject<any>("modal")!;

const orders = ref<OrderScanItem[]>([]);
const loading = ref(false);
const scanned = ref(false);
const error = ref<string | null>(null);
const network = ref("testnet");
const signerWallets = ref<SignerWalletItem[]>([]);
const matchPhase = ref<'select' | 'opening' | 'waiting'>('select');
const matchForm = ref<MatchOrderRequest & { tx_hash: string }>({
  tx_hash: "",
  order_output_index: 0,
  seller_address: "",
});

const columns: ColumnDef[] = [
  { key: "tx_hash", label: t("common.txHash"), align: "center" },
  { key: "fiber_pubkey", label: t("orders.buyerFiberPubkey"), align: "center" },
  {
    key: "channel_capacity",
    label: t("common.capacity"),
    sortable: true,
    align: "center",
  },
  {
    key: "annualized_yield",
    label: t("common.annualizedYield"),
    align: "center",
  },
  { key: "actions", label: t("common.actions"), align: "center" },
];

async function scanOrders() {
  loading.value = true;
  error.value = null;
  try {
    orders.value = await api.scanOrders();
    scanned.value = true;
  } catch (e: any) {
    console.error('Failed to scan orders:', e);
    error.value = e.message || t("orders.scanFailed");
    toast.error(error.value!);
  } finally {
    loading.value = false;
  }
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(t("common.copied"));
  } catch {
    console.warn('Clipboard API unavailable, using fallback');
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
    toast.success(t("common.copied"));
  }
}

function showProgressModal() {
  modal.show({
    title: t('orders.matchTitle'),
    content: {
      setup() {
        return () =>
          h('div', { class: 'match-progress' }, [
            h('span', { class: 'spinner' }),
            h('p', { class: 'progress-text' },
              matchPhase.value === 'opening'
                ? t('orders.matchStepChannel')
                : t('orders.matchStepWaiting'),
            ),
          ])
      },
    },
    confirmText: ' ', // truthy but invisible (prevents default '确定')
    cancelText: ' ',  // truthy but invisible
    onConfirm: () => {},
    onCancel: () => {}, // no-op — prevents any close
  })
}

async function showConnectionCheck(order: OrderScanItem) {
  const buyerPubkey = order.fiber_pubkey
  const connected = ref(false)
  const checking = ref(true)

  async function doCheck() {
    checking.value = true
    try {
      const status = await api.checkPeerConnection(buyerPubkey)
      connected.value = status.connected
    } catch (e) {
      console.error('Failed to check peer connection:', e);
      connected.value = false
    } finally {
      checking.value = false
    }
  }

  async function doConnect() {
    checking.value = true
    try {
      await api.connectToPeer(buyerPubkey)
      toast.success(t('orders.peerConnectSuccess'))
      await doCheck()
    } catch (e: any) {
      console.error('Failed to connect to peer:', e);
      toast.error(e.message || t('orders.peerConnectFailed'))
      checking.value = false
    }
  }

  // Footer "连接" button component
  const ConnectBtn = {
    setup() {
      return () =>
        h('button', { class: 'btn btn-primary btn-sm', onClick: doConnect }, t('orders.peerConnect'))
    },
  }

  // Sync footer buttons with connection state
  const stopWatch = watch([connected, checking], () => {
    if (connected.value) {
      modal.confirmText.value = t('orders.peerContinue')
      modal.extra.value = undefined
    } else if (!checking.value) {
      modal.confirmText.value = null
      modal.extra.value = ConnectBtn
    } else {
      modal.confirmText.value = null
      modal.extra.value = undefined
    }
  })

  modal.show({
    title: t('orders.peerCheckTitle'),
    content: {
      setup() {
        return () =>
          h('div', { class: 'peer-check' }, [
            h('p', { class: 'peer-pubkey' }, [
              h('span', { class: 'peer-label' }, t('orders.buyerFiberPubkey') + ': '),
              h('code', { class: 'font-mono peer-key' }, truncateAddress(buyerPubkey, 20, 16)),
            ]),
            h('div', { class: 'peer-status-row' }, [
              checking.value
                ? h('span', { class: 'spinner peer-spinner' })
                : connected.value
                  ? h('span', { class: 'peer-status connected' }, '✓ ' + t('orders.peerConnected'))
                  : h('span', { class: 'peer-status disconnected' }, '✗ ' + t('orders.peerNotConnected')),
            ]),
          ])
      },
    },
    confirmText: null,
    cancelText: t('common.cancel'),
    onConfirm: () => {
      if (!connected.value) return
      stopWatch()
      modal.hide()
      showMatchModal(order)
    },
    onCancel: () => {
      stopWatch()
      modal.hide()
    },
  })

  doCheck()
}

async function showMatchModal(order: OrderScanItem) {
  // Load available HD wallet addresses for the seller selector.
  try {
    signerWallets.value = await api.getSignerWallets();
  } catch (e) {
    console.error('Failed to load signer wallets:', e);
    signerWallets.value = [];
  }

  matchForm.value = {
    tx_hash: order.tx_hash,
    order_output_index: order.output_index,
    seller_address:
      signerWallets.value.length > 0
        ? signerWallets.value[0].ckb_address
        : "",
  };
  matchPhase.value = 'select';

  modal.show({
    title: t("orders.matchTitle"),
    content: MatchOrderForm,
    contentProps: {
      modelValue: matchForm.value,
      wallets: signerWallets.value,
      "onUpdate:modelValue": (v: typeof matchForm.value) => {
        matchForm.value = v;
      },
    },
    confirmText: t("orders.matchConfirm"),
    onConfirm: async () => {
      const f = matchForm.value;
      if (!f.seller_address) {
        toast.warning(t("orders.fillAllParams"));
        return Promise.reject();
      }

      // Switch to progress modal (fixed, non-closable).
      modal.hide();
      await new Promise((r) => setTimeout(r, 200)); // let hide animation complete
      matchPhase.value = 'opening';
      showProgressModal();

      // Transition to "waiting" phase after a few seconds if still running.
      const phaseTimer = setTimeout(() => {
        matchPhase.value = 'waiting';
      }, 5000);

      try {
        const result = await api.matchOrder(f.tx_hash, {
          order_output_index: f.order_output_index,
          seller_address: f.seller_address,
        });
        clearTimeout(phaseTimer);
        modal.hide();
        toast.success(
          `${t("orders.matchSuccess")}! TX: ${truncateAddress(result.tx_hash)}`,
        );
        await scanOrders();
      } catch (e: any) {
        console.error('Failed to match order:', e);
        clearTimeout(phaseTimer);
        modal.hide();
        toast.error(e.message || t("orders.matchFailed"));
      }
    },
    onCancel: () => modal.hide(),
  });
}

onMounted(async () => {
  api.getServerInfo().then((info) => { network.value = info.network; }).catch((e) => { console.error('Failed to get server info:', e); });
  await scanOrders();
});
</script>

<template>
  <div class="page-orders">
    <div class="page-header">
      <h2 class="page-title">
        {{ t("orders.title") }}
      </h2>
      <button
        class="btn btn-primary"
        :disabled="loading"
        @click="scanOrders"
      >
        <span
          v-if="loading"
          class="spinner"
        />
        {{ loading ? t("orders.scanning") : t("orders.scan") }}
      </button>
    </div>
    <EmptyState
      v-if="error"
      icon="⚠️"
      :message="error"
      :action-label="t('common.retry')"
      @action="scanOrders"
    />
    <EmptyState
      v-else-if="scanned && !orders.length"
      icon="📋"
      :message="t('orders.noOrders')"
      :action-label="t('orders.rescan')"
      @action="scanOrders"
    />
    <DataTable
      v-else-if="scanned"
      :columns="columns"
      :rows="orders"
      :loading="loading"
    >
      <template #cell-tx_hash="{ value }">
        <a
          :href="explorerTxUrl(String(value), network)"
          target="_blank"
          rel="noopener noreferrer"
          class="tx-link font-mono"
          :title="String(value)"
        >{{ truncateAddress(String(value), 12, 8) }}</a>
      </template>
      <template #cell-fiber_pubkey="{ value }">
        <code
          class="font-mono copyable fiber-key-cell"
          :title="String(value)"
          @click="copyToClipboard(String(value))"
        >{{ truncateAddress(String(value), 20, 16) }}</code>
      </template>
      <template #cell-channel_capacity="{ value }">
        {{
          formatCKB(Number(value))
        }}
      </template>
      <template #cell-annualized_yield="{ row }">
        {{
          formatAPY(Number(row.shannons_per_block), Number(row.channel_capacity))
        }}
      </template>
      <template #cell-actions="{ row }">
        <button
          class="btn btn-sm btn-primary"
          @click="showConnectionCheck(row)"
        >
          {{ t("orders.match") }}
        </button>
      </template>
    </DataTable>
  </div>
</template>

<style scoped>
.page-orders {
  max-width: 1200px;
  margin: 0 auto;
}
.page-orders :deep(.data-table) {
  table-layout: fixed;
  width: 100%;
}
.page-orders :deep(.data-table th:nth-child(1)),
.page-orders :deep(.data-table td:nth-child(1)) {
  width: 20%;
  overflow: hidden;
}
.page-orders :deep(.data-table th:nth-child(2)),
.page-orders :deep(.data-table td:nth-child(2)) {
  width: 32%;
  overflow: hidden;
}
.page-orders :deep(.data-table th:nth-child(3)),
.page-orders :deep(.data-table td:nth-child(3)),
.page-orders :deep(.data-table th:nth-child(4)),
.page-orders :deep(.data-table td:nth-child(4)),
.page-orders :deep(.data-table th:nth-child(5)),
.page-orders :deep(.data-table td:nth-child(5)) {
  width: 14%;
}
.page-orders :deep(.data-table th),
.page-orders :deep(.data-table td) {
  text-align: center;
}
.page-orders :deep(.tx-link) {
  display: inline-block;
  max-width: 100%;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  vertical-align: middle;
  font-size: var(--fs-caption);
  color: var(--primary-500);
  text-decoration: none;
}
.page-orders :deep(.tx-link:hover) {
  color: var(--primary-400);
  text-decoration: underline;
}
.page-orders :deep(.fiber-key-cell) {
  display: inline-block;
  max-width: 100%;
  white-space: nowrap;
  vertical-align: middle;
  font-size: var(--fs-caption);
  cursor: pointer;
  transition: color var(--transition-base);
}
.page-orders :deep(.fiber-key-cell:hover) {
  color: var(--primary-500);
}
.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-xl);
}
.page-title {
  font-size: var(--fs-h2);
  font-weight: var(--fw-h2);
  line-height: var(--lh-h2);
  color: var(--text-primary);
}
.btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-xs);
  padding: 0 var(--space-md);
  height: 32px;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--fs-body);
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-base);
  font-weight: 500;
}
.btn-primary {
  background: var(--primary-500);
  color: #fff;
}
.btn-primary:hover:not(:disabled) {
  background: var(--primary-400);
}
.btn-primary:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.btn-sm {
  height: 28px;
  font-size: var(--fs-caption);
  padding: 0 var(--space-sm);
}
</style>

<!-- Global styles for progress modal (teleported to body, scoped won't reach) -->
<style>
.match-progress {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-lg);
  padding: var(--space-xl) 0;
}
.match-progress .spinner {
  display: inline-block;
  width: 40px;
  height: 40px;
  border: 3px solid var(--border-light);
  border-top-color: var(--primary-500);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}
.match-progress .progress-text {
  font-size: var(--fs-body);
  color: var(--text-secondary);
  text-align: center;
  margin: 0;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Peer connection check modal */
.peer-check {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-lg);
  padding: var(--space-md) 0;
}
.peer-pubkey {
  margin: 0;
  font-size: var(--fs-body);
  color: var(--text-secondary);
}
.peer-key {
  font-size: var(--fs-caption);
  word-break: break-all;
}
.peer-status-row {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 28px;
}
.peer-spinner {
  width: 24px;
  height: 24px;
  border-width: 2px;
}
.peer-status {
  font-size: var(--fs-body);
  font-weight: 500;
}
.peer-status.connected { color: var(--success, #52c41a); }
.peer-status.disconnected { color: var(--danger, #ff4d4f); }
.peer-actions {
  display: flex;
  gap: var(--space-sm);
  justify-content: center;
}
.peer-actions .btn {
  height: 32px;
  padding: 0 var(--space-md);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--fs-body);
  cursor: pointer;
}
.peer-actions .btn-primary { background: var(--primary-500); color: #fff; }
.peer-actions .btn-primary:hover:not(:disabled) { background: var(--primary-400); }
.peer-actions .btn-default { background: var(--bg-card); color: var(--text-primary); border: 1px solid var(--border-dark); }
.peer-actions .btn:disabled { opacity: 0.5; cursor: not-allowed; }
.peer-actions .btn-sm { height: 28px; font-size: var(--fs-small); }
</style>
