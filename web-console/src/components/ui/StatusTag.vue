<script setup lang="ts">
import { computed } from 'vue'
import { statusColor, statusLabelKey } from '@/utils/format'
import { useI18n } from '@/composables/useI18n'

const props = withDefaults(defineProps<{
  status: string
  label?: string
}>(), {
  label: undefined,
})

const { t } = useI18n()
const color = computed(() => statusColor(props.status))
const text = computed(() => props.label || t(statusLabelKey(props.status)))
</script>

<template>
  <span
    class="status-tag"
    :class="`tag-${color}`"
  >{{ text }}</span>
</template>

<style scoped>
.status-tag {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  font-size: var(--fs-small);
  font-weight: 500;
  line-height: var(--lh-small);
  white-space: nowrap;
}

.tag-green  { background: #f6ffed; color: var(--success); border: 1px solid #b7eb8f; }
.tag-red    { background: #fff2f0; color: var(--danger);  border: 1px solid #ffccc7; }
.tag-yellow { background: #fffbe6; color: #d48806;         border: 1px solid #ffe58f; }
.tag-blue   { background: #e6f7ff; color: var(--info);    border: 1px solid #91d5ff; }
.tag-gray   { background: #fafafa; color: var(--text-muted); border: 1px solid var(--border-dark); }
</style>
