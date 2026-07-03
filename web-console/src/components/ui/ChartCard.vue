<script setup lang="ts">
withDefaults(defineProps<{
  title: string
  period?: string
  loading?: boolean
}>(), {
  period: undefined,
  loading: false,
})
</script>

<template>
  <div class="chart-card card">
    <div class="chart-card-header">
      <h3 class="chart-card-title">
        {{ title }}
      </h3>
      <div class="chart-card-actions">
        <span
          v-if="period"
          class="chart-period"
        >{{ period }}</span>
        <slot name="actions" />
      </div>
    </div>
    <div class="chart-card-body">
      <div
        v-if="loading"
        class="skeleton-chart"
      />
      <slot v-else />
    </div>
    <div
      v-if="$slots.legend"
      class="chart-card-legend"
    >
      <slot name="legend" />
    </div>
  </div>
</template>

<style scoped>
.chart-card {
  background: var(--bg-card);
  border-radius: var(--radius-lg);
  border: 1px solid var(--border-light);
  box-shadow: var(--shadow-base);
  padding: var(--space-xl);
  display: flex;
  flex-direction: column;
}

.chart-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-md);
  padding-bottom: var(--space-sm);
  border-bottom: 1px solid var(--border-light);
}

.chart-card-title {
  font-size: var(--fs-h3);
  font-weight: var(--fw-h3);
  color: var(--text-primary);
}

.chart-card-actions {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
}

.chart-period {
  font-size: var(--fs-caption);
  color: var(--text-muted);
}

.chart-card-body {
  flex: 1;
  min-height: 280px;
  position: relative;
}

.chart-card-legend {
  display: flex;
  justify-content: center;
  gap: var(--space-lg);
  padding-top: var(--space-sm);
  margin-top: var(--space-sm);
  border-top: 1px solid var(--border-light);
}

.skeleton-chart {
  position: absolute;
  inset: 0;
  background: linear-gradient(90deg, var(--gray-200) 25%, var(--gray-300) 50%, var(--gray-200) 75%);
  background-size: 200% 100%;
  animation: skeleton-shimmer 1.5s infinite;
  border-radius: var(--radius-md);
}
</style>
