<script setup lang="ts">
import { ref, reactive, computed, onMounted, inject, h } from "vue";
import { useRouter } from "vue-router";
import { useApi } from "@/composables/useApi";
import { useI18n } from "@/composables/useI18n";
import {
  truncateAddress,
  formatCKB,
  formatAPY,
  explorerTxUrl,
} from "@/utils/format";
import StatusTag from "@/components/ui/StatusTag.vue";
import MatchOrderForm from "@/components/ui/MatchOrderForm.vue";
import DataTable, { type ColumnDef } from "@/components/ui/DataTable.vue";
import EmptyState from "@/components/ui/EmptyState.vue";
import type {
  OrderScanItem,
  MatchOrderRequest,
  SignerWalletItem,
  MatchReadiness,
} from "@/types/api";

const api = useApi();
const { t } = useI18n();
const router = useRouter();
const toast = inject<any>("toast")!;
const modal = inject<any>("modal")!;

const orders = ref<OrderScanItem[]>([]);
const loading = ref(false);
const scanned = ref(false);
const error = ref<string | null>(null);
const network = ref("testnet");

const matchPhase = ref<"select" | "opening" | "waiting">("select");
const matchForm = ref<MatchOrderRequest & { tx_hash: string }>({
  tx_hash: "",
  order_output_index: 0,
  seller_address: "",
});

// Per-order readiness state
const readiness = reactive<Record<string, MatchReadiness | null>>({});
const pollingTimers: Record<string, ReturnType<typeof setInterval>> = {};

const columns = computed<ColumnDef[]>(() => [
  {
    key: "tx_hash",
    label: t("common.txHash"),
    align: "center",
    width: "155px",
  },
  {
    key: "fiber_pubkey",
    label: t("orders.buyerFiberPubkey"),
    align: "center",
    width: "130px",
  },
  {
    key: "channel_capacity",
    label: t("common.capacity"),
    sortable: true,
    align: "center",
    width: "120px",
  },
  {
    key: "annualized_yield",
    label: t("common.annualizedYield"),
    align: "center",
    width: "65px",
  },
  {
    key: "match_status",
    label: t("orders.matchStatus"),
    align: "center",
    width: "145px",
  },
  {
    key: "actions",
    label: t("common.actions"),
    align: "center",
    width: "95px",
  },
]);

async function scanOrders() {
  loading.value = true;
  error.value = null;
  try {
    orders.value = await api.scanOrders();
    scanned.value = true;
    loading.value = false;
    // Fire all readiness checks in parallel — each updates the reactive
    // readiness map independently so rows appear progressively.
    await Promise.allSettled(
      orders.value.map((o) => fetchReadiness(o.tx_hash)),
    );
  } catch (e: any) {
    console.error("Failed to scan orders:", e);
    error.value = e.message || t("orders.scanFailed");
    toast.error(error.value!);
    loading.value = false;
  }
}

