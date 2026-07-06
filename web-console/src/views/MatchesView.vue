<script setup lang="ts">
import { ref, computed, onMounted, inject, h } from "vue";
import { useApi } from "@/composables/useApi";
import { useI18n } from "@/composables/useI18n";
import { truncateAddress, formatCKB, explorerTxUrl } from "@/utils/format";
import DataTable, { type ColumnDef } from "@/components/ui/DataTable.vue";
import StatusTag from "@/components/ui/StatusTag.vue";
import EmptyState from "@/components/ui/EmptyState.vue";
import type { TrackedMatch, MatchDetail } from "@/types/api";
import type { useTxConfirm } from "@/composables/useTxConfirm";

const api = useApi();
const { t } = useI18n();
const toast = inject<any>("toast")!;
const modal = inject<any>("modal")!;
const txConfirm = inject<ReturnType<typeof useTxConfirm>>("txConfirm")!;

const matches = ref<TrackedMatch[]>([]);
const loading = ref(true);
const error = ref<string | null>(null);
const filterStatus = ref<string>("");
const network = ref("testnet");
const signerLockHashes = ref<string[]>([]);
const minExtractionShannons = ref(0);

// ── Expand / fold state ──
const expandedRows = ref<Set<string>>(new Set());
const matchDetails = ref<Record<string, MatchDetail>>({});
const loadingDetail = ref<Set<string>>(new Set());
const extractingRows = ref<Set<string>>(new Set());

function matchKey(m: TrackedMatch): string {
  return `${m.tx_hash}:${m.output_index}`;
}

function isExpanded(m: TrackedMatch): boolean {
  return expandedRows.value.has(matchKey(m));
}

async function toggleExpand(m: TrackedMatch) {
  const key = matchKey(m);
  if (expandedRows.value.has(key)) {
    expandedRows.value.delete(key);
  } else {
    expandedRows.value.add(key);
    if (!matchDetails.value[key]) {
      loadingDetail.value.add(key);
      try {
        matchDetails.value[key] = await api.getMatchDetail(
          m.tx_hash,
          m.output_index,
        );
      } catch (e: any) {
        toast.error(e.message || "Failed to load extraction history");
        expandedRows.value.delete(key);
      } finally {
        loadingDetail.value.delete(key);
      }
    }
  }
  // trigger reactivity
  expandedRows.value = new Set(expandedRows.value);
}

const filters = [
  { labelKey: "matches.all", value: "" },
  { labelKey: "matches.live", value: "live" },
  { labelKey: "matches.exhausted", value: "exhausted" },
  { labelKey: "matches.destroyed", value: "destroyed" },
];

const columns = computed<ColumnDef[]>(() => [
  { key: "_expand", label: "", width: "40px", align: "center" },
  { key: "tx_hash", label: t("common.txHash"), align: "center" },
  { key: "created_at", label: t("matches.matchTime"), align: "center" },
  {
    key: "remaining_days",
    label: t("matches.remainingDays"),
    sortable: true,
    align: "center",
  },
  {
    key: "extractable",
    label: t("matches.extractable"),
    sortable: true,
    align: "center",
  },
  { key: "rent", label: t("matches.rent"), align: "center" },
  { key: "status", label: t("common.status"), align: "center" },
  { key: "actions", label: t("common.actions"), align: "center" },
]);

const totalRent = (m: TrackedMatch) => m.ckb_capacity || 0;
const extractedRent = (m: TrackedMatch) => m.extracted_total ?? 0;
const extractableAmount = (m: TrackedMatch) => m.extractable_shannons ?? 0;
const isExtractableMet = (m: TrackedMatch) =>
  extractableAmount(m) >= minExtractionShannons.value;

async function loadSignerLockHashes() {
  try {
    const wallets = await api.getSignerWallets();
    signerLockHashes.value = wallets.map((w) => w.lock_hash).filter(Boolean);
  } catch {
    signerLockHashes.value = [];
  }
}

async function loadMatches() {
  loading.value = true;
  error.value = null;
  try {
    const hashes =
      signerLockHashes.value.length > 0 ? signerLockHashes.value : undefined;
    matches.value = await api.listMatches(
      filterStatus.value || undefined,
      hashes,
    );
  } catch (e: any) {
    console.error("Failed to load matches:", e);
    error.value = e.message || t("matches.loadFailed");
  } finally {
    loading.value = false;
  }
}

