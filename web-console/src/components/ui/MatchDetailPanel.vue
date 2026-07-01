<script setup lang="ts">
import { inject } from 'vue'
import StatusTag from '@/components/ui/StatusTag.vue'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress } from '@/utils/format'

export interface DetailField {
  label: string
  value: string
  /** Full value for tooltip; defaults to value */
  title?: string
  type?: 'text' | 'mono' | 'hash' | 'status'
  copyable?: boolean
  href?: string
}

export interface DetailSection {
  title: string
  fields: DetailField[]
}

export interface ExtractionHistoryRow {
  amount: string
  block: string
  txHash: string
  timestamp: string
}

defineProps<{
  sections: DetailSection[]
  extractionHistory?: {
    title: string
    headers: [string, string, string, string]
    rows: ExtractionHistoryRow[]
    emptyText: string
  }
}>()

const toast = inject<{ success: (msg: string) => void }>('toast')
const { t } = useI18n()

async function copyValue(value: string) {
  try {
    await navigator.clipboard.writeText(value)
  } catch {
    const ta = document.createElement('textarea')
    ta.value = value
    ta.style.position = 'fixed'
    ta.style.opacity = '0'
    document.body.appendChild(ta)
    ta.select()
    document.execCommand('copy')
    document.body.removeChild(ta)
  }
  toast?.success?.(t('common.copied'))
}
</script>

<template>
  <div class="match-detail-panel">
    <section
      v-for="(section, idx) in sections"
      :key="idx"
      class="detail-section"
    >
      <h4 class="detail-section-title">
        {{ section.title }}
      </h4>
      <dl class="detail-grid">
        <div
          v-for="(field, fieldIdx) in section.fields"
          :key="fieldIdx"
          class="detail-row"
        >
          <dt class="detail-label">
            {{ field.label }}
          </dt>
          <dd
            class="detail-value"
            :class="{
              'detail-value--mono': field.type === 'mono' || field.type === 'hash',
            }"
          >
            <StatusTag
              v-if="field.type === 'status'"
              :status="field.value"
            />
            <a
              v-else-if="field.href"
              :href="field.href"
              target="_blank"
              rel="noopener noreferrer"
              class="detail-link"
              :title="field.title || field.value"
            >
              {{ field.type === 'hash' ? truncateAddress(field.value, 12, 8) : field.value }}
            </a>
            <button
              v-else-if="field.copyable || field.type === 'hash'"
              type="button"
              class="detail-hash"
              :title="field.title || field.value"
              @click="copyValue(field.value)"
            >
              <span class="detail-hash-text">
                {{ field.type === 'hash' ? truncateAddress(field.value, 12, 8) : field.value }}
              </span>
              <span
                class="detail-copy-icon"
                aria-hidden="true"
              >⎘</span>
            </button>
            <span
              v-else
              :title="field.title"
            >{{ field.value }}</span>
          </dd>
        </div>
      </dl>
    </section>

    <section
      v-if="extractionHistory"
      class="detail-section"
    >
      <h4 class="detail-section-title">
        {{ extractionHistory.title }}
      </h4>
      <div class="detail-table-wrap">
        <table class="detail-table">
          <thead>
            <tr>
              <th
                v-for="(header, hIdx) in extractionHistory.headers"
                :key="hIdx"
              >
                {{ header }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-if="!extractionHistory.rows.length"
            >
              <td
                colspan="4"
                class="detail-table-empty"
              >
                {{ extractionHistory.emptyText }}
              </td>
            </tr>
            <tr
              v-for="(row, rIdx) in extractionHistory.rows"
              v-else
              :key="rIdx"
            >
              <td>{{ row.amount }}</td>
              <td>{{ row.block }}</td>
              <td>
                <button
                  type="button"
                  class="detail-hash detail-hash--compact"
                  :title="row.txHash"
                  @click="copyValue(row.txHash)"
                >
                  <span class="detail-hash-text">{{ truncateAddress(row.txHash, 8, 6) }}</span>
                  <span
                    class="detail-copy-icon"
                    aria-hidden="true"
                  >⎘</span>
                </button>
              </td>
              <td>{{ row.timestamp }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>

<style scoped>
.match-detail-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}

.detail-section {
  background: var(--gray-50);
  border: 1px solid var(--border-light);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.detail-section-title {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  margin: 0;
  padding: var(--space-sm) var(--space-md);
  font-size: var(--fs-caption);
  font-weight: 600;
  color: var(--text-primary);
  background: var(--bg-card);
  border-bottom: 1px solid var(--border-light);
}

.detail-section-title::before {
  content: '';
  width: 3px;
  height: 14px;
  background: var(--primary-500);
  border-radius: 2px;
  flex-shrink: 0;
}

.detail-grid {
  margin: 0;
  padding: var(--space-xs) var(--space-md) var(--space-sm);
}

.detail-row {
  display: grid;
  grid-template-columns: minmax(96px, 34%) 1fr;
  gap: var(--space-sm) var(--space-md);
  padding: var(--space-sm) 0;
  align-items: start;
}

.detail-row + .detail-row {
  border-top: 1px dashed var(--border-base);
}

.detail-label {
  margin: 0;
  font-size: var(--fs-caption);
  color: var(--text-secondary);
  line-height: var(--lh-caption);
}

.detail-value {
  margin: 0;
  font-size: var(--fs-body);
  color: var(--text-primary);
  line-height: var(--lh-body);
  text-align: right;
  min-width: 0;
  word-break: break-word;
}

.detail-value--mono {
  font-family: var(--font-mono);
  font-size: var(--fs-caption);
}

.detail-link {
  color: var(--primary-500);
  text-decoration: none;
  font-family: var(--font-mono);
  font-size: var(--fs-caption);
}

.detail-link:hover {
  text-decoration: underline;
}

.detail-hash {
  display: inline-flex;
  align-items: center;
  gap: var(--space-xs);
  max-width: 100%;
  padding: 2px 8px;
  background: var(--bg-card);
  border: 1px solid var(--border-base);
  border-radius: var(--radius-sm);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: var(--fs-caption);
  cursor: pointer;
  transition: border-color var(--transition-base), color var(--transition-base);
}

.detail-hash:hover {
  border-color: var(--primary-300);
  color: var(--primary-500);
}

.detail-hash--compact {
  padding: 1px 6px;
}

.detail-hash-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail-copy-icon {
  flex-shrink: 0;
  font-size: var(--fs-small);
  color: var(--text-muted);
  line-height: 1;
}

.detail-hash:hover .detail-copy-icon {
  color: var(--primary-500);
}

.detail-table-wrap {
  overflow-x: auto;
  padding: 0 var(--space-md) var(--space-sm);
}

.detail-table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--fs-caption);
}

.detail-table th {
  background: var(--bg-card);
  color: var(--text-secondary);
  font-weight: 500;
  padding: 8px 10px;
  border-bottom: 1px solid var(--border-light);
  text-align: left;
  white-space: nowrap;
}

.detail-table td {
  padding: 8px 10px;
  border-bottom: 1px solid var(--border-light);
  color: var(--text-primary);
}

.detail-table tbody tr:last-child td {
  border-bottom: none;
}

.detail-table tbody tr:hover td {
  background: rgba(255, 255, 255, 0.65);
}

.detail-table-empty {
  text-align: center;
  color: var(--text-disabled);
  padding: var(--space-md) 0 !important;
  font-style: italic;
}
</style>