async function copyToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success(t("common.copied"));
  } catch {
    console.warn("Clipboard API unavailable, using fallback");
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

// ── Per-order readiness ────────────────────────────────────────────

async function fetchReadiness(txHash: string) {
  try {
    readiness[txHash] = await api.getMatchReadiness(txHash);
  } catch {
    readiness[txHash] = null;
  }
}

function startPolling(txHash: string) {
  stopPolling(txHash);
  fetchReadiness(txHash);
  pollingTimers[txHash] = setInterval(() => fetchReadiness(txHash), 3000);
}

function stopPolling(txHash: string) {
  if (pollingTimers[txHash]) {
    clearInterval(pollingTimers[txHash]);
    delete pollingTimers[txHash];
  }
}

function isChannelBeingCreated(txHash: string): boolean {
  return !!readiness[txHash]?.pending_channel;
}

async function connectPeerForOrder(txHash: string, pubkey: string) {
  const ok = await modal.confirm(
    t("orders.confirmConnectPeer", { pubkey: truncateAddress(pubkey, 12, 8) }),
    { title: t("orders.peerConnect"), confirmText: t("orders.peerConnect") },
  );
  if (!ok) return;
  try {
    await api.connectToPeer(pubkey);
    toast.success(t("orders.peerConnectSuccess"));
    startPolling(txHash);
  } catch (e: any) {
    toast.error(e.message || t("orders.peerConnectFailed"));
  }
}

async function createChannelForOrder(txHash: string) {
  const r = readiness[txHash];
  const cap = r?.required_capacity
    ? `${(r.required_capacity / 100_000_000).toFixed(0)} CKB`
    : "—";
  const ok = await modal.confirm(
    t("orders.confirmCreateChannel", { peer: truncateAddress(r?.fiber_pubkey || "", 12, 8), capacity: cap }),
    { title: t("orders.createChannel"), confirmText: t("orders.createChannel") },
  );
  if (!ok) return;
  try {
    const result = await api.createOrderChannel(txHash);
    toast.success(
      t("orders.channelCreating", {
        id: result.temporary_channel_id.slice(0, 16),
      }),
    );
    router.push("/channels");
  } catch (e: any) {
    toast.error(e.message || t("orders.channelCreateFailed"));
  }
}

function showProgressModal() {
  modal.show({
    title: t("orders.matchTitle"),
    content: {
      setup() {
        return () =>
          h("div", { class: "match-progress" }, [
            h("span", { class: "spinner" }),
            h(
              "p",
              { class: "progress-text" },
              matchPhase.value === "opening"
                ? t("orders.matchStepChannel")
                : t("orders.matchStepWaiting"),
            ),
          ]);
      },
    },
    confirmText: null,
    cancelText: null,
    onConfirm: () => {},
    onCancel: () => {},
  });
}

async function showMatchModal(order: OrderScanItem) {
  const modalData = reactive({
    wallets: [] as SignerWalletItem[],
    walletsLoading: true,
    needsUnlock: false,
    unlocking: false,
    unlockError: '',
  });

  matchForm.value = {
    tx_hash: order.tx_hash,
    order_output_index: order.output_index,
    seller_address: "",
  };
  matchPhase.value = "select";

  async function handleUnlock(password: string) {
    modalData.unlocking = true;
    modalData.unlockError = '';
    try {
      await api.unlockWallet({ password });
      modalData.needsUnlock = false;
      modal.confirmText.value = t("orders.matchConfirm");
      // Reload wallets — signer is now unlocked.
      const wallets = await api.getSignerWallets();
      modalData.wallets.splice(0, modalData.wallets.length, ...wallets);
      if (!matchForm.value.seller_address && wallets.length > 0) {
        matchForm.value = {
          ...matchForm.value,
          seller_address: wallets[0].ckb_address,
        };
      }
      // Auto-proceed with the match now that the wallet is unlocked.
      triggerMatch();
    } catch (e: any) {
      modalData.unlockError = e.message || t("orders.matchFailed");
    } finally {
      modalData.unlocking = false;
    }
  }

  async function triggerMatch() {
    const f = matchForm.value;
    if (!f.seller_address) {
      toast.warning(t("orders.fillAllParams"));
      return;
    }

    modal.hide();
    await new Promise((r) => setTimeout(r, 200));
    matchPhase.value = "opening";
    showProgressModal();

    const phaseTimer = setTimeout(() => {
      matchPhase.value = "waiting";
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
      console.error("Failed to match order:", e);
      clearTimeout(phaseTimer);
      modal.hide();
      toast.error(e.message || t("orders.matchFailed"));
    }
  }

  modal.show({
    title: t("orders.matchTitle"),
    content: MatchOrderForm,
    contentProps: {
      modelValue: matchForm.value,
      get wallets() { return modalData.wallets },
      get walletsLoading() { return modalData.walletsLoading },
      get needsUnlock() { return modalData.needsUnlock },
      get unlocking() { return modalData.unlocking },
      get unlockError() { return modalData.unlockError },
      "onUpdate:modelValue": (v: typeof matchForm.value) => {
        matchForm.value = v;
      },
      "onUnlock": handleUnlock,
    },
    confirmText: t("orders.matchConfirm"),
    onConfirm: async () => {
      const f = matchForm.value;
      if (!f.seller_address) {
        toast.warning(t("orders.fillAllParams"));
        return Promise.reject();
      }

      // Check if the wallet is locked. If so, switch to password prompt
      // IN the match dialog instead of immediately failing.
      try {
        const session = await api.getWalletSession();
        if (!session.active) {
          modalData.needsUnlock = true;
          modal.confirmText.value = null;
          return Promise.reject();
        }
      } catch {
        // Can't check — proceed; the match endpoint will handle the error.
      }

      await triggerMatch();
      return Promise.reject(); // triggerMatch handles its own flow
    },
    onCancel: () => modal.hide(),
  });

  // Load wallets in the background — always show addresses regardless of lock state.
  api.getSignerWallets()
    .then((wallets) => {
      modalData.wallets.splice(0, modalData.wallets.length, ...wallets);
      if (!matchForm.value.seller_address && wallets.length > 0) {
        matchForm.value = {
          ...matchForm.value,
          seller_address: wallets[0].ckb_address,
        };
      }
    })
    .catch((e) => {
      console.error("Failed to load signer wallets:", e);
    })
    .finally(() => {
      modalData.walletsLoading = false;
    });
}

onMounted(async () => {
  api
    .getServerInfo()
    .then((info) => {
      network.value = info.network;
    })
    .catch((e) => {
      console.error("Failed to get server info:", e);
    });
  await scanOrders();
});
</script>

<template>
  <div class="page-orders">
    <div class="toolbar">
      <div class="toolbar-spacer" />
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
        >{{ truncateAddress(String(value), 12, 8) }}</code>
      </template>
      <template #cell-channel_capacity="{ value }">
        {{ formatCKB(Number(value)) }}
      </template>
      <template #cell-annualized_yield="{ row }">
        {{
          formatAPY(
            Number(row.shannons_per_block),
            Number(row.channel_capacity),
          )
        }}
      </template>
      <template #cell-match_status="{ row }">
        <div class="match-status-cell">
          <StatusTag
            v-if="readiness[row.tx_hash]?.peer_connected"
            status="live"
            :label="t('orders.peerConnected')"
          />
          <StatusTag
            v-else-if="readiness[row.tx_hash] && !readiness[row.tx_hash]!.peer_connected"
            status="destroyed"
            :label="t('orders.peerNotConnected')"
          />
          <span
            v-else
            class="inline-skeleton"
          >
            <span class="inline-skeleton-bar" />
          </span>
          <span class="status-sep">/</span>
          <StatusTag
            v-if="readiness[row.tx_hash]?.compatible_channel"
            status="live"
            :label="t('orders.channelAvailable')"
          />
          <StatusTag
            v-else-if="readiness[row.tx_hash]?.pending_channel"
            status="pending"
            :label="t('orders.channelCreatingShort')"
          />
          <StatusTag
            v-else-if="
              readiness[row.tx_hash]
                && !readiness[row.tx_hash]!.compatible_channel
                && !readiness[row.tx_hash]!.pending_channel
            "
            status="pending"
            :label="t('orders.channelNone')"
          />
          <span
            v-else
            class="inline-skeleton"
          >
            <span class="inline-skeleton-bar" />
          </span>
        </div>
      </template>
      <template #cell-actions="{ row }">
        <button
          v-if="!readiness[row.tx_hash]?.peer_connected"
          class="btn btn-sm btn-outline"
          @click="
            connectPeerForOrder(
              row.tx_hash,
              readiness[row.tx_hash]?.fiber_pubkey || row.fiber_pubkey,
            )
          "
        >
          {{ t("orders.peerConnect") }}
        </button>
        <button
          v-else-if="isChannelBeingCreated(row.tx_hash)"
          class="btn btn-sm btn-primary"
          disabled
        >
          {{ t("orders.channelCreatingShort") }}
        </button>
        <button
          v-else-if="!readiness[row.tx_hash]?.compatible_channel"
          class="btn btn-sm btn-primary"
          @click="createChannelForOrder(row.tx_hash)"
        >
          {{ t("orders.createChannel") }}
        </button>
        <button
          v-else
          class="btn btn-sm btn-primary"
          @click="showMatchModal(row)"
        >
          {{ t("orders.match") }}
        </button>
      </template>
    </DataTable>
  </div>
</template>

<style scoped>
.match-status-cell {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
}
.status-sep {
  color: var(--text-disabled);
  font-size: var(--fs-small);
}
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
.toolbar {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  margin-bottom: var(--space-xl);
}
.toolbar-spacer {
  flex: 1;
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
.btn-outline {
  background: transparent;
  color: var(--primary-500);
  border: 1px solid var(--primary-500);
}
.btn-outline:hover {
  background: var(--primary-50);
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

/* Inline skeleton for readiness placeholder */
.inline-skeleton {
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.inline-skeleton-bar {
  display: inline-block;
  width: 50px;
  height: 10px;
  border-radius: var(--radius-sm);
  background-color: var(--gray-100);
  background-image: linear-gradient(
    100deg,
    transparent 40%,
    rgba(255, 255, 255, 0.5) 50%,
    transparent 60%
  );
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.5s ease-in-out infinite;
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
.peer-status.connected {
  color: var(--success, #52c41a);
}
.peer-status.disconnected {
  color: var(--danger, #ff4d4f);
}
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
.peer-actions .btn-primary {
  background: var(--primary-500);
  color: #fff;
}
.peer-actions .btn-primary:hover:not(:disabled) {
  background: var(--primary-400);
}
.peer-actions .btn-default {
  background: var(--bg-card);
  color: var(--text-primary);
  border: 1px solid var(--border-dark);
}
.peer-actions .btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.peer-actions .btn-sm {
  height: 28px;
  font-size: var(--fs-small);
}
</style>
