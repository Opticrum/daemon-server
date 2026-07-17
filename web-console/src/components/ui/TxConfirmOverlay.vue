<script setup lang="ts">
import { watch } from 'vue'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress } from '@/utils/format'

const props = withDefaults(defineProps<{
  visible: boolean
  title?: string
  message?: string
  txHash?: string
  explorerUrl?: string
}>(), {
  title: '',
  message: '',
  txHash: '',
  explorerUrl: '',
})

const { t } = useI18n()

watch(() => props.visible, (v) => {
  document.body.style.overflow = v ? 'hidden' : ''
})
</script>

<template>
  <Teleport to="body">
    <div
      v-if="visible"
      class="tx-confirm-overlay"
      data-testid="tx-confirm-overlay"
      aria-live="polite"
      aria-busy="true"
    >
      <div
        class="tx-confirm-card"
        role="alertdialog"
        aria-modal="true"
        :aria-label="title"
      >
        <span class="tx-confirm-spinner" />
        <h3
          v-if="title"
          class="tx-confirm-title"
        >
          {{ title }}
        </h3>
        <p
          v-if="message"
          class="tx-confirm-message"
        >
          {{ message }}
        </p>
        <div
          v-if="txHash"
          class="tx-confirm-hash"
        >
          <span class="tx-confirm-hash-label">{{ t('txConfirm.txHash') }}</span>
          <a
            class="tx-confirm-hash-link"
            :href="explorerUrl"
            target="_blank"
            rel="noopener noreferrer"
          >{{ truncateAddress(txHash, 10, 8) }}</a>
          <p class="tx-confirm-hash-hint">
            {{ t('txConfirm.sentHint') }}
          </p>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.tx-confirm-overlay {
  position: fixed;
  inset: 0;
  z-index: 1100;
  background: rgba(0, 0, 0, 0.58);
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: auto;
  animation: txConfirmFadeIn 0.2s ease;
}

.tx-confirm-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-md);
  padding: var(--space-xl) var(--space-2xl);
  min-width: 280px;
  max-width: 420px;
  background: var(--bg-card);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  animation: txConfirmScaleIn 0.2s cubic-bezier(0.645, 0.045, 0.355, 1);
}

.tx-confirm-spinner {
  display: inline-block;
  width: 40px;
  height: 40px;
  border: 3px solid var(--border-light);
  border-top-color: var(--primary-500);
  border-radius: 50%;
  animation: txConfirmSpin 0.8s linear infinite;
}

.tx-confirm-title {
  margin: 0;
  font-size: var(--fs-h3);
  font-weight: var(--fw-h3);
  color: var(--text-primary);
  text-align: center;
}

.tx-confirm-message {
  margin: 0;
  font-size: var(--fs-body);
  color: var(--text-secondary);
  text-align: center;
  line-height: 1.5;
}
.tx-confirm-hash {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-xs);
  margin-top: var(--space-sm);
}
.tx-confirm-hash-label {
  font-size: var(--fs-caption);
  font-weight: 600;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.tx-confirm-hash-link {
  font-family: var(--font-mono, monospace);
  font-size: var(--fs-body);
  color: var(--primary-500);
  text-decoration: none;
  word-break: break-all;
}
.tx-confirm-hash-link:hover {
  text-decoration: underline;
}
.tx-confirm-hash-hint {
  margin: 0;
  font-size: var(--fs-caption);
  color: var(--text-secondary);
  text-align: center;
}

@keyframes txConfirmFadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

@keyframes txConfirmScaleIn {
  from { opacity: 0; transform: scale(0.95); }
  to { opacity: 1; transform: scale(1); }
}

@keyframes txConfirmSpin {
  to { transform: rotate(360deg); }
}
</style>
