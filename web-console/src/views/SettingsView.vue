<script setup lang="ts">
import { ref, onMounted, inject, h } from "vue";
import { useApi } from "@/composables/useApi";
import { useI18n } from "@/composables/useI18n";
import StatusTag from "@/components/ui/StatusTag.vue";
import AutomationUnlockForm from "@/components/ui/AutomationUnlockForm.vue";
import AutomationConsole from "@/components/ui/AutomationConsole.vue";
import type { RuntimeConfig, ServerInfo, SignerWalletItem } from "@/types/api";

const api = useApi();
const { t } = useI18n();
const toast = inject<any>("toast")!;
const modal = inject<any>("modal")!;

const activeTab = ref<"auto-match" | "rent-extraction" | "network">(
  "auto-match",
);

// Runtime Config (shared by auto-match and rent-extraction tabs)
const config = ref<RuntimeConfig | null>(null);
const configLoading = ref(true);
const editingMatch = ref(false);
const editingRent = ref(false);
const editingNetwork = ref(false);
const editForm = ref<RuntimeConfig>({
  fee_rate: 0,
  confirm_count: 1,
  rent_extraction_enabled: true,
  scheduler_interval_secs: 0,
  min_extraction_amount_shannons: 0,
  auto_match_enabled: false,
  auto_match_min_capacity: 0,
  auto_match_max_escrow_blocks: 0,
  auto_match_interval_secs: 0,
  automation_signer_address: "",
  chain_cache_enabled: true,
  chain_cache_interval_secs: 30,
  wallet_tx_sync_enabled: true,
  wallet_tx_sync_interval_secs: 60,
  ckb_rpc_url: "",
  ckb_indexer_url: "",
  fiber_rpc_url: "",
});

async function loadConfig() {
  configLoading.value = true;
  try {
    config.value = await api.getRuntimeConfig();
    editForm.value = { ...config.value };
  } catch (e: any) {
    console.error("Failed to load runtime config:", e);
    toast.error(e.message || t("settings.configLoadFailed"));
  } finally {
    configLoading.value = false;
  }
}

function startEditingMatch() {
  if (config.value) editForm.value = { ...config.value };
  editingMatch.value = true;
}
function cancelEditingMatch() {
  editForm.value = { ...config.value! };
  editingMatch.value = false;
}

function startEditingRent() {
  if (config.value) editForm.value = { ...config.value };
  editingRent.value = true;
}
function cancelEditingRent() {
  editForm.value = { ...config.value! };
  editingRent.value = false;
}

function startEditingNetwork() {
  if (config.value) editForm.value = { ...config.value };
  editingNetwork.value = true;
}
function cancelEditingNetwork() {
  editForm.value = { ...config.value! };
  editingNetwork.value = false;
}

async function doSaveConfig() {
  const updated = await api.updateRuntimeConfig(editForm.value);
  config.value = updated;
  editForm.value = { ...updated };
  editingMatch.value = false;
  editingRent.value = false;
  editingNetwork.value = false;
}

