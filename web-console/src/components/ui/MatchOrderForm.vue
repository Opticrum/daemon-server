<script setup lang="ts">
import type { MatchOrderRequest, SignerWalletItem } from '@/types/api'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress, formatCKB } from '@/utils/format'

const { t } = useI18n()

const props = defineProps<{
  modelValue: MatchOrderRequest & { tx_hash?: string }
  wallets?: SignerWalletItem[]
}>()

const emit = defineEmits<{
  'update:modelValue': [value: MatchOrderRequest & { tx_hash?: string }]
}>()

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
    <div class="form-group">
      <label
        class="form-label"
        :title="modelValue.seller_address"
      >{{ t('orders.selectSellerAddr') }}</label>
      <!-- Dropdown when HD wallet addresses are available -->
      <select
        v-if="wallets && wallets.length > 0"
        :value="modelValue.seller_address"
        class="form-input form-select"
        @change="update('seller_address', ($event.target as HTMLSelectElement).value)"
      >
        <option
          value=""
          disabled
        >
          -- {{ t('orders.selectSellerAddr') }} --
        </option>
        <option
          v-for="w in wallets"
          :key="w.id"
          :value="w.ckb_address"
        >
          #{{ w.derivation_index }} &mdash; {{ truncateAddress(w.ckb_address, 14, 8) }} &mdash; {{ formatCKB(w.balance_shannons) }}
        </option>
      </select>
      <!-- Fallback text input when no wallets available -->
      <input
        v-else
        :value="modelValue.seller_address"
        type="text"
        class="form-input font-mono"
        placeholder="ckb1q..."
        @input="update('seller_address', ($event.target as HTMLInputElement).value)"
      >
      <span
        v-if="!wallets || wallets.length === 0"
        class="form-hint"
      >{{ t('orders.noWalletsUnlocked') }}</span>
    </div>
    <p class="form-note">
      {{ t('orders.channelAutoNote') }}
    </p>
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

.form-select {
  cursor: pointer;
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 12 12'%3E%3Cpath d='M6 8L1 3h10z' fill='%23888'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 10px center;
  padding-right: 30px;
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
</style>
