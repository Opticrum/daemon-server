<script setup lang="ts">
import { ref } from 'vue'
import { useChart } from '@/composables/useChart'
import { truncateAddress, formatNumber } from '@/utils/format'
import type { RankingItem } from '@/types/api'

const props = defineProps<{
  data: RankingItem[]
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)

useChart(canvasRef, () => ({
  type: 'bar',
  data: {
    labels: props.data.map((d) => truncateAddress(d.address, 10, 4)),
    datasets: [{
      label: '累计提取 (CKB)',
      data: props.data.map((d) => d.extracted),
      backgroundColor: props.data.map((_, i) =>
        ['#1890ff', '#52c41a', '#faad14', '#722ed1', '#13c2c2'][i] || '#1890ff'
      ),
      borderRadius: 4,
      barPercentage: 0.5,
    }],
  },
  options: {
    indexAxis: 'y',
    responsive: true,
    maintainAspectRatio: false,
    plugins: {
      legend: { display: false },
      tooltip: {
        callbacks: {
          label: (ctx) => ` 累计提取: ${formatNumber(ctx.parsed.x ?? 0)} CKB`,
        },
      },
    },
    scales: {
      x: {
        grid: { color: '#f0f0f0' },
        ticks: { color: '#8c8c8c', font: { size: 11 }, callback: (v) => `${Number(v) / 1000}k` },
      },
      y: {
        grid: { display: false },
        ticks: {
          color: '#595959',
          font: { size: 11 },
          callback: (_v, i) => {
            const item = props.data[i]
            return item ? `${item.label} ⭐${item.rating}` : ''
          },
        },
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
