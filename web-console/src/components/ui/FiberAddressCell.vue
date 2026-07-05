<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress } from '@/utils/format'

const props = defineProps<{
  address?: string | null
}>()

const emit = defineEmits<{
  copy: [text: string]
}>()

const { t } = useI18n()

const hasAddress = computed(() => !!props.address?.trim())

const tooltipText = computed(() =>
  hasAddress.value ? String(props.address) : t('common.noFiberAddress'),
)

const displayText = computed(() =>
  hasAddress.value
    ? truncateAddress(String(props.address), 12, 8)
    : t('common.dhtOnly'),
)

function onClick() {
  if (hasAddress.value) {
    emit('copy', String(props.address))
  }
}
</script>

<template>
  <span
    class="fiber-addr-cell tooltip-trigger"
    :class="hasAddress ? 'has-addr' : 'no-addr'"
    :data-tooltip="tooltipText"
    @click="onClick"
  >
    <span
      class="addr-dot"
      :class="hasAddress ? 'filled' : 'hollow'"
    />
    <span
      class="addr-text font-mono"
      :class="{ muted: !hasAddress }"
    >{{ displayText }}</span>
  </span>
</template>

<style scoped>
.fiber-addr-cell {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: var(--fs-small);
  line-height: 1;
  max-width: 100%;
  white-space: nowrap;
  vertical-align: middle;
}

.has-addr {
  color: var(--success);
  cursor: pointer;
}

.has-addr:hover {
  color: var(--primary-500);
}

.no-addr {
  color: var(--text-disabled);
  cursor: help;
}

.addr-dot {
  display: inline-block;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.addr-dot.filled {
  background: var(--success);
}

.addr-dot.hollow {
  background: transparent;
  border: 1.5px solid currentColor;
}

.addr-text {
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
}

.addr-text.muted {
  font-weight: 400;
}

/* Custom hover tooltip — always shown on hover */
.tooltip-trigger {
  position: relative;
}

.tooltip-trigger::after {
  content: attr(data-tooltip);
  position: absolute;
  bottom: calc(100% + 10px);
  left: 50%;
  transform: translateX(-50%);
  padding: 6px 12px;
  background: rgba(0, 0, 0, 0.85);
  color: #fff;
  font-size: var(--fs-caption, 12px);
  font-weight: 400;
  line-height: 1.45;
  /* Escape narrow table-cell width so tooltip is not squeezed column-wide */
  width: max-content;
  max-width: 320px;
  white-space: normal;
  word-break: break-word;
  overflow-wrap: break-word;
  text-align: center;
  border-radius: 6px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.18);
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.2s ease 0.15s;
  z-index: 999;
}

.tooltip-trigger::before {
  content: '';
  position: absolute;
  bottom: calc(100% + 4px);
  left: 50%;
  transform: translateX(-50%);
  width: 0;
  height: 0;
  border-left: 6px solid transparent;
  border-right: 6px solid transparent;
  border-top: 6px solid rgba(0, 0, 0, 0.85);
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.2s ease 0.15s;
  z-index: 999;
}

.tooltip-trigger:hover::after,
.tooltip-trigger:hover::before {
  opacity: 1;
}
</style>