function setFilter(status: string) {
  filterStatus.value = status;
  loadMatches();
}

// ── Wallet unlock gate for extract ──

async function runExtractWithOverlay(match: TrackedMatch) {
  const key = matchKey(match);
  extractingRows.value = new Set([...extractingRows.value, key]);
  try {
    await txConfirm.run({
      action: () => api.extractRent(match.tx_hash, match.output_index),
      onSuccess: () => loadMatches(),
    });
    toast.success(t("matches.extractSuccess"), 5000);
  } catch (e: any) {
    toast.error(e.message || "Extract failed");
    loadMatches();
    throw e;
  } finally {
    extractingRows.value.delete(key);
    extractingRows.value = new Set(extractingRows.value);
  }
}

async function extractRent(match: TrackedMatch) {
  // Check if wallet is unlocked; if not, prompt for password first.
  try {
    const session = await api.getWalletSession();
    if (session.active) {
      try {
        await runExtractWithOverlay(match);
      } catch {
        // Error toast already shown in runExtractWithOverlay.
      }
      return;
    }
  } catch {
    // Can't check session — proceed and let the endpoint handle the error.
  }

  // Wallet is locked — show unlock modal then retry.
  const unlockPassword = ref("");
  const unlockError = ref("");

  function buildUnlockContent() {
    return {
      setup() {
        return () =>
          h("div", { class: "unlock-form" }, [
            h("p", { class: "unlock-hint" }, t("wallets.unlockHint")),
            h("input", {
              type: "password",
              class: "form-input unlock-input-full",
              placeholder: t("orders.unlockPasswordPlaceholder"),
              value: unlockPassword.value,
              onInput: (e: Event) => {
                unlockPassword.value = (e.target as HTMLInputElement).value;
                unlockError.value = "";
              },
            }),
            unlockError.value
              ? h("span", { class: "unlock-error" }, unlockError.value)
              : null,
          ]);
      },
    };
  }

  async function tryUnlock() {
    // Bug fix: return false to keep modal open and show error inline
    if (!unlockPassword.value.trim()) {
      unlockError.value = t("wallets.fillRequired");
      return false;
    }
    try {
      await api.unlockWallet({ password: unlockPassword.value });
      modal.hide();
      await new Promise((r) => setTimeout(r, 200));
      await runExtractWithOverlay(match);
    } catch (e: any) {
      if (modal.visible.value) {
        unlockError.value = e.message || t("wallets.unlockFailed");
        return false;
      }
    }
  }

  modal.show({
    title: t("wallets.unlockTitle"),
    content: buildUnlockContent(),
    confirmText: t("orders.unlockAndContinue"),
    cancelText: t("common.cancel"),
    onConfirm: tryUnlock,
    onCancel: () => modal.hide(),
  });
}

async function doDestroyMatch(match: TrackedMatch) {
  const label = truncateAddress(match.tx_hash, 8, 6);
  const ok = await modal.confirm(t("matches.destroyConfirm", { id: label }), {
    title: t("matches.destroyTitle"),
    danger: true,
    confirmText: t("matches.destroy"),
  });
  if (!ok) return;
  try {
    await txConfirm.run({
      action: () => api.destroyMatch(match.tx_hash, match.output_index),
      onSuccess: () => loadMatches(),
    });
    toast.success(t("matches.destroySuccess"));
  } catch (e: any) {
    toast.error(e.message || "Destroy failed");
    loadMatches();
  }
}

onMounted(async () => {
  await Promise.all([
    api
      .getServerInfo()
      .then((info) => {
        network.value = info.network;
      })
      .catch(() => {}),
    api
      .getRuntimeConfig()
      .then((cfg) => {
        minExtractionShannons.value = cfg.min_extraction_amount_shannons;
      })
      .catch(() => {}),
    loadSignerLockHashes(),
  ]);
  await loadMatches();
});
</script>

