<script setup lang="ts">
import { computed } from 'vue'
import type { ImportWalletRequest } from '@/types/api'

const props = defineProps<{
  modelValue: ImportWalletRequest
}>()

const emit = defineEmits<{
  'update:modelValue': [value: ImportWalletRequest]
}>()

function update(field: keyof ImportWalletRequest, value: string) {
  emit('update:modelValue', { ...props.modelValue, [field]: value })
}

const hexValid = computed(() => {
  const hex = props.modelValue.private_key_hex
  if (!hex) return null // not yet entered
  return /^[0-9a-fA-F]{64}$/.test(hex)
})
</script>

<template>
  <div class="import-form">
    <div class="form-group">
      <label class="form-label">钱包标签</label>
      <input
        :value="modelValue.label"
        type="text"
        class="form-input"
        placeholder="例如：主钱包"
        @input="update('label', ($event.target as HTMLInputElement).value)"
      />
    </div>
    <div class="form-group">
      <label class="form-label">私钥 (64 位十六进制)</label>
      <input
        :value="modelValue.private_key_hex"
        type="password"
        class="form-input font-mono"
        placeholder="输入 64 位十六进制私钥"
        maxlength="64"
        @input="update('private_key_hex', ($event.target as HTMLInputElement).value)"
      />
      <span v-if="hexValid === false" class="form-hint text-danger">
        ⚠️ 私钥必须为 64 位十六进制字符
      </span>
      <span v-else-if="hexValid === true" class="form-hint text-success">
        ✓ 格式正确
      </span>
    </div>
    <div class="form-group">
      <label class="form-label">加密密码（可选）</label>
      <input
        :value="modelValue.password || ''"
        type="password"
        class="form-input"
        placeholder="留空则不加密存储"
        @input="update('password', ($event.target as HTMLInputElement).value)"
      />
    </div>
  </div>
</template>

<style scoped>
.import-form {
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
}
.form-input:focus {
  border-color: var(--primary-500);
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}
.form-input::placeholder {
  color: var(--text-disabled);
}

.form-hint {
  font-size: var(--fs-caption);
}
</style>