function promptUnlockThenSave() {
  const selectedAddress = ref("");
  const pw = ref("");
  const err = ref("");
  const showPassword = ref(false);
  const wallets = ref<SignerWalletItem[]>([]);
  const walletsLoading = ref(true);

  api
    .getSignerWallets()
    .then((list) => {
      wallets.value = list;
      const current = config.value?.automation_signer_address;
      if (current && list.some((w) => w.ckb_address === current)) {
        selectedAddress.value = current;
      } else if (list.length > 0) {
        selectedAddress.value = list[0].ckb_address;
      }
    })
    .catch(() => {
      err.value = t("settings.loadSignerWalletsFailed");
    })
    .finally(() => {
      walletsLoading.value = false;
    });

  modal.show({
    title: t("settings.selectSignerToEnable"),
    wide: true,
    content: {
      setup() {
        return () =>
          h(AutomationUnlockForm, {
            selectedAddress: selectedAddress.value,
            password: pw.value,
            error: err.value,
            wallets: wallets.value,
            walletsLoading: walletsLoading.value,
            showPassword: showPassword.value,
            "onUpdate:selectedAddress": (value: string) => {
              selectedAddress.value = value;
              err.value = "";
            },
            "onUpdate:password": (value: string) => {
              pw.value = value;
              err.value = "";
            },
          });
      },
    },
    confirmText: t("common.continue"),
    onConfirm: async () => {
      if (!selectedAddress.value) {
        err.value = t("settings.selectSignerRequired");
        return false;
      }

      editForm.value.automation_signer_address = selectedAddress.value;

      if (!showPassword.value) {
        let sessionActive = false;
        try {
          sessionActive = (await api.getWalletSession()).active;
        } catch {
          // treat as locked — proceed to password step
        }
        if (!sessionActive) {
          showPassword.value = true;
          modal.title.value = t("settings.unlockToEnable");
          modal.confirmText.value = t("settings.unlockAndSave");
          return false;
        }

        try {
          await doSaveConfig();
          toast.success(t("settings.configSaved"));
        } catch (e: any) {
          err.value = e.message || t("settings.configSaveFailed");
          return false;
        }
        return;
      }

      if (!pw.value) {
        err.value = t("wallets.fillRequired");
        return false;
      }
      try {
        await api.unlockWallet({ password: pw.value });
        await doSaveConfig();
        toast.success(t("settings.configSaved"));
      } catch (e: any) {
        err.value = e.message || t("settings.configSaveFailed");
        return false;
      }
    },
    onCancel: () => modal.hide(),
  });
}

async function saveConfig() {
  const enablingAutoMatch =
    editForm.value.auto_match_enabled && !config.value?.auto_match_enabled;
  const enablingRentExtraction =
    editForm.value.rent_extraction_enabled &&
    !config.value?.rent_extraction_enabled;

  if (enablingAutoMatch || enablingRentExtraction) {
    promptUnlockThenSave();
    return;
  }

  try {
    await doSaveConfig();
    toast.success(t("settings.configSaved"));
  } catch (e: any) {
    console.error("Failed to save config:", e);
    toast.error(e.message || t("settings.configSaveFailed"));
  }
}

async function resetConfig() {
  try {
    const defaults = await api.resetRuntimeConfig();
    config.value = defaults;
    if (editingMatch.value || editingRent.value || editingNetwork.value)
      editForm.value = { ...defaults };
    toast.success(t("settings.configSaved"));
  } catch (e: any) {
    console.error("Failed to reset config:", e);
    toast.error(e.message || t("settings.configResetFailed"));
  }
}

// Network info
const serverInfo = ref<ServerInfo | null>(null);

async function loadServerInfo() {
  try {
    serverInfo.value = await api.getServerInfo();
  } catch (e) {
    console.warn("Failed to load server info:", e);
  }
}

onMounted(() => {
  loadConfig();
  loadServerInfo();
});
</script>