<template>
  <div class="page-matches">
    <div class="filter-bar">
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
      <button
        class="btn btn-default"
        :disabled="loading"
        @click="loadMatches()"
      >
        <span
          v-if="loading"
          class="spinner-sm"
        />
        {{ loading ? t("common.loading") : t("matches.refresh") }}
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
      :message="
        filterStatus
          ? t('matches.noStatusMatches', {
            status: t(
              filters.find((f) => f.value === filterStatus)?.labelKey || '',
            ),
          })
          : t('matches.noMatches')
      "
    />
    <DataTable
      v-else
      :columns="columns"
      :rows="matches"
      :loading="loading"
      :expanded-row-keys="expandedRows"
      :row-key="matchKey"
      :clickable-rows="true"
      @row-click="toggleExpand"
    >
      <template #cell-_expand="{ row }">
        <span class="expand-indicator">
          <span
            v-if="loadingDetail.has(matchKey(row))"
            class="expand-spinner"
          />
          <span
            v-else
            class="expand-chevron"
            :class="{ expanded: isExpanded(row) }"
          >▶</span>
        </span>
      </template>
      <template #cell-tx_hash="{ value }">
        <a
          :href="explorerTxUrl(String(value), network)"
          target="_blank"
          rel="noopener noreferrer"
          class="tx-link font-mono"
        >{{ truncateAddress(String(value), 12, 8) }}</a>
      </template>
      <template #cell-created_at="{ value }">
        <span class="match-time">{{
          value ? new Date(Number(value)).toLocaleString() : "—"
        }}</span>
      </template>
      <template #cell-remaining_days="{ value }">
        <span :class="{ 'text-exhausted': Number(value) <= 0 }">
          {{ Number(value) <= 0 ? "—" : Number(value).toFixed(1) + " d" }}
        </span>
      </template>
      <template #cell-extractable="{ row }">
        <span
          :class="[
            isExtractableMet(row)
              ? 'text-extractable-met'
              : 'text-extractable-low',
            !isExtractableMet(row) ? 'tooltip-trigger' : '',
          ]"
          :data-tooltip="
            isExtractableMet(row) ? '' : t('matches.extractBelowThreshold')
          "
        >
          {{ formatCKB(extractableAmount(row)) }}
        </span>
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
            v-if="extractingRows.has(matchKey(row))"
            class="btn btn-sm btn-primary"
            disabled
          >
            <span class="spinner-sm" />
            {{ t("matches.extract") }}
          </button>
          <button
            v-else-if="row.status === 'live' && isExtractableMet(row)"
            class="btn btn-sm btn-primary"
            @click.stop="extractRent(row)"
          >
            {{ t("matches.extract") }}
          </button>
          <button
            v-else-if="row.status === 'live'"
            class="btn btn-sm btn-disabled"
            disabled
          >
            {{ t("matches.extract") }}
          </button>
          <button
            v-else-if="row.status === 'exhausted'"
            class="btn btn-sm btn-danger"
            @click.stop="doDestroyMatch(row)"
          >
            {{ t("matches.destroy") }}
          </button>
        </div>
      </template>
      <template #expanded="{ row }">
        <div class="expanded-content">
          <div class="extraction-header">
            <h4 class="extraction-title">
              {{ t("matches.extractionHistory") }}
            </h4>
            <span
              v-if="matchDetails[matchKey(row)]"
              class="extraction-summary"
            >
              {{ t("matches.rent") }}:
              {{
                formatCKB(matchDetails[matchKey(row)].extracted_total_shannons)
              }}
              / {{ formatCKB(row.ckb_capacity) }}
            </span>
          </div>

          <!-- Loading skeleton -->
          <table
            v-if="loadingDetail.has(matchKey(row))"
            class="extraction-table"
          >
            <thead>
              <tr>
                <th>{{ t("matches.rent").split("/")[0].trim() }}</th>
                <th>{{ t("matches.lastExtractionBlock") }}</th>
                <th>{{ t("common.txHash") }}</th>
                <th>{{ t("common.createdAt") }}</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="i in 3"
                :key="`skel-${i}`"
                class="skeleton-row"
              >
                <td
                  v-for="j in 4"
                  :key="j"
                >
                  <span class="skeleton-bar" />
                </td>
              </tr>
            </tbody>
          </table>

          <!-- Loaded data -->
          <table
            v-else
            class="extraction-table"
          >
            <thead>
              <tr>
                <th>{{ t("matches.rent").split("/")[0].trim() }}</th>
                <th>{{ t("matches.lastExtractionBlock") }}</th>
                <th>{{ t("common.txHash") }}</th>
                <th>{{ t("common.createdAt") }}</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-if="!matchDetails[matchKey(row)]?.extraction_history?.length"
              >
                <td
                  colspan="4"
                  class="extraction-empty"
                >
                  {{ t("matches.noExtractions") }}
                </td>
              </tr>
              <tr
                v-for="ex in matchDetails[matchKey(row)]?.extraction_history"
                :key="ex.id"
              >
                <td>{{ formatCKB(ex.extracted_amount) }}</td>
                <td>{{ ex.tip_block }}</td>
                <td>
                  <a
                    :href="explorerTxUrl(ex.tx_hash, network)"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="tx-link font-mono"
                  >{{ truncateAddress(ex.tx_hash, 12, 8) }}</a>
                </td>
                <td>
                  {{
                    ex.timestamp ? new Date(ex.timestamp).toLocaleString() : "—"
                  }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </template>
    </DataTable>
  </div>
</template>

<style scoped>
.page-matches {
  max-width: 1200px;
  margin: 0 auto;
}
.page-matches :deep(.table-wrapper) {
  overflow: visible !important;
}
.page-matches :deep(.data-table td) {
  overflow: visible !important;
}
.page-matches :deep(.data-table tbody tr) {
  overflow: visible !important;
}
.filter-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-lg);
}
.filter-tabs {
  display: flex;
  gap: var(--space-xs);
}
.filter-tab {
  padding: var(--space-xs) var(--space-md);
  border: 1px solid var(--border-dark);
  border-radius: var(--radius-md);
  background: var(--bg-card);
  color: var(--text-secondary);
  font-size: var(--fs-body);
  cursor: pointer;
  transition: all var(--transition-base);
}
.filter-tab:hover {
  color: var(--primary-500);
  border-color: var(--primary-500);
}
.filter-tab.active {
  background: var(--primary-500);
  border-color: var(--primary-500);
  color: #fff;
}
.tx-link {
  color: var(--primary-500);
  text-decoration: none;
}
.tx-link:hover {
  text-decoration: underline;
}
.text-exhausted {
  color: var(--text-disabled);
}
.text-extractable-met {
  color: #52c41a;
  font-weight: 600;
}
.text-extractable-low {
  color: #ff4d4f;
}
.btn-disabled {
  background: var(--gray-200);
  color: var(--text-disabled);
  cursor: not-allowed;
  border: 1px solid var(--border-light);
}
.btn-disabled:hover {
  background: var(--gray-200);
}

