<script setup lang="ts">
import { ref } from 'vue'
import { useChart } from '@/composables/useChart'
import type { DistributionItem } from '@/types/api'

const props = defineProps<{
  data: DistributionItem[]
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)

useChart(canvasRef, () => ({
  type: 'doughnut',
  data: {
    labels: props.data.map((d) => d.label),
    datasets: [{
      data: props.data.map((d) => d.value),
      backgroundColor: props.data.map((d) => d.color),
      borderWidth: 2,
      borderColor: '#fff',
      hoverBorderWidth: 3,
    }],
  },
  options: {
    responsive: true,
    maintainAspectRatio: false,
    cutout: '60%',
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
      tooltip: {
        callbacks: {
          label: (ctx) => ` ${ctx.label}: ${ctx.parsed}%`,
        },
      },
    },
  },
  plugins: [{
    id: 'centerText',
    afterDraw(chart) {
      const { ctx } = chart
      const total = (chart.data.datasets[0].data as number[]).reduce((a, b) => a + b, 0)
      ctx.save()
      ctx.font = 'bold 18px -apple-system, sans-serif'
      ctx.fillStyle = '#262626'
      ctx.textAlign = 'center'
      ctx.textBaseline = 'middle'
      ctx.fillText(String(total), chart.width / 2, chart.height / 2 - 8)
      ctx.font = '12px -apple-system, sans-serif'
      ctx.fillStyle = '#8c8c8c'
      ctx.fillText('总计匹配', chart.width / 2, chart.height / 2 + 14)
      ctx.restore()
    },
  }],
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
  min-height: 280px;
}
.chart-container canvas {
  width: 100% !important;
  height: 100% !important;
}
</style>
