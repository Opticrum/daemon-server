<script setup lang="ts">
import { ref } from 'vue'
import { Chart } from 'chart.js'
import { useChart } from '@/composables/useChart'
import type { TrendDataPoint } from '@/types/api'

const props = defineProps<{
  data: TrendDataPoint[]
  color?: string
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)

useChart(canvasRef, () => {
  const ctx = canvasRef.value?.getContext('2d')
  const gradient = ctx
    ? ctx.createLinearGradient(0, 0, 0, 300)
    : undefined
  if (gradient) {
    gradient.addColorStop(0, 'rgba(24, 144, 255, 0.15)')
    gradient.addColorStop(1, 'rgba(24, 144, 255, 0)')
  }

  return {
    type: 'line',
    data: {
      labels: props.data.map((d) => d.date),
      datasets: [{
        label: '收益 (CKB)',
        data: props.data.map((d) => d.value),
        borderColor: props.color || '#1890ff',
        backgroundColor: gradient,
        borderWidth: 2,
        pointRadius: 3,
        pointHoverRadius: 6,
        pointBackgroundColor: '#fff',
        pointBorderColor: props.color || '#1890ff',
        pointBorderWidth: 2,
        tension: 0.4,
        fill: true,
      }],
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      interaction: { intersect: false, mode: 'index' },
      plugins: {
        legend: { display: false },
        tooltip: {
          backgroundColor: '#fff',
          titleColor: '#262626',
          bodyColor: '#595959',
          borderColor: '#e8e8e8',
          borderWidth: 1,
          padding: 12,
          cornerRadius: 8,
          displayColors: false,
        },
      },
      scales: {
        x: {
          grid: { display: false },
          ticks: { color: '#8c8c8c', font: { size: 11 } },
        },
        y: {
          grid: { color: '#f0f0f0' },
          ticks: { color: '#8c8c8c', font: { size: 11 }, callback: (v) => `${Number(v) / 1000}k` },
        },
      },
    },
  }
})
</script>

<template>
  <div class="chart-container">
    <canvas ref="canvasRef" />
  </div>
</template>

<style scoped>
.chart-container {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 280px;
}
.chart-container canvas {
  width: 100% !important;
  height: 100% !important;
}
</style>