/* Custom hover tooltip with arrow */
.tooltip-trigger {
  position: relative;
  cursor: help;
}
/* Tooltip body */
.tooltip-trigger::after {
  content: attr(data-tooltip);
  position: absolute;
  bottom: calc(100% + 10px);
  left: 50%;
  transform: translateX(-50%);
  padding: 6px 12px;
  background: rgba(0, 0, 0, 0.85);
  color: #fff;
  font-size: var(--fs-caption, 12px);
  font-weight: 400;
  line-height: 1.45;
  white-space: nowrap;
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.18);
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.2s ease 0.3s;
  z-index: 999;
}
/* Tooltip arrow (downward triangle) */
.tooltip-trigger::before {
  content: "";
  position: absolute;
  bottom: calc(100% + 4px);
  left: 50%;
  transform: translateX(-50%);
  width: 0;
  height: 0;
  border-left: 6px solid transparent;
  border-right: 6px solid transparent;
  border-top: 6px solid rgba(0, 0, 0, 0.85);
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.2s ease 0.3s;
  z-index: 999;
}
.tooltip-trigger:hover::after,
.tooltip-trigger:hover::before {
  opacity: 1;
}
.copyable {
  cursor: pointer;
  transition: color var(--transition-base);
}
.copyable:hover {
  color: var(--primary-500);
}
.actions-group {
  display: flex;
  gap: 4px;
  justify-content: center;
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
.btn-default {
  background: var(--bg-card);
  color: var(--text-primary);
  border: 1px solid var(--border-dark);
}
.btn-default:hover {
  color: var(--primary-500);
  border-color: var(--primary-500);
}
.btn-primary {
  background: var(--primary-500);
  color: #fff;
}
.btn-primary:hover {
  background: var(--primary-400);
}
.btn-danger {
  background: var(--danger);
  color: #fff;
}
.btn-danger:hover {
  background: #ff7875;
}
.btn-sm {
  height: 28px;
  font-size: var(--fs-caption);
  padding: 0 var(--space-sm);
}
.btn:disabled {
  opacity: 0.65;
  cursor: not-allowed;
}
.spinner-sm {
  width: 14px;
  height: 14px;
  border: 2px solid var(--gray-300);
  border-top-color: var(--primary-500);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
  display: inline-block;
}
.md-footer-actions {
  display: flex;
  gap: 8px;
}

/* Expand indicator */
.expand-indicator {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
}
.expand-chevron {
  display: inline-block;
  font-size: 10px;
  line-height: 1;
  color: var(--text-muted);
  transition: transform 0.2s ease;
}
.expand-chevron.expanded {
  transform: rotate(90deg);
}
.expand-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--border-light);
  border-top-color: var(--primary-500);
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