<template>
  <div class="page-settings">
    <div class="sub-tabs">
      <button
        class="sub-tab"
        :class="{ active: activeTab === 'auto-match' }"
        @click="activeTab = 'auto-match'"
      >
        {{ t("settings.autoMatch") }}
      </button>
      <button
        class="sub-tab"
        :class="{ active: activeTab === 'rent-extraction' }"
        @click="activeTab = 'rent-extraction'"
      >
        {{ t("settings.rentExtraction") }}
      </button>
      <button
        class="sub-tab"
        :class="{ active: activeTab === 'network' }"
        @click="activeTab = 'network'"
      >
        {{ t("settings.network") }}
      </button>
    </div>

    <!-- Auto-Match Tab -->
    <div
      v-if="activeTab === 'auto-match'"
      class="card config-card"
    >
      <div class="card-header">
        <h3>{{ t("settings.autoMatchConfig") }}</h3>
        <div class="card-header-actions">
          <button
            v-if="!editingMatch"
            class="btn btn-default btn-sm"
            @click="resetConfig"
          >
            {{ t("settings.reset") }}
          </button>
          <button
            v-if="!editingMatch"
            class="btn btn-default btn-sm"
            @click="startEditingMatch"
          >
            {{ t("settings.edit") }}
          </button>
        </div>
      </div>
      <div
        v-if="configLoading"
        class="text-muted"
      >
        {{ t("common.loading") }}
      </div>
      <template v-else-if="config">
        <div
          v-if="!editingMatch"
          class="config-display"
        >
          <div class="config-row">
            <span class="config-label">{{ t("settings.enabled") }}</span><StatusTag
              :status="config.auto_match_enabled ? 'live' : 'destroyed'"
              :label="
                config.auto_match_enabled
                  ? t('settings.enabledLabel')
                  : t('settings.disabledLabel')
              "
            />
          </div>
          <div class="config-row">
            <span class="config-label">{{ t("settings.minCapacity") }}</span><span>{{ (config.auto_match_min_capacity / 100_000_000).toFixed(0) }}
              {{ t("common.CKB") }}</span>
          </div>
          <div class="config-row">
            <span class="config-label">{{ t("settings.maxEscrow") }}</span><span>{{ config.auto_match_max_escrow_blocks.toLocaleString() }}
              {{ t("settings.blocks") }}</span>
          </div>
          <div class="config-row">
            <span class="config-label">{{ t("settings.interval") }}</span><span>{{ config.auto_match_interval_secs }}
              {{ t("settings.seconds") }}</span>
          </div>
          <div
            v-if="config.automation_signer_address"
            class="config-row"
          >
            <span class="config-label">{{ t("settings.signerAddress") }}</span><span
              class="font-mono"
              :title="config.automation_signer_address"
            >{{ config.automation_signer_address }}</span>
          </div>
        </div>
        <div
          v-else
          class="config-form"
        >
          <div class="form-group">
            <label class="form-label"><input
                                        v-model="editForm.auto_match_enabled"
                                        type="checkbox"
                                      >
              {{ t("settings.enableAutoMatch") }}</label>
          </div>
          <div class="form-group">
            <label class="form-label">{{ t("settings.minCapacity") }} (shannons)</label><input
              v-model.number="editForm.auto_match_min_capacity"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-group">
            <label class="form-label">{{ t("settings.maxEscrow") }}</label><input
              v-model.number="editForm.auto_match_max_escrow_blocks"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-group">
            <label class="form-label">{{ t("settings.interval") }} ({{ t("settings.seconds") }})</label><input
              v-model.number="editForm.auto_match_interval_secs"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-actions">
            <button
              class="btn btn-default"
              @click="cancelEditingMatch"
            >
              {{ t("settings.cancel") }}
            </button><button
              class="btn btn-primary"
              @click="saveConfig"
            >
              {{ t("settings.save") }}
            </button>
          </div>
        </div>
      </template>
    </div>

    <!-- Rent Extraction Tab -->
    <div
      v-if="activeTab === 'rent-extraction'"
      class="card config-card"
    >
      <div class="card-header">
        <h3>{{ t("settings.rentExtractionConfig") }}</h3>
        <div class="card-header-actions">
          <button
            v-if="!editingRent"
            class="btn btn-default btn-sm"
            @click="resetConfig"
          >
            {{ t("settings.reset") }}
          </button>
          <button
            v-if="!editingRent"
            class="btn btn-default btn-sm"
            @click="startEditingRent"
          >
            {{ t("settings.edit") }}
          </button>
        </div>
      </div>
      <div
        v-if="configLoading"
        class="text-muted"
      >
        {{ t("common.loading") }}
      </div>
      <template v-else-if="config">
        <!-- Display mode -->
        <div
          v-if="!editingRent"
          class="config-display"
        >
          <div class="config-row">
            <span class="config-label">{{ t("settings.enabled") }}</span>
            <StatusTag
              :status="config.rent_extraction_enabled ? 'live' : 'destroyed'"
              :label="
                config.rent_extraction_enabled
                  ? t('settings.enabledLabel')
                  : t('settings.disabledLabel')
              "
            />
          </div>
          <div class="config-row">
            <span class="config-label">{{
              t("settings.schedulerInterval")
            }}</span><span>{{ config.scheduler_interval_secs }}
              {{ t("settings.seconds") }}</span>
          </div>
          <div class="config-row">
            <span class="config-label">{{ t("settings.minExtraction") }}</span><span>{{
                                                                                       (config.min_extraction_amount_shannons / 100_000_000).toFixed(2)
                                                                                     }}
              {{ t("common.CKB") }}</span>
          </div>
        </div>
        <!-- Edit mode -->
        <div
          v-else
          class="config-form"
        >
          <div class="form-group">
            <label class="form-label">
              <input
                v-model="editForm.rent_extraction_enabled"
                type="checkbox"
              >
              {{ t("settings.enableRentExtraction") }}
            </label>
          </div>
          <div class="form-group">
            <label class="form-label">{{ t("settings.schedulerInterval") }} ({{
              t("settings.seconds")
            }})</label><input
              v-model.number="editForm.scheduler_interval_secs"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-group">
            <label class="form-label">{{ t("settings.minExtraction") }} (shannons)</label><input
              v-model.number="editForm.min_extraction_amount_shannons"
              type="number"
              class="form-input"
            >
          </div>
          <div class="form-actions">
            <button
              class="btn btn-default"
              @click="cancelEditingRent"
            >
              {{ t("settings.cancel") }}
            </button><button
              class="btn btn-primary"
              @click="saveConfig"
            >
              {{ t("settings.save") }}
            </button>
          </div>
        </div>
      </template>
    </div>

    <div
      v-if="activeTab === 'network'"
      class="card config-card"
    >
      <div class="card-header">
        <h3>{{ t("settings.networkInfo") }}</h3>
        <div class="card-header-actions">
          <button
            v-if="!editingNetwork && config"
            class="btn btn-default btn-sm"
            @click="resetConfig"
          >
            {{ t("settings.reset") }}
          </button>
          <button
            v-if="!editingNetwork && config"
            class="btn btn-default btn-sm"
            @click="startEditingNetwork"
          >
            {{ t("settings.edit") }}
          </button>
        </div>
      </div>
      <!-- Display mode -->
      <template v-if="config && !editingNetwork">
        <div
          v-if="serverInfo"
          class="config-display"
        >
          <div class="config-row">
            <span class="config-label">{{ t("settings.network") }}</span>
            <StatusTag
              :status="serverInfo.network === 'mainnet' ? 'destroyed' : 'live'"
              :label="serverInfo.network === 'mainnet' ? 'Mainnet' : 'Testnet'"
            />
          </div>
          <div class="config-row">
            <span class="config-label">CKB RPC</span><code
              class="font-mono"
              style="font-size: var(--fs-caption)"
            >{{
              config.ckb_rpc_url
            }}</code>
          </div>
          <div class="config-row">
            <span class="config-label">CKB Indexer</span><code
              class="font-mono"
              style="font-size: var(--fs-caption)"
            >{{
              config.ckb_indexer_url
            }}</code>
          </div>
          <div class="config-row">
            <span class="config-label">Fiber RPC</span><code
              class="font-mono"
              style="font-size: var(--fs-caption)"
            >{{
              config.fiber_rpc_url
            }}</code>
          </div>
          <div class="config-row">
            <span class="config-label">{{ t("settings.feeRate") }}</span><span>{{ config.fee_rate.toLocaleString() }} shannons/KB</span>
          </div>
          <div class="config-row">
            <span class="config-label">{{ t("settings.confirmCount") }}</span><span>{{ config.confirm_count }}
              {{ t("settings.confirmCountHint") }}</span>
          </div>
        </div>
      </template>
      <!-- Edit mode -->
      <div
        v-else-if="config && editingNetwork"
        class="config-form"
      >
        <div class="form-group">
          <label class="form-label">CKB RPC</label><input
            v-model="editForm.ckb_rpc_url"
            type="text"
            class="form-input"
          >
        </div>
        <div class="form-group">
          <label class="form-label">CKB Indexer</label><input
            v-model="editForm.ckb_indexer_url"
            type="text"
            class="form-input"
          >
        </div>
        <div class="form-group">
          <label class="form-label">Fiber RPC</label><input
            v-model="editForm.fiber_rpc_url"
            type="text"
            class="form-input"
          >
        </div>
        <div class="form-group">
          <label class="form-label">{{ t("settings.feeRate") }}</label><input
            v-model.number="editForm.fee_rate"
            type="number"
            class="form-input"
          >
        </div>
        <div class="form-group">
          <label class="form-label">{{ t("settings.confirmCount") }} ({{ t("settings.confirmCountHint") }})</label><input
            v-model.number="editForm.confirm_count"
            type="number"
            min="1"
            class="form-input"
          >
        </div>
        <div class="form-actions">
          <button
            class="btn btn-default"
            @click="cancelEditingNetwork"
          >
            {{ t("settings.cancel") }}
          </button><button
            class="btn btn-primary"
            @click="saveConfig"
          >
            {{ t("settings.save") }}
          </button>
        </div>
      </div>
      <div
        v-else
        class="text-muted"
      >
        {{ t("common.loading") }}
      </div>
    </div>

    <AutomationConsole
      v-if="config"
      :auto-match-enabled="config.auto_match_enabled"
      :rent-extraction-enabled="config.rent_extraction_enabled"
    />
  </div>
