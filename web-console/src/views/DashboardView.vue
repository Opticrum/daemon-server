<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useApi, type DashboardResponse } from "@/composables/useApi";
import { useI18n } from "@/composables/useI18n";
import { formatNumber, formatCKB } from "@/utils/format";
import KpiCard from "@/components/ui/KpiCard.vue";
import ChartCard from "@/components/ui/ChartCard.vue";
import TrendChart from "@/components/charts/TrendChart.vue";
import DonutChart from "@/components/charts/DonutChart.vue";

const api = useApi();
const { t } = useI18n();

const dash = ref<DashboardResponse | null>(null);
const loading = ref(true);

async function loadDashboard() {
  loading.value = true;
  try {
    dash.value = await api.getDashboard();
  } catch (e) {
    console.error("Failed to load dashboard:", e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => loadDashboard());

function findTrend(key: string) {
  return dash.value?.trends?.find((trend) => trend.key === key);
}
</script>

<template>
  <div class="dashboard">
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
        :value="
          dash
            ? `¥${formatNumber(dash.total_extracted_shannons / 100_000_000)}`
            : '—'
        "
        :unit="t('common.CKB')"
        :trend="findTrend('revenue')?.delta_pct"
        :sparkline-data="dash?.sparklines?.revenue"
      />
      <KpiCard
        :title="t('dashboard.activeOrders')"
        :value="dash?.active_orders_count ?? '—'"
        :unit="t('dashboard.orders')"
        :trend="findTrend('orders')?.delta_pct"
        :sparkline-data="dash?.sparklines?.orders"
      />
      <KpiCard
        :title="t('dashboard.availChannels')"
        :value="dash?.channel_count ?? '—'"
        :unit="t('dashboard.channels')"
        :trend="findTrend('channels')?.delta_pct"
        :sparkline-data="dash?.sparklines?.channels"
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
  </div>
</template>

<style scoped>
.dashboard {
  max-width: 1440px;
  margin: 0 auto;
}
.kpi-grid {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: var(--space-md);
  margin-bottom: var(--space-md);
}
.chart-row {
  display: grid;
  grid-template-columns: repeat(24, 1fr);
  gap: var(--space-md);
  margin-bottom: var(--space-md);
}
.chart-col-16 {
  grid-column: span 16;
}
.chart-col-8 {
  grid-column: span 8;
}
@media (max-width: 1439px) {
  .kpi-grid {
    grid-template-columns: repeat(4, 1fr);
  }
  .chart-col-16 {
    grid-column: span 16;
  }
  .chart-col-8 {
    grid-column: span 8;
  }
}
@media (max-width: 1199px) {
  .kpi-grid {
    grid-template-columns: repeat(3, 1fr);
  }
  .chart-row {
    grid-template-columns: 1fr;
  }
  .chart-col-16,
  .chart-col-8 {
    grid-column: span 1;
  }
}
@media (max-width: 991px) {
  .kpi-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}
@media (max-width: 767px) {
  .kpi-grid {
    grid-template-columns: 1fr;
  }
}
</style>