/* Expanded content */
.expanded-content {
  padding: var(--space-md);
}
.extraction-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-sm);
}
.extraction-title {
  margin: 0;
  font-size: var(--fs-caption);
  font-weight: 600;
  color: var(--text-secondary);
}
.extraction-summary {
  font-size: var(--fs-small);
  color: var(--text-muted);
  font-variant-numeric: tabular-nums;
}
.extraction-table {
  width: 88%;
  margin: 0 auto;
  border-collapse: collapse;
  font-size: var(--fs-caption);
  background: var(--bg-card);
  border: 1px solid var(--border-light);
  border-radius: var(--radius-md);
  overflow: hidden;
}
.extraction-table th {
  background: var(--gray-50);
  color: var(--text-secondary);
  font-weight: 500;
  padding: 6px 10px;
  border-bottom: 1px solid var(--border-light);
  text-align: center;
  white-space: nowrap;
}
.extraction-table td {
  padding: 6px 10px;
  border-bottom: 1px solid var(--border-light);
  color: var(--text-primary);
  text-align: center;
}
.extraction-table tbody tr:last-child td {
  border-bottom: none;
}
.extraction-table tbody tr:hover td {
  background: var(--gray-25, #fafbfc);
}
.extraction-empty {
  text-align: center;
  color: var(--text-disabled);
  padding: var(--space-md) 0 !important;
  font-style: italic;
}

/* Sub-table skeleton loading */
.skeleton-row td {
  padding: 8px 10px;
}
.skeleton-bar {
  display: block;
  height: 12px;
  border-radius: var(--radius-sm, 4px);
  background: linear-gradient(
    90deg,
    var(--gray-100, #f0f0f0) 25%,
    var(--gray-200, #e0e0e0) 50%,
    var(--gray-100, #f0f0f0) 75%
  );
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.5s ease-in-out infinite;
}
</style>

<!-- Global styles for unlock modal (teleported to body, scoped won't reach) -->
<style>
.unlock-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
  padding: var(--space-xs) 0;
}
.unlock-hint {
  margin: 0;
  font-size: var(--fs-body);
  color: var(--text-secondary);
  text-align: center;
  line-height: var(--lh-body);
}
.unlock-input-full {
  width: 100%;
  height: 36px;
  padding: 0 var(--space-sm);
  border: 1px solid var(--border-dark);
  border-radius: var(--radius-md);
  font-size: var(--fs-body);
  color: var(--text-primary);
  background: var(--bg-card);
  outline: none;
  box-sizing: border-box;
  transition:
    border-color var(--transition-base),
    box-shadow var(--transition-base);
}
.unlock-input-full:focus {
  border-color: var(--primary-500);
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}
.unlock-input-full::placeholder {
  color: var(--text-disabled);
}
.unlock-error {
  font-size: var(--fs-small);
  color: var(--danger);
  text-align: center;
}
</style>
