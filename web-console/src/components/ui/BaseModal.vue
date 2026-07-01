<script setup lang="ts">
import { watch, type Component } from 'vue'

import { useI18n } from '@/composables/useI18n'

const { t } = useI18n()

const props = withDefaults(defineProps<{
  visible: boolean
  title?: string
  confirmText?: string | null
  cancelText?: string | null
  danger?: boolean
  wide?: boolean
  loading?: boolean
  extra?: Component
}>(), {
  title: '',
  confirmText: '',
  cancelText: '',
  danger: false,
  wide: false,
  loading: false,
  extra: undefined,
})

const emit = defineEmits<{
  confirm: []
  cancel: []
}>()

// Lock body scroll when modal is open
watch(() => props.visible, (v) => {
  document.body.style.overflow = v ? 'hidden' : ''
})

function onOverlayClick(e: MouseEvent) {
  if ((e.target as HTMLElement).classList.contains('modal-overlay')) {
    emit('cancel')
  }
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    emit('cancel')
  }
}
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="modal-overlay"
      data-testid="modal-overlay"
      @click="onOverlayClick"
      @keydown="onKeydown"
    >
      <div
        class="modal-card"
        :class="{ 'modal-card--wide': wide }"
        role="dialog"
        aria-modal="true"
        :aria-label="title"
      >
        <div
          v-if="title"
          class="modal-header"
        >
          <h3 class="modal-title">
            {{ title }}
          </h3>
        </div>
        <div class="modal-body">
          <slot />
        </div>
        <div class="modal-footer">
          <template v-if="cancelText !== null">
            <button
              class="btn btn-default"
              data-testid="modal-cancel"
              :disabled="loading"
              @click="emit('cancel')"
            >
              {{ cancelText || t('common.cancel') }}
            </button>
          </template>
          <component
            :is="extra"
            v-if="extra"
          />
          <template v-if="confirmText !== null">
            <button
              class="btn"
              :class="danger ? 'btn-danger' : 'btn-primary'"
              data-testid="modal-confirm"
              :disabled="loading"
              @click="emit('confirm')"
            >
              <span
                v-if="loading"
                class="spinner"
                style="width:14px;height:14px;border-width:2px;"
              />
              {{ confirmText || t('common.confirm') }}
            </button>
          </template>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  animation: fadeIn 0.2s ease;
}

.modal-card {
  background: var(--bg-card);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  width: 90vw;
  max-width: 520px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  animation: scaleIn 0.2s cubic-bezier(0.645, 0.045, 0.355, 1);
}

.modal-card--wide {
  max-width: 680px;
}

.modal-header {
  padding: var(--space-xl) var(--space-xl) var(--space-sm);
  border-bottom: 1px solid var(--border-light);
  text-align: center;
}
.modal-title {
  font-size: var(--fs-h3);
  font-weight: var(--fw-h3);
  color: var(--text-primary);
}

.modal-body {
  padding: var(--space-xl);
  overflow-y: auto;
  overflow-x: hidden;
  flex: 1;
  min-width: 0;
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-sm);
  padding: var(--space-sm) var(--space-xl) var(--space-xl);
  border-top: 1px solid var(--border-light);
}

/* Buttons */
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
.btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-primary {
  background: var(--primary-500);
  color: #fff;
}
.btn-primary:hover:not(:disabled) {
  background: var(--primary-400);
}

.btn-default {
  background: var(--bg-card);
  color: var(--text-primary);
  border: 1px solid var(--border-dark);
}
.btn-default:hover:not(:disabled) {
  color: var(--primary-500);
  border-color: var(--primary-500);
}

.btn-danger {
  background: var(--danger);
  color: #fff;
}
.btn-danger:hover:not(:disabled) {
  background: #ff7875;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to   { opacity: 1; }
}

@keyframes scaleIn {
  from { opacity: 0; transform: scale(0.95); }
  to   { opacity: 1; transform: scale(1); }
}
</style>
