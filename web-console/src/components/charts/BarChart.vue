<script setup lang="ts">
import { ref } from 'vue'
import { useChart } from '@/composables/useChart'
import type { MonthlyDataPoint } from '@/types/api'

const props = defineProps<{
  data: MonthlyDataPoint[]
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)

useChart(canvasRef, () => ({
  type: 'bar',
  data: {
    labels: props.data.map((d) => d.month),
    datasets: [
      {
        label: '匹配数',
        data: props.data.map((d) => d.matches),
        backgroundColor: '#1890ff',
        borderRadius: 4,
        barPercentage: 0.6,
        categoryPercentage: 0.8,
      },
      {
        label: '收益 (×100 CKB)',
        data: props.data.map((d) => Math.round(d.revenue / 100)),
        backgroundColor: '#91d5ff',
        borderRadius: 4,
        barPercentage: 0.6,
        categoryPercentage: 0.8,
      },
    ],
  },
  options: {
    responsive: true,
    maintainAspectRatio: false,
    interaction: { intersect: false, mode: 'index' },
    plugins: {
      legend: {
        position: 'bottom',
        labels: {
          padding: 16,
          usePointStyle: true,
          pointStyleWidth: 8,
          color: '#595959',
          font: { size: 12 },
        },
      },
    },
    scales: {
      x: {
        grid: { display: false },
        ticks: { color: '#8c8c8c', font: { size: 11 } },
      },
      y: {
        grid: { color: '#f0f0f0' },
        ticks: { color: '#8c8c8c', font: { size: 11 } },
      },
    },
  },
}))
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
  min-height: 260px;
}
.chart-container canvas {
  width: 100% !important;
  height: 100% !important;
}
</style>
