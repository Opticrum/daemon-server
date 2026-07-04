<script setup lang="ts">
import { useI18n } from '@/composables/useI18n'

defineProps<{
  password: string
  error?: string
}>()

const emit = defineEmits<{
  'update:password': [value: string]
}>()

const { t } = useI18n()

function onInput(e: Event) {
  emit('update:password', (e.target as HTMLInputElement).value)
}
</script>

<template>
  <div class="automation-unlock">
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
      @input="onInput"
    >
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
  gap: var(--space-sm);
}

.automation-unlock__hint {
  margin: 0;
  font-size: var(--fs-body);
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