</template>

<style scoped>
.page-settings {
  max-width: 900px;
  margin: 0 auto;
}
.sub-tabs {
  display: flex;
  gap: var(--space-xs);
  margin-bottom: var(--space-xl);
  border-bottom: 1px solid var(--border-light);
}
.sub-tab {
  padding: var(--space-xs) var(--space-md);
  border: none;
  border-bottom: 2px solid transparent;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--fs-body);
  cursor: pointer;
  transition: all var(--transition-base);
  margin-bottom: -1px;
}
.sub-tab:hover {
  color: var(--primary-500);
}
.sub-tab.active {
  color: var(--primary-500);
  border-bottom-color: var(--primary-500);
  font-weight: 500;
}
.card {
  background: var(--bg-card);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-light);
  box-shadow: var(--shadow-base);
  padding: var(--space-xl);
}
.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-lg);
}
.card-header h3 {
  font-size: var(--fs-h3);
  font-weight: var(--fw-h3);
}
.card-header-actions {
  display: flex;
  gap: var(--space-xs);
}
.config-display {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}
.config-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-sm) 0;
  border-bottom: 1px solid var(--border-light);
  font-size: var(--fs-body);
}
.config-row:last-child {
  border-bottom: none;
}
.config-label {
  color: var(--text-secondary);
}
.config-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}
.form-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-xs);
}
.form-label {
  font-size: var(--fs-body);
  color: var(--text-secondary);
  display: flex;
  align-items: center;
  gap: var(--space-xs);
}
.form-input {
  height: 32px;
  padding: 0 var(--space-sm);
  border: 1px solid var(--border-dark);
  border-radius: var(--radius-md);
  font-size: var(--fs-body);
  color: var(--text-primary);
  background: var(--bg-card);
  outline: none;
  transition:
    border-color var(--transition-base),
    box-shadow var(--transition-base);
}
.form-input:focus {
  border-color: var(--primary-500);
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}
.form-actions {
  display: flex;
  gap: var(--space-sm);
  justify-content: flex-end;
  margin-top: var(--space-sm);
}
.form-hint {
  font-size: var(--fs-small);
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
.btn-primary:hover {
  background: var(--primary-400);
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
.btn-sm {
  height: 28px;
  font-size: var(--fs-caption);
  padding: 0 var(--space-sm);
}
</style>
