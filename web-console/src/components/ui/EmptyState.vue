<script setup lang="ts">
import { useI18n } from '@/composables/useI18n'

const { t } = useI18n()

withDefaults(defineProps<{
  icon?: string
  message?: string
  actionLabel?: string
}>(), {
  icon: '📭',
  message: '',
  actionLabel: undefined,
})

defineEmits<{
  action: []
}>()
</script>

<template>
  <div
    class="empty-state"
    data-testid="empty-state"
  >
    <span class="empty-icon">{{ icon }}</span>
    <p class="empty-message">
      {{ message || t('common.noData') }}
    </p>
    <button
      v-if="actionLabel"
      class="btn btn-primary btn-sm"
      @click="$emit('action')"
    >
      {{ actionLabel }}
    </button>
  </div>
</template>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-4xl) var(--space-xl);
  text-align: center;
}

.empty-icon {
  font-size: 48px;
  margin-bottom: var(--space-md);
  opacity: 0.6;
}

.empty-message {
  font-size: var(--fs-body);
  color: var(--text-muted);
  margin-bottom: var(--space-lg);
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
.btn-sm {
  height: 28px;
  font-size: var(--fs-caption);
  padding: 0 var(--space-sm);
}
</style>
