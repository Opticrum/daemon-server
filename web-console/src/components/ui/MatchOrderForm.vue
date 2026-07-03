<script setup lang="ts">
import { ref } from 'vue'
import type { MatchOrderRequest, SignerWalletItem } from '@/types/api'
import { useI18n } from '@/composables/useI18n'
import WalletSelector from '@/components/ui/WalletSelector.vue'

const { t } = useI18n()

const props = defineProps<{
  modelValue: MatchOrderRequest & { tx_hash?: string }
  wallets?: SignerWalletItem[]
  walletsLoading?: boolean
  /** When true, show password prompt instead of address selector. */
  needsUnlock?: boolean
  unlocking?: boolean
  unlockError?: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: MatchOrderRequest & { tx_hash?: string }]
  'unlock': [password: string]
  'cancelUnlock': []
}>()

const unlockPassword = ref('')

function handleUnlock() {
  if (!unlockPassword.value.trim()) return
  emit('unlock', unlockPassword.value)
}

function update(field: string, value: string | number) {
  emit('update:modelValue', { ...props.modelValue, [field]: value })
}
</script>

<template>
  <div class="match-form">
    <div
      v-if="modelValue.tx_hash"
      class="form-group"
    >
      <label class="form-label">订单交易哈希</label>
      <code class="form-static font-mono">{{ modelValue.tx_hash }}</code>
    </div>

    <!-- Password prompt — shown when wallet needs unlocking at confirm time -->
    <div
      v-if="needsUnlock"
      class="form-group"
    >
      <label class="form-label">{{ t('orders.selectSellerAddr') }}</label>
      <div class="locked-section">
        <p class="locked-msg">
          🔒 {{ t('orders.walletLockedForMatch') }}
        </p>
        <div class="unlock-row">
          <input
            v-model="unlockPassword"
            type="password"
            class="form-input unlock-input"
            :placeholder="t('orders.unlockPasswordPlaceholder')"
            @keydown.enter="handleUnlock"
          >
          <button
            type="button"
            class="btn-unlock"
            :disabled="unlocking || !unlockPassword.trim()"
            @click="handleUnlock"
          >
            <span
              v-if="unlocking"
              class="spinner"
            />
            {{ unlocking ? t('orders.unlocking') : t('orders.unlockAndContinue') }}
          </button>
        </div>
        <span
          v-if="unlockError"
          class="unlock-error"
        >{{ unlockError }}</span>
      </div>
      <p class="form-note">
        {{ t('orders.channelAutoNote') }}
      </p>
    </div>

    <!-- Normal address selection -->
    <template v-else>
      <div class="form-group">
        <label
          class="form-label"
          :title="modelValue.seller_address"
        >{{ t('orders.selectSellerAddr') }}</label>

        <WalletSelector
          v-if="walletsLoading || (wallets && wallets.length > 0)"
          :model-value="modelValue.seller_address"
          :wallets="wallets ?? []"
          :loading="walletsLoading ?? false"
          :placeholder="t('orders.selectSellerAddr')"
          @update:model-value="update('seller_address', $event)"
        />
        <input
          v-else
          :value="modelValue.seller_address"
          type="text"
          class="form-input font-mono"
          placeholder="ckb1q..."
          @input="update('seller_address', ($event.target as HTMLInputElement).value)"
        >
        <span
          v-if="!walletsLoading && (!wallets || wallets.length === 0)"
          class="form-hint"
        >{{ t('orders.noWalletsUnlocked') }}</span>
      </div>
      <p class="form-note">
        {{ t('orders.channelAutoNote') }}
      </p>
    </template>
  </div>
</template>

<style scoped>
.match-form {
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
  font-weight: 500;
  text-align: center;
}

.form-input {
  height: 36px;
  padding: 0 var(--space-sm);
  border: 1px solid var(--border-dark);
  border-radius: var(--radius-md);
  font-size: var(--fs-body);
  color: var(--text-primary);
  background: var(--bg-card);
  outline: none;
  transition: border-color var(--transition-base), box-shadow var(--transition-base);
  text-align: center;
}
.form-input:focus {
  border-color: var(--primary-500);
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}
.form-input::placeholder {
  color: var(--text-disabled);
}

.form-hint {
  font-size: var(--fs-small);
  color: var(--text-disabled);
  text-align: center;
  margin-top: 2px;
}

.form-note {
  font-size: var(--fs-small);
  color: var(--text-disabled);
  text-align: center;
  margin: 0;
}

.form-static {
  display: block;
  padding: var(--space-xs) var(--space-sm);
  background: var(--gray-50);
  border-radius: var(--radius-sm);
  font-size: var(--fs-caption);
  color: var(--text-secondary);
  word-break: break-all;
  text-align: center;
}

/* ── Password prompt ── */
.locked-section {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-sm);
}
.locked-msg {
  margin: 0;
  font-size: var(--fs-body);
  color: var(--text-secondary);
  text-align: center;
}
.unlock-row {
  display: flex;
  gap: var(--space-xs);
  width: 100%;
}
.unlock-input {
  flex: 1;
  text-align: left;
}
.btn-unlock {
  display: inline-flex;
  align-items: center;
  gap: var(--space-xs);
  height: 36px;
  padding: 0 var(--space-md);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--fs-body);
  font-family: inherit;
  font-weight: 500;
  cursor: pointer;
  background: var(--primary-500);
  color: #fff;
  white-space: nowrap;
  transition: background var(--transition-base);
}
.btn-unlock:hover:not(:disabled) { background: var(--primary-400); }
.btn-unlock:disabled { opacity: 0.6; cursor: not-allowed; }
.unlock-error {
  font-size: var(--fs-small);
  color: var(--danger);
}
.spinner {
  display: inline-block;
  width: 14px;
  height: 14px;
  border: 2px solid rgba(255,255,255,0.3);
  border-top-color: #fff;
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}
</style>
