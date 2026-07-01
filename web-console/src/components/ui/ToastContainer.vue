<script setup lang="ts">
import type { ToastMessage } from '@/composables/useToast'

defineProps<{
  messages: ToastMessage[]
}>()

defineEmits<{
  remove: [id: number]
}>()

const typeIcons: Record<string, string> = {
  success: '✅',
  error: '❌',
  warning: '⚠️',
  info: 'ℹ️',
}
</script>

<template>
  <Teleport to="body">
    <div
      class="toast-container"
      aria-live="polite"
    >
      <TransitionGroup name="toast">
        <div
          v-for="msg in messages"
          :key="msg.id"
          class="toast-item"
          :class="`toast-${msg.type}`"
          @click="$emit('remove', msg.id)"
        >
          <span class="toast-icon">{{ typeIcons[msg.type] }}</span>
          <span class="toast-message">{{ msg.message }}</span>
        </div>
      </TransitionGroup>
    </div>
  </Teleport>
</template>

<style scoped>
.toast-container {
  position: fixed;
  top: calc(var(--header-height) + var(--space-md));
  right: var(--space-xl);
  z-index: 2000;
  display: flex;
  flex-direction: column;
  gap: var(--space-xs);
  pointer-events: none;
}

.toast-item {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  padding: var(--space-sm) var(--space-lg);
  background: var(--bg-card);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  font-size: var(--fs-body);
  cursor: pointer;
  pointer-events: auto;
  min-width: 240px;
  max-width: 400px;
  border-left: 3px solid var(--gray-400);
}

.toast-success { border-left-color: var(--success); }
.toast-error   { border-left-color: var(--danger); }
.toast-warning { border-left-color: var(--warning); }
.toast-info    { border-left-color: var(--info); }

.toast-icon {
  font-size: 16px;
  flex-shrink: 0;
}

.toast-message {
  flex: 1;
  color: var(--text-primary);
  line-height: var(--lh-body);
  overflow-wrap: break-word;
  word-break: break-word;
  overflow: hidden;
}

/* Transition */
.toast-enter-active {
  transition: all 0.3s cubic-bezier(0.645, 0.045, 0.355, 1);
}
.toast-leave-active {
  transition: all 0.2s ease-in;
}
.toast-enter-from {
  opacity: 0;
  transform: translateX(40px);
}
.toast-leave-to {
  opacity: 0;
  transform: translateX(40px);
}
</style>
