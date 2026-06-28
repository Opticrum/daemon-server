import { ApiError, type StatsResponse, type AutoMatchConfig, type WalletResponse, type ImportWalletRequest, type OrderScanItem, type MatchOrderRequest, type MatchOrderResult, type TrackedMatch, type MatchScanItem, type ExtractRentResult, type FiberChannelInfo, type UnsignedTx, type TrendDataPoint, type DistributionItem, type MonthlyDataPoint, type RankingItem, type ServerInfo } from '@/types/api'

// ═══════════════════════════════════════════
// Fetch wrapper
// ═══════════════════════════════════════════

const BASE = '/api'

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const res = await fetch(BASE + path, {
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
    ...options,
  })

  let data: any
  try {
    data = await res.json()
  } catch {
    throw new ApiError(res.status, 'parse_error', `Invalid response: ${res.status} ${res.statusText}`)
  }

  if (!res.ok) {
    throw new ApiError(res.status, data.error || 'unknown', data.message || res.statusText)
  }

  return data as T
}

// ═══════════════════════════════════════════
// Gateway API — all calls go through /api/console/*
// ═══════════════════════════════════════════

export function useApi() {
  return {
    // ── Dashboard ──
    getDashboard: (): Promise<DashboardResponse> =>
      request('/console/dashboard'),

    // ── Wallets ──
    listWallets: (): Promise<WalletResponse[]> =>
      request('/console/wallets'),
    importWallet: (body: ImportWalletRequest): Promise<WalletResponse> =>
      request('/console/wallets', { method: 'POST', body: JSON.stringify(body) }),
    deleteWallet: (id: number): Promise<{ deleted: boolean }> =>
      request(`/console/wallets/${id}`, { method: 'DELETE' }),

    // ── Orders ──
    scanOrders: (): Promise<OrderScanItem[]> =>
      request('/console/orders'),
    matchOrder: (txHash: string, body: MatchOrderRequest): Promise<MatchOrderResult> =>
      request(`/console/orders/${encodeURIComponent(txHash)}/match`, { method: 'POST', body: JSON.stringify(body) }),

    // ── Matches ──
    listMatches: (status?: string): Promise<TrackedMatch[]> =>
      request('/console/matches' + (status ? `?status=${encodeURIComponent(status)}` : '')),
    extractRent: (id: number): Promise<ExtractRentResult> =>
      request(`/console/matches/${id}/extract`, { method: 'POST' }),
    destroyMatch: (id: number): Promise<{ tx_hash: string; status: string }> =>
      request(`/console/matches/${id}/destroy`, { method: 'POST' }),

    // ── Channels ──
    scanChannels: (owner?: string): Promise<FiberChannelInfo[]> =>
      request('/console/channels' + (owner ? `?owner=${encodeURIComponent(owner)}` : '')),

    // ── Signing ──
    listUnsignedTxs: (): Promise<UnsignedTx[]> =>
      request('/console/signing'),
    getUnsignedTx: (id: string): Promise<UnsignedTx> =>
      request(`/console/signing/${encodeURIComponent(id)}`),
    submitWitnesses: (id: string, witnesses: any): Promise<{ id: string; status: string }> =>
      request(`/console/signing/${encodeURIComponent(id)}/witnesses`, { method: 'POST', body: JSON.stringify({ witnesses }) }),
    submitTx: (id: string): Promise<{ id: string; tx_hash: string; status: string }> =>
      request(`/console/signing/${encodeURIComponent(id)}/submit`, { method: 'POST' }),

    // ── Config ──
    getAutoMatchConfig: (): Promise<AutoMatchConfig> =>
      request('/console/config'),
    updateAutoMatchConfig: (body: Partial<AutoMatchConfig>): Promise<any> =>
      request('/console/config', { method: 'PUT', body: JSON.stringify(body) }),

    // ── Server Info ──
    getServerInfo: (): Promise<ServerInfo> =>
      request('/console/server-info'),

    // ── Scheduler + Signer ──
    getSchedulerStatus: (): Promise<any> =>
      request('/console/scheduler/status'),
    getSignerInfo: (): Promise<{ label: string; lock_hashes: string[] }> =>
      request('/console/signer-info'),
  }
}

// ═══════════════════════════════════════════
// Dashboard response type (from /api/console/dashboard)
// ═══════════════════════════════════════════

export interface DashboardResponse {
  total_matches: number
  live_matches: number
  exhausted_matches: number
  destroyed_matches: number
  total_extracted_shannons: number
  active_orders_count: number
  wallet_count: number
  channel_count: number
  tip_block: number
  trends: KpiTrendItem[]
  extraction_history: { date: string; value: number }[]
  match_distribution: { label: string; value: number; color: string }[]
  monthly_stats: { month: string; matches: number; revenue: number }[]
  top_sellers: { address: string; label: string; extracted: number; rating: number }[]
  sparklines: Record<string, number[]>
  scheduler: {
    extractor: { last_run: string | null; last_duration_ms: number; cycles: number; last_processed: number; last_error: string | null }
    matcher: { last_run: string | null; last_duration_ms: number; cycles: number; last_processed: number; last_error: string | null }
    tip_block: number
  }
}

export interface KpiTrendItem {
  key: string
  current: number
  previous: number
  delta_pct: number
  delta_label: string
}
