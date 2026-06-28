<script setup lang="ts">
import type { MatchOrderRequest } from '@/types/api'

const props = defineProps<{
  modelValue: MatchOrderRequest & { tx_hash?: string }
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
    <div v-if="modelValue.tx_hash" class="form-group">
      <label class="form-label">订单交易哈希</label>
      <code class="form-static font-mono">{{ modelValue.tx_hash }}</code>
    </div>
    <div class="form-group">
      <label class="form-label">卖家地址</label>
      <input
        :value="modelValue.seller_address"
        type="text"
        class="form-input font-mono"
        placeholder="ckb1q..."
        @input="update('seller_address', ($event.target as HTMLInputElement).value)"
      />
    </div>
    <div class="form-group">
      <label class="form-label">通道 Outpoint 交易哈希</label>
      <input
        :value="modelValue.channel_outpoint_tx_hash"
        type="text"
        class="form-input font-mono"
        placeholder="0x..."
        @input="update('channel_outpoint_tx_hash', ($event.target as HTMLInputElement).value)"
      />
    </div>
    <div class="form-group">
      <label class="form-label">通道 Outpoint 输出索引</label>
      <input
        :value="modelValue.channel_outpoint_index"
        type="number"
        class="form-input"
        min="0"
        @input="update('channel_outpoint_index', Number(($event.target as HTMLInputElement).value))"
      />
    </div>
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
