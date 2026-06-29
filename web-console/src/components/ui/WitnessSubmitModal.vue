<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  txId: string
  txData: unknown
  operation: string
  modelValue: string
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const isValid = computed(() => {
  const val = props.modelValue
  if (!val.trim()) return null
  try {
    JSON.parse(val)
    return true
  } catch {
    return false
  }
})
</script>

<template>
  <div class="witness-modal">
    <div class="tx-info">
      <div class="info-row">
        <span class="info-label">交易 ID</span>
        <code class="font-mono">{{ txId }}</code>
      </div>
      <div class="info-row">
        <span class="info-label">操作</span>
        <span>{{ operation }}</span>
      </div>
    </div>

    <details class="tx-detail">
      <summary>查看交易数据</summary>
      <pre class="tx-data">{{ JSON.stringify(txData, null, 2) }}</pre>
    </details>

    <div class="form-group">
      <label class="form-label">粘贴已签名的 Witnesses（JSON 格式）</label>
      <textarea
        :value="modelValue"
        class="witness-textarea font-mono"
        rows="6"
        placeholder="粘贴 JSON，例如: [{&quot;lock&quot;: &quot;...&quot;, &quot;input_type&quot;: &quot;&quot;, &quot;output_type&quot;: &quot;&quot;}]"
        @input="emit('update:modelValue', ($event.target as HTMLTextAreaElement).value)"
      />
      <span
        v-if="isValid === true"
        class="form-hint text-success"
      >✓ JSON 格式正确</span>
      <span
        v-else-if="isValid === false"
        class="form-hint text-danger"
      >JSON 格式错误，请检查</span>
    </div>
  </div>
</template>

<style scoped>
.witness-modal {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}

.tx-info {
  display: flex;
  flex-direction: column;
  gap: var(--space-xs);
  padding: var(--space-sm);
  background: var(--gray-50);
  border-radius: var(--radius-md);
}

.info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: var(--fs-caption);
}
.info-label {
  color: var(--text-muted);
  flex-shrink: 0;
  margin-right: var(--space-md);
}
.info-row code {
  font-size: var(--fs-small);
  color: var(--text-secondary);
  word-break: break-all;
  text-align: right;
}

.tx-detail {
  font-size: var(--fs-caption);
}
.tx-detail summary {
  cursor: pointer;
  color: var(--primary-500);
  font-weight: 500;
}
.tx-data {
  margin-top: var(--space-xs);
  padding: var(--space-sm);
  background: var(--gray-50);
  border-radius: var(--radius-sm);
  font-size: var(--fs-small);
  max-height: 200px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-all;
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
}

.witness-textarea {
  padding: var(--space-sm);
  border: 1px solid var(--border-dark);
  border-radius: var(--radius-md);
  font-size: var(--fs-caption);
  color: var(--text-primary);
  background: var(--bg-card);
  outline: none;
  resize: vertical;
  transition: border-color var(--transition-base), box-shadow var(--transition-base);
}
.witness-textarea:focus {
  border-color: var(--primary-500);
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}
.witness-textarea::placeholder {
  color: var(--text-disabled);
}

.form-hint {
  font-size: var(--fs-caption);
}
</style>
