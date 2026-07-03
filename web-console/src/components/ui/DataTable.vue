<script setup lang="ts" generic="T extends Record<string, any>">
import { ref, computed, watch, onBeforeUnmount } from 'vue'

export interface ColumnDef {
  key: string
  label: string
  sortable?: boolean
  align?: 'left' | 'right' | 'center'
  width?: string
}

import { useI18n } from '@/composables/useI18n'

const { t } = useI18n()

const props = withDefaults(defineProps<{
  columns: ColumnDef[]
  rows: T[]
  loading?: boolean
  emptyText?: string
  pageSize?: number
  expandedRowKeys?: Set<string | number>
  rowKey?: string | ((row: T) => string | number)
  /** Number of skeleton placeholder rows to show when loading with no data. */
  skeletonRows?: number
  /** When true, rows get pointer cursor and emit row-click on click. */
  clickableRows?: boolean
}>(), {
  loading: false,
  emptyText: '',
  pageSize: 15,
  expandedRowKeys: () => new Set(),
  rowKey: undefined,
  skeletonRows: 0,
  clickableRows: false,
})

const emit = defineEmits<{
  'row-click': [row: T, index: number]
}>()

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

// Staggered row entrance animation trigger
const justLoaded = ref(false)
let enterTimer: ReturnType<typeof setTimeout> | null = null

watch([() => props.loading, () => props.rows.length], ([loading, len]) => {
  if (!loading && len > 0) {
    justLoaded.value = true
    if (enterTimer) clearTimeout(enterTimer)
    enterTimer = setTimeout(() => { justLoaded.value = false }, 700)
  }
})

onBeforeUnmount(() => {
  if (enterTimer) clearTimeout(enterTimer)
})

function getRowKey(row: T, index: number): string | number {
  if (!props.rowKey) return index
  if (typeof props.rowKey === 'function') return props.rowKey(row)
  return String(row[props.rowKey] ?? index)
}

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
    <!-- Partial-loading progress bar: sliding blue bar when refreshing with data -->
    <div
      v-if="loading && rows.length > 0"
      class="table-progress-bar-wrapper"
    >
      <div class="table-progress-bar" />
    </div>
    <table class="data-table">
      <thead>
        <tr>
          <th
            v-for="col in columns"
            :key="col.key"
            :style="col.width ? { width: col.width } : {}"
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
      <tbody :class="{ 'table-entering': justLoaded }">
        <!-- Skeleton rows: loading with no data yet -->
        <tr
          v-for="i in (loading && rows.length === 0 ? (skeletonRows || 5) : 0)"
          :key="`skel-${i}`"
          class="skeleton-row"
        >
          <td
            v-for="col in columns"
            :key="col.key"
            :class="[`align-${col.align || 'left'}`, `cell-${col.key}`]"
          >
            <span class="skeleton-bar" />
          </td>
        </tr>
        <tr v-if="!loading && !rows.length">
          <td
            :colspan="columns.length"
            class="empty-cell"
          >
            {{ emptyText || t('common.noData') }}
          </td>
        </tr>
        <template
          v-for="(row, i) in paginatedRows"
          :key="i"
        >
          <tr
            :class="{
              'row-expanded': expandedRowKeys.has(getRowKey(row, i)),
              'row-clickable': clickableRows,
            }"
            @click="clickableRows && emit('row-click', row, i)"
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
          <tr
            v-if="expandedRowKeys.has(getRowKey(row, i))"
            class="expanded-row"
          >
            <td :colspan="columns.length">
              <slot
                name="expanded"
                :row="row"
              />
            </td>
          </tr>
        </template>
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
.data-table tbody tr.row-clickable {
  cursor: pointer;
}
.data-table tbody tr.row-clickable:hover td {
  background: rgba(24, 144, 255, 0.06);
}
.data-table tbody tr:last-child td {
  border-bottom: none;
}

/* Expandable rows */
.data-table tbody tr.row-expanded td {
  border-bottom: none;
}
.data-table tbody tr.expanded-row td {
  padding: 0;
  background: var(--gray-25, #fafbfc);
  border-bottom: 1px solid var(--border-light);
}
.data-table tbody tr.expanded-row:hover td {
  background: var(--gray-25, #fafbfc);
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

/* Skeleton rows */
.skeleton-row td {
  padding: var(--space-sm) var(--space-md);
  border-bottom: 1px solid var(--border-light);
}
.skeleton-bar {
  display: block;
  height: 14px;
  border-radius: var(--radius-sm);
  background-color: var(--gray-100);
  background-image: linear-gradient(
    100deg,
    transparent 40%,
    rgba(255, 255, 255, 0.55) 50%,
    transparent 60%
  );
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.6s ease-in-out infinite;
}
/* Variable widths for realistic table geometry */
.skeleton-row td:nth-child(odd) .skeleton-bar  { width: 85%; }
.skeleton-row td:nth-child(even) .skeleton-bar { width: 60%; }
.skeleton-row td:last-child .skeleton-bar       { width: 40%; }

/* Table progress bar: sliding blue gradient when refreshing with data */
.table-progress-bar-wrapper {
  height: 3px;
  background: var(--gray-100);
  position: relative;
  overflow: hidden;
}
.table-progress-bar {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  width: 35%;
  background: linear-gradient(
    90deg,
    transparent 0%,
    var(--primary-400) 50%,
    transparent 100%
  );
  animation: progress-slide 1.4s ease-in-out infinite;
  border-radius: 1.5px;
}

/* Staggered row entrance animation */
.data-table tbody.table-entering > tr:not(.skeleton-row) {
  animation: table-row-enter 0.4s ease backwards;
}
.data-table tbody.table-entering > tr:nth-child(1)  { animation-delay: 0.00s; }
.data-table tbody.table-entering > tr:nth-child(2)  { animation-delay: 0.05s; }
.data-table tbody.table-entering > tr:nth-child(3)  { animation-delay: 0.10s; }
.data-table tbody.table-entering > tr:nth-child(4)  { animation-delay: 0.15s; }
.data-table tbody.table-entering > tr:nth-child(5)  { animation-delay: 0.20s; }
.data-table tbody.table-entering > tr:nth-child(6)  { animation-delay: 0.25s; }
.data-table tbody.table-entering > tr:nth-child(7)  { animation-delay: 0.30s; }
.data-table tbody.table-entering > tr:nth-child(8)  { animation-delay: 0.35s; }
.data-table tbody.table-entering > tr:nth-child(9)  { animation-delay: 0.40s; }
.data-table tbody.table-entering > tr:nth-child(10) { animation-delay: 0.45s; }
.data-table tbody.table-entering > tr:nth-child(11) { animation-delay: 0.50s; }
.data-table tbody.table-entering > tr:nth-child(12) { animation-delay: 0.55s; }
.data-table tbody.table-entering > tr:nth-child(13) { animation-delay: 0.60s; }
.data-table tbody.table-entering > tr:nth-child(14) { animation-delay: 0.65s; }
.data-table tbody.table-entering > tr:nth-child(15) { animation-delay: 0.70s; }

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
