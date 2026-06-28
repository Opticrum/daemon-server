<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { Chart } from 'chart.js'
import { useI18n } from '@/composables/useI18n'

const { t } = useI18n()

const props = withDefaults(defineProps<{
  title: string
  value: number | string
  unit?: string
  trend?: number        // percentage, positive = up
  trendLabel?: string
  sparklineData?: number[]
  sparklineColor?: string
}>(), {
  unit: '',
  trend: undefined,
  trendLabel: '',
  sparklineData: undefined,
  sparklineColor: '#1890ff',
})

const sparklineCanvas = ref<HTMLCanvasElement | null>(null)
let chart: Chart | null = null

function initSparkline() {
  if (!sparklineCanvas.value || !props.sparklineData?.length) return

  chart = new Chart(sparklineCanvas.value, {
    type: 'line',
    data: {
      labels: props.sparklineData.map((_, i) => String(i)),
      datasets: [{
        data: props.sparklineData,
        borderColor: props.trend !== undefined
          ? (props.trend >= 0 ? '#52c41a' : '#ff4d4f')
          : props.sparklineColor,
        borderWidth: 1.5,
        pointRadius: 0,
        tension: 0.4,
        fill: false,
      }],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: { legend: { display: false }, tooltip: { enabled: false } },
      scales: {
        x: { display: false },
        y: { display: false, min: Math.min(...props.sparklineData) * 0.9 },
      },
    },
  })
}

onMounted(initSparkline)
onUnmounted(() => {
  if (chart) { chart.destroy(); chart = null }
})
watch(() => props.sparklineData, () => {
  if (chart) { chart.destroy(); chart = null }
  initSparkline()
})
</script>

<template>
  <div class="kpi-card">
    <div class="kpi-header">
      <span class="kpi-title">{{ title }}</span>
    </div>
    <div class="kpi-value-row">
      <span class="kpi-number">{{ value }}</span>
      <span v-if="unit" class="kpi-unit">{{ unit }}</span>
    </div>
    <div v-if="trend !== undefined" class="kpi-trend">
      <span class="trend-arrow" :class="trend >= 0 ? 'trend-up' : 'trend-down'">
        {{ trend >= 0 ? '↑' : '↓' }}
      </span>
      <span class="trend-value" :class="trend >= 0 ? 'text-success' : 'text-danger'">
        {{ Math.abs(trend) }}%
      </span>
      <span class="trend-label">{{ trendLabel || t('dashboard.prevMonth') }}</span>
    </div>
    <div v-if="sparklineData?.length" class="kpi-sparkline">
      <canvas ref="sparklineCanvas" />
    </div>
  </div>
</template>

<style scoped>
.kpi-card {
  background: linear-gradient(135deg, var(--bg-card), var(--gray-50));
  border-radius: var(--radius-xl);
  border: 1px solid var(--border-light);
  box-shadow: var(--shadow-base);
  padding: var(--space-lg) var(--space-xl);
  display: flex;
  flex-direction: column;
  gap: var(--space-xs);
  transition: box-shadow var(--transition-base), transform var(--transition-base);
}
.kpi-card:hover {
  box-shadow: var(--shadow-lg);
  transform: translateY(-2px);
}

.kpi-header {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
}
.kpi-title {
  font-size: var(--fs-body);
  color: var(--text-secondary);
}

.kpi-value-row {
  display: flex;
  align-items: baseline;
  gap: var(--space-xs);
}
.kpi-number {
  font-size: var(--fs-kpi);
  font-weight: var(--fw-kpi);
  line-height: var(--lh-kpi);
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}
.kpi-unit {
  font-size: var(--fs-kpi-unit);
  color: var(--text-muted);
}

.kpi-trend {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--fs-caption);
}
.trend-arrow {
  font-weight: 700;
}
.trend-up { color: var(--success); }
.trend-down { color: var(--danger); }
.trend-value {
  font-weight: 600;
}
.trend-label {
  color: var(--text-muted);
}

.kpi-sparkline {
  margin-top: var(--space-xs);
  height: 36px;
}
.kpi-sparkline canvas {
  width: 100% !important;
  height: 100% !important;
}
</style>
