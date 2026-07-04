<script setup lang="ts">
import { useI18n } from '@/composables/useI18n'
import WalletSelector from '@/components/ui/WalletSelector.vue'
import type { SignerWalletItem } from '@/types/api'

defineProps<{
  selectedAddress: string
  password: string
  wallets: SignerWalletItem[]
  walletsLoading?: boolean
  showPassword?: boolean
  error?: string
}>()

const emit = defineEmits<{
  'update:selectedAddress': [value: string]
  'update:password': [value: string]
}>()

const { t } = useI18n()

function onAddressChange(value: string) {
  emit('update:selectedAddress', value)
}

function onPasswordInput(e: Event) {
  emit('update:password', (e.target as HTMLInputElement).value)
}
</script>

<template>
  <div class="automation-unlock">
    <div class="automation-unlock__section">
      <label class="automation-unlock__label">{{ t('settings.selectSignerAddress') }}</label>
      <WalletSelector
        :model-value="selectedAddress"
        :wallets="wallets"
        :loading="walletsLoading ?? false"
        :placeholder="t('settings.selectSignerAddress')"
        @update:model-value="onAddressChange"
      />
      <p
        v-if="!showPassword"
        class="automation-unlock__hint"
      >
        {{ t('settings.selectSignerHint') }}
      </p>
    </div>

    <div
      v-if="showPassword"
      class="automation-unlock__section automation-unlock__section--password"
    >
      <label class="automation-unlock__label">{{ t('settings.unlockPasswordLabel') }}</label>
      <p class="automation-unlock__hint">
        {{ t('settings.unlockHint') }}
      </p>
      <input
        type="password"
        class="automation-unlock__input"
        :value="password"
        :placeholder="t('orders.unlockPasswordPlaceholder')"
        autocomplete="current-password"
        autofocus
        @input="onPasswordInput"
      >
    </div>

    <p
      v-if="error"
      class="automation-unlock__error"
      role="alert"
    >
      {{ error }}
    </p>
  </div>
</template>

<style scoped>
.automation-unlock {
  display: flex;
  flex-direction: column;
  gap: var(--space-lg);
}

.automation-unlock__section {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
}

.automation-unlock__section--password {
  padding-top: var(--space-md);
  border-top: 1px solid var(--border-light);
}

.automation-unlock__label {
  font-size: var(--fs-body);
  color: var(--text-secondary);
  font-weight: 500;
  text-align: center;
}

.automation-unlock__hint {
  margin: 0;
  font-size: var(--fs-small);
  color: var(--text-secondary);
  line-height: var(--lh-body);
  text-align: center;
}

.automation-unlock__input {
  width: 100%;
  height: 40px;
  padding: 0 var(--space-md);
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

.automation-unlock__input:focus {
  border-color: var(--primary-500);
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}

.automation-unlock__input::placeholder {
  color: var(--text-disabled);
}

.automation-unlock__error {
  margin: 0;
  font-size: var(--fs-small);
  color: var(--danger);
  text-align: center;
}
</style>
