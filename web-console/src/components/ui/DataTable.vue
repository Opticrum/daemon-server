<script setup lang="ts" generic="T extends Record<string, any>">
import { ref, computed } from 'vue'

export interface ColumnDef {
  key: string
  label: string
  sortable?: boolean
  align?: 'left' | 'right' | 'center'
}

import { useI18n } from '@/composables/useI18n'

const { t } = useI18n()

const props = withDefaults(defineProps<{
  columns: ColumnDef[]
  rows: T[]
  loading?: boolean
  emptyText?: string
  pageSize?: number
}>(), {
  loading: false,
  emptyText: '',
  pageSize: 15,
})

const sortKey = ref<string | null>(null)
const sortDir = ref<'asc' | 'desc'>('asc')
const currentPage = ref(1)

const sortedRows = computed(() => {
  if (!sortKey.value) return props.rows
  const key = sortKey.value
  const dir = sortDir.value === 'asc' ? 1 : -1
  return [...props.rows].sort((a, b) => {
    const va = a[key]
    const vb = b[key]
    if (typeof va === 'number' && typeof vb === 'number') return (va - vb) * dir
    return String(va).localeCompare(String(vb)) * dir
  })
})

const totalPages = computed(() => Math.max(1, Math.ceil(sortedRows.value.length / props.pageSize)))

const paginatedRows = computed(() => {
  const start = (currentPage.value - 1) * props.pageSize
  return sortedRows.value.slice(start, start + props.pageSize)
})

function toggleSort(key: string) {
  if (sortKey.value === key) {
    sortDir.value = sortDir.value === 'asc' ? 'desc' : 'asc'
  } else {
    sortKey.value = key
    sortDir.value = 'asc'
  }
  currentPage.value = 1
}

function goPage(p: number) {
  if (p >= 1 && p <= totalPages.value) {
    currentPage.value = p
  }
}

// Pagination page numbers to display
const pageNumbers = computed(() => {
  const pages: (number | '...')[] = []
  const total = totalPages.value
  if (total <= 7) {
    for (let i = 1; i <= total; i++) pages.push(i)
    return pages
  }
  pages.push(1)
  if (currentPage.value > 3) pages.push('...')
  for (let i = Math.max(2, currentPage.value - 1); i <= Math.min(total - 1, currentPage.value + 1); i++) {
    pages.push(i)
  }
  if (currentPage.value < total - 2) pages.push('...')
  pages.push(total)
  return pages
})
</script>

<template>
  <div class="table-wrapper">
    <table class="data-table">
      <thead>
        <tr>
          <th
            v-for="col in columns"
            :key="col.key"
            :class="{ sortable: col.sortable, [`align-${col.align || 'left'}`]: true }"
            @click="col.sortable && toggleSort(col.key)"
          >
            <span class="th-label">{{ col.label }}</span>
            <span
              v-if="col.sortable && sortKey === col.key"
              class="sort-arrow"
            >
              {{ sortDir === 'asc' ? '▲' : '▼' }}
            </span>
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-if="loading">
          <td
            :colspan="columns.length"
            class="loading-cell"
          >
            <span class="spinner" /> {{ t('common.loading') }}
          </td>
        </tr>
        <tr v-else-if="!rows.length">
          <td
            :colspan="columns.length"
            class="empty-cell"
          >
            {{ emptyText || t('common.noData') }}
          </td>
        </tr>
        <tr
          v-for="(row, i) in paginatedRows"
          :key="i"
        >
          <td
            v-for="col in columns"
            :key="col.key"
            :class="[`align-${col.align || 'left'}`, `cell-${col.key}`]"
          >
            <slot
              :name="`cell-${col.key}`"
              :row="row"
              :value="row[col.key]"
            >
              {{ row[col.key] }}
            </slot>
          </td>
        </tr>
      </tbody>
    </table>

    <div
      v-if="totalPages > 1 && !loading && rows.length"
      class="pagination"
    >
      <button
        class="page-btn"
        :disabled="currentPage === 1"
        @click="goPage(currentPage - 1)"
      >
        ‹
      </button>
      <template
        v-for="p in pageNumbers"
        :key="p"
      >
        <span
          v-if="p === '...'"
          class="page-ellipsis"
        >...</span>
        <button
          v-else
          class="page-btn"
          :class="{ active: p === currentPage }"
          @click="goPage(p)"
        >
          {{ p }}
        </button>
      </template>
      <button
        class="page-btn"
        :disabled="currentPage === totalPages"
        @click="goPage(currentPage + 1)"
      >
        ›
      </button>
    </div>
  </div>
</template>

<style scoped>
.table-wrapper {
  background: var(--bg-card);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-light);
  overflow: hidden;
}

.data-table {
  width: 100%;
  border-collapse: collapse;
}
.data-table th {
  background: var(--gray-50);
  color: var(--text-secondary);
  font-weight: 500;
  font-size: var(--fs-caption);
  padding: var(--space-sm) var(--space-md);
  border-bottom: 1px solid var(--border-light);
  white-space: nowrap;
  user-select: none;
}
.data-table th.sortable {
  cursor: pointer;
}
.data-table th.sortable:hover {
  background: var(--gray-100);
}
.th-label {
  margin-right: 4px;
}
.sort-arrow {
  font-size: 10px;
  color: var(--primary-500);
}
.data-table td {
  padding: var(--space-sm) var(--space-md);
  border-bottom: 1px solid var(--border-light);
  font-size: var(--fs-body);
  color: var(--text-primary);
}
.data-table tbody tr:hover td {
  background: var(--gray-50);
}
.data-table tbody tr:last-child td {
  border-bottom: none;
}

.data-table th.align-left,
.data-table td.align-left { text-align: left; }
.data-table th.align-right,
.data-table td.align-right { text-align: right; font-variant-numeric: tabular-nums; }
.data-table th.align-center,
.data-table td.align-center { text-align: center; }

.loading-cell,
.empty-cell {
  text-align: center !important;
  padding: var(--space-3xl) var(--space-md) !important;
  color: var(--text-muted);
}
.loading-cell {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-xs);
}

.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: var(--space-md);
  border-top: 1px solid var(--border-light);
}

.page-btn {
  min-width: 32px;
  height: 32px;
  padding: 0 var(--space-xs);
  border: 1px solid var(--border-dark);
  border-radius: var(--radius-md);
  background: var(--bg-card);
  color: var(--text-secondary);
  font-size: var(--fs-body);
  cursor: pointer;
  transition: all var(--transition-base);
}
.page-btn:hover:not(:disabled):not(.active) {
  color: var(--primary-500);
  border-color: var(--primary-500);
}
.page-btn.active {
  background: var(--primary-500);
  border-color: var(--primary-500);
  color: #fff;
}
.page-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.page-ellipsis {
  padding: 0 4px;
  color: var(--text-muted);
}
</style>
