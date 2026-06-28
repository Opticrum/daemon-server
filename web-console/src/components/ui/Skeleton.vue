<script setup lang="ts">
withDefaults(defineProps<{
  type?: 'card' | 'table' | 'text'
  rows?: number
  cols?: number
}>(), {
  type: 'card',
  rows: 3,
  cols: 4,
})
</script>

<template>
  <div v-if="type === 'card'" class="skeleton-cards">
    <div v-for="i in cols" :key="i" class="skeleton-card">
      <div class="skeleton-line w-40" />
      <div class="skeleton-line w-60 skeleton-line-lg" />
      <div class="skeleton-line w-80" />
    </div>
  </div>
  <div v-else-if="type === 'table'" class="skeleton-table">
    <div v-for="i in rows" :key="i" class="skeleton-row">
      <div v-for="j in 4" :key="j" class="skeleton-cell">
        <div class="skeleton-line" :style="{ width: (40 + Math.random() * 40).toFixed(0) + '%' }" />
      </div>
    </div>
  </div>
  <div v-else class="skeleton-text">
    <div v-for="i in rows" :key="i" class="skeleton-line" :style="{ width: (60 + Math.random() * 40).toFixed(0) + '%' }" />
  </div>
</template>

<style scoped>
.skeleton-line {
  height: 14px;
  border-radius: 4px;
  background: linear-gradient(90deg, var(--gray-200) 25%, var(--gray-300) 50%, var(--gray-200) 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
  margin-bottom: 8px;
}
.skeleton-line-lg { height: 32px; margin-bottom: 12px; }

.skeleton-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: var(--space-md);
}
.skeleton-card {
  padding: var(--space-xl);
  background: var(--bg-card);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-light);
}

.skeleton-table {
  background: var(--bg-card);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-light);
  overflow: hidden;
}
.skeleton-row {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--space-md);
  padding: var(--space-sm) var(--space-xl);
  border-bottom: 1px solid var(--border-light);
}
.skeleton-cell {
  padding: var(--space-xs) 0;
}

.skeleton-text {
  padding: var(--space-md) 0;
}

.w-40 { width: 40%; }
.w-60 { width: 60%; }
.w-80 { width: 80%; }

@keyframes shimmer {
  0%   { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}
</style>
