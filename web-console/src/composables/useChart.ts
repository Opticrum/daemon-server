import { ref, onMounted, onUnmounted, type Ref } from 'vue'
import { Chart, type ChartConfiguration } from 'chart.js'

export function useChart(
  canvasRef: Ref<HTMLCanvasElement | null>,
  createConfig: () => ChartConfiguration,
) {
  const chart = ref<Chart | null>(null)

  onMounted(() => {
    if (!canvasRef.value) return
    chart.value = new Chart(canvasRef.value, createConfig())
  })

  onUnmounted(() => {
    if (chart.value) {
      chart.value.destroy()
      chart.value = null
    }
  })

  function update(labels: string[], data: number[]) {
    if (!chart.value) return
    chart.value.data.labels = labels
    chart.value.data.datasets[0].data = data
    chart.value.update()
  }

  return { chart, update }
}
