<script setup lang="ts">
import { ref, onMounted, inject } from "vue";
import { useApi } from "@/composables/useApi";
import { useI18n } from "@/composables/useI18n";
import { truncateAddress, formatCKB, formatAPY, explorerTxUrl } from "@/utils/format";
import type { ServerInfo } from "@/types/api";
import MatchOrderForm from "@/components/ui/MatchOrderForm.vue";
import DataTable, { type ColumnDef } from "@/components/ui/DataTable.vue";
import EmptyState from "@/components/ui/EmptyState.vue";
import type { OrderScanItem, MatchOrderRequest } from "@/types/api";

const api = useApi();
const { t } = useI18n();
const toast = inject<any>("toast")!;
const modal = inject<any>("modal")!;

const orders = ref<OrderScanItem[]>([]);
const loading = ref(false);
const scanned = ref(false);
const error = ref<string | null>(null);
const network = ref("testnet");
const matchForm = ref<MatchOrderRequest & { tx_hash: string }>({
  tx_hash: "",
  order_output_index: 0,
  seller_address: "",
  channel_outpoint_tx_hash: "",
  channel_outpoint_index: 0,
});

const columns: ColumnDef[] = [
  { key: "tx_hash", label: t("common.txHash"), align: "center" },
  {
    key: "channel_capacity",
    label: t("common.capacity"),
    sortable: true,
    align: "center",
  },
  {
    key: "shannons_per_block",
    label: t("common.ratePerBlock"),
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
    error.value = e.message || t("orders.scanFailed");
    toast.error(error.value!);
  } finally {
    loading.value = false;
  }
}

function showMatchModal(order: OrderScanItem) {
  matchForm.value = {
    tx_hash: order.tx_hash,
    order_output_index: order.output_index,
    seller_address: "",
    channel_outpoint_tx_hash: "",
    channel_outpoint_index: 0,
  };
  modal.show({
    title: t("orders.matchTitle"),
    content: MatchOrderForm,
    contentProps: {
      modelValue: matchForm.value,
      "onUpdate:modelValue": (v: typeof matchForm.value) => {
        matchForm.value = v;
      },
    },
    confirmText: t("orders.matchConfirm"),
    onConfirm: async () => {
      const f = matchForm.value;
      if (!f.seller_address || !f.channel_outpoint_tx_hash) {
        toast.warning(t("orders.fillAllParams"));
        return Promise.reject();
      }
      try {
        const result = await api.matchOrder(f.tx_hash, {
          order_output_index: f.order_output_index,
          seller_address: f.seller_address,
          channel_outpoint_tx_hash: f.channel_outpoint_tx_hash,
          channel_outpoint_index: f.channel_outpoint_index,
        });
        toast.success(
          `${t("orders.matchSuccess")}! TX: ${truncateAddress(result.tx_hash)}`,
        );
        modal.hide();
      } catch (e: any) {
        toast.error(e.message || t("orders.matchFailed"));
        throw e;
      }
    },
    onCancel: () => modal.hide(),
  });
}

onMounted(async () => {
  api.getServerInfo().then((info) => { network.value = info.network; }).catch(() => {});
  await scanOrders();
});
</script>

<template>
  <div class="page-orders">
    <div class="page-header">
      <h2 class="page-title">{{ t("orders.title") }}</h2>
      <button class="btn btn-primary" :disabled="loading" @click="scanOrders">
        <span v-if="loading" class="spinner" />
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
        >{{ truncateAddress(String(value), 20, 16) }}</a>
      </template>
      <template #cell-channel_capacity="{ value }">{{
        formatCKB(Number(value))
      }}</template>
      <template #cell-shannons_per_block="{ value }">{{
        Number(value).toLocaleString()
      }}</template>
      <template #cell-annualized_yield="{ row }">{{
        formatAPY(Number(row.shannons_per_block), Number(row.channel_capacity))
      }}</template>
      <template #cell-actions="{ row }"
        ><button class="btn btn-sm btn-primary" @click="showMatchModal(row)">
          {{ t("orders.match") }}
        </button></template
      >
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
  width: 36%;
  overflow: hidden;
}
.page-orders :deep(.data-table th:nth-child(2)),
.page-orders :deep(.data-table td:nth-child(2)),
.page-orders :deep(.data-table th:nth-child(3)),
.page-orders :deep(.data-table td:nth-child(3)),
.page-orders :deep(.data-table th:nth-child(4)),
.page-orders :deep(.data-table td:nth-child(4)),
.page-orders :deep(.data-table th:nth-child(5)),
.page-orders :deep(.data-table td:nth-child(5)) {
  width: 16%;
}
.page-orders :deep(.data-table th),
.page-orders :deep(.data-table td) {
  text-align: center;
}
.page-orders :deep(.tx-link) {
  display: inline-block;
  max-width: 100%;
  white-space: nowrap;
  vertical-align: middle;
  color: var(--primary-500);
  text-decoration: none;
}
.page-orders :deep(.tx-link:hover) {
  color: var(--primary-400);
  text-decoration: underline;
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
