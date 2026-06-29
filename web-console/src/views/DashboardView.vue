<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useApi, type DashboardResponse } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'
import { formatNumber, formatCKB } from '@/utils/format'
import KpiCard from '@/components/ui/KpiCard.vue'
import ChartCard from '@/components/ui/ChartCard.vue'
import TrendChart from '@/components/charts/TrendChart.vue'
import DonutChart from '@/components/charts/DonutChart.vue'
import BarChart from '@/components/charts/BarChart.vue'
import RankingBar from '@/components/charts/RankingBar.vue'

const api = useApi()
const { t } = useI18n()

const dash = ref<DashboardResponse | null>(null)
const loading = ref(true)

onMounted(async () => {
  try {
    dash.value = await api.getDashboard()
  } catch {
    // error handled silently — dashboard shows zero state
  } finally {
    loading.value = false
  }
})

function findTrend(key: string) {
  return dash.value?.trends?.find(trend => trend.key === key)
}
</script>

<template>
  <div class="dashboard">
    <h2 class="page-title">
      {{ t('dashboard.title') }}
    </h2>

    <div class="kpi-grid">
      <KpiCard
        :title="t('dashboard.totalMatches')"
        :value="dash ? formatNumber(dash.total_matches) : '—'"
        :unit="t('dashboard.times')"
        :trend="findTrend('matches')?.delta_pct"
        :sparkline-data="dash?.sparklines?.matches"
      />
      <KpiCard
        :title="t('dashboard.monthlyRevenue')"
        :value="dash ? `¥${formatNumber(dash.total_extracted_shannons / 100_000_000)}` : '—'"
        :unit="t('common.CKB')"
        :trend="findTrend('revenue')?.delta_pct"
        :sparkline-data="dash?.sparklines?.revenue"
      />
      <KpiCard
        :title="t('dashboard.activeOrders')"
        :value="dash?.active_orders_count ?? '—'"
        :unit="t('dashboard.orders')"
        :trend="findTrend('orders')?.delta_pct"
        :sparkline-data="dash?.sparklines?.revenue"
      />
      <KpiCard
        :title="t('dashboard.availChannels')"
        :value="dash?.channel_count ?? '—'"
        :unit="t('dashboard.channels')"
        :trend="findTrend('channels')?.delta_pct"
        :sparkline-data="dash?.sparklines?.revenue"
      />
      <KpiCard
        :title="t('dashboard.totalExtracted')"
        :value="dash ? formatCKB(dash.total_extracted_shannons) : '—'"
        unit=""
        :trend="findTrend('extracted')?.delta_pct"
        :sparkline-data="dash?.sparklines?.extracted"
      />
    </div>

    <div class="chart-row">
      <ChartCard
        :title="t('dashboard.revenueTrend')"
        :period="t('dashboard.last30Days')"
        :loading="loading"
        class="chart-col-16"
      >
        <TrendChart
          v-if="dash?.extraction_history?.length"
          :data="dash.extraction_history"
        />
      </ChartCard>
      <ChartCard
        :title="t('dashboard.matchDist')"
        :loading="loading"
        class="chart-col-8"
      >
        <DonutChart
          v-if="dash?.match_distribution?.length"
          :data="dash.match_distribution"
        />
      </ChartCard>
    </div>

    <div class="chart-row">
      <ChartCard
        :title="t('dashboard.monthlyMatch')"
        :loading="loading"
        class="chart-col-14"
      >
        <BarChart
          v-if="dash?.monthly_stats?.length"
          :data="dash.monthly_stats"
        />
      </ChartCard>
      <ChartCard
        :title="t('dashboard.topSellers')"
        :loading="loading"
        class="chart-col-10"
      >
        <RankingBar
          v-if="dash?.top_sellers?.length"
          :data="dash.top_sellers"
        />
      </ChartCard>
    </div>
  </div>
</template>

<style scoped>
.dashboard { max-width: 1440px; margin: 0 auto; }
.page-title { font-size: var(--fs-h2); font-weight: var(--fw-h2); line-height: var(--lh-h2); color: var(--text-primary); margin-bottom: var(--space-xl); }
.kpi-grid { display: grid; grid-template-columns: repeat(5, 1fr); gap: var(--space-md); margin-bottom: var(--space-md); }
.chart-row { display: grid; grid-template-columns: repeat(24, 1fr); gap: var(--space-md); margin-bottom: var(--space-md); }
.chart-col-16 { grid-column: span 16; }
.chart-col-14 { grid-column: span 14; }
.chart-col-10 { grid-column: span 10; }
.chart-col-8  { grid-column: span 8; }
@media (max-width: 1439px) { .kpi-grid { grid-template-columns: repeat(4, 1fr); } .chart-col-16, .chart-col-14 { grid-column: span 16; } .chart-col-10, .chart-col-8 { grid-column: span 8; } }
@media (max-width: 1199px) { .kpi-grid { grid-template-columns: repeat(3, 1fr); } .chart-row { grid-template-columns: 1fr; } .chart-col-16, .chart-col-14, .chart-col-10, .chart-col-8 { grid-column: span 1; } }
@media (max-width: 991px) { .kpi-grid { grid-template-columns: repeat(2, 1fr); } }
@media (max-width: 767px) { .kpi-grid { grid-template-columns: 1fr; } }
</style>
