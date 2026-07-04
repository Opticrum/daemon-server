import { ApiError, type RuntimeConfig, type WalletResponse, type ImportWalletRequest, type CreateHdWalletRequest, type ImportMnemonicRequest, type CreateHdWalletResponse, type UnlockWalletRequest, type UnlockWalletResponse, type HdStatusResponse, type WalletBalanceResponse, type AddressBalanceItem, type RefreshHdWalletResponse, type RefreshHdWalletRequest, type WalletSessionStatus, type OrderScanItem, type MatchOrderRequest, type MatchOrderResult, type MatchReadiness, type TrackedMatch, type MatchDetail, type ExtractRentResult, type ChannelWithMatch, type FiberChannelInfo, type FiberNodeInfoResponse, type ServerInfo, type SignerWalletItem, type PeerConnectionStatus, type SchedulerStatusResponse } from '@/types/api'

// ═══════════════════════════════════════════
// Fetch wrapper
// ═══════════════════════════════════════════

const BASE = '/api'

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  const res = await fetch(BASE + path, {
    credentials: 'same-origin',
    headers: {
      'Content-Type': 'application/json',
      ...options.headers,
    },
    ...options,
  })

  let data: any
  try {
    data = await res.json()
  } catch (e) {
    console.error('Failed to parse API response:', e)
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

    // ── HD Wallet ──
    createHdWallet: (body: CreateHdWalletRequest): Promise<CreateHdWalletResponse> =>
      request('/console/wallets/create-hd', { method: 'POST', body: JSON.stringify(body) }),
    unlockWallet: (body: UnlockWalletRequest): Promise<UnlockWalletResponse> =>
      request('/console/wallets/unlock', { method: 'POST', body: JSON.stringify(body) }),
    getWalletSession: (): Promise<WalletSessionStatus> =>
      request('/console/wallets/session'),
    lockWallet: (): Promise<{ locked: boolean }> =>
      request('/console/wallets/lock', { method: 'POST' }),
    deriveMoreAddresses: (body: { password?: string; count?: number }): Promise<WalletResponse[]> =>
      request('/console/wallets/derive-more', { method: 'POST', body: JSON.stringify(body) }),
    getHdStatus: (): Promise<HdStatusResponse> =>
      request('/console/wallets/hd-status'),
    getWalletBalance: (): Promise<WalletBalanceResponse> =>
      request('/console/wallets/balance'),
    getAddressBalances: (): Promise<AddressBalanceItem[]> =>
      request('/console/wallets/balances'),
    refreshHdWallet: (body: RefreshHdWalletRequest = {}): Promise<RefreshHdWalletResponse> =>
      request('/console/wallets/refresh-hd', { method: 'POST', body: JSON.stringify(body) }),
    getSignerWallets: (): Promise<SignerWalletItem[]> =>
      request('/console/signer/wallets'),
    importMnemonic: (body: ImportMnemonicRequest): Promise<CreateHdWalletResponse> =>
      request('/console/wallets/import-mnemonic', { method: 'POST', body: JSON.stringify(body) }),
    deleteHdWallet: (): Promise<{ deleted: boolean }> =>
      request('/console/wallets/delete-hd', { method: 'DELETE' }),
    revealMnemonic: (password: string): Promise<{ mnemonic: string }> =>
      request('/console/wallets/reveal-mnemonic', { method: 'POST', body: JSON.stringify({ password }) }),

    // ── Orders ──
    scanOrders: (): Promise<OrderScanItem[]> =>
      request('/console/orders'),
    getMatchReadiness: (txHash: string): Promise<MatchReadiness> =>
      request(`/console/orders/${encodeURIComponent(txHash)}/match-readiness`),
    createOrderChannel: (txHash: string): Promise<{ temporary_channel_id: string; peer: string; capacity: number }> =>
      request(`/console/orders/${encodeURIComponent(txHash)}/create-channel`, { method: 'POST' }),
    matchOrder: (txHash: string, body: MatchOrderRequest): Promise<MatchOrderResult> =>
      request(`/console/orders/${encodeURIComponent(txHash)}/match`, {
        method: 'POST',
        body: JSON.stringify(body),
        signal: AbortSignal.timeout(300_000),
      }),

    // ── Matches ──
    getMatchDetail: (txHash: string, outputIndex: number): Promise<MatchDetail> =>
      request(`/console/matches/${encodeURIComponent(txHash)}/${outputIndex}`),
    listMatches: (status?: string, lockHashes?: string[]): Promise<TrackedMatch[]> => {
      const params = new URLSearchParams()
      if (status) params.set('status', status)
      if (lockHashes && lockHashes.length > 0) params.set('lock_hashes', lockHashes.join(','))
      const qs = params.toString()
      return request('/console/matches' + (qs ? `?${qs}` : ''))
    },
    extractRent: (txHash: string, outputIndex: number): Promise<ExtractRentResult> =>
      request(`/console/matches/${encodeURIComponent(txHash)}/${outputIndex}/extract`, { method: 'POST' }),
    destroyMatch: (txHash: string, outputIndex: number): Promise<{ tx_hash: string; status: string }> =>
      request(`/console/matches/${encodeURIComponent(txHash)}/${outputIndex}/destroy`, { method: 'POST' }),

    // ── Channels ──
    scanChannels: (): Promise<ChannelWithMatch[]> =>
      request('/console/channels'),
    /// Fast path: channels only, no match cross-referencing.
    scanChannelsOnly: (): Promise<FiberChannelInfo[]> =>
      request('/console/channels-only'),
    /// Cross-reference channels with on-chain match cells.
    scanChannelMatches: (): Promise<ChannelWithMatch[]> =>
      request('/console/channel-matches'),
    closeChannel: (channelId: string, force?: boolean): Promise<{ closed: boolean }> =>
      request(
        `/console/channels/${encodeURIComponent(channelId)}/close`,
        { method: 'POST', body: JSON.stringify({ force: force ?? false }) },
      ),
    deleteChannel: (channelId: string): Promise<{ deleted: boolean }> =>
      request(`/console/channels/${encodeURIComponent(channelId)}`, { method: 'DELETE' }),

    // ── Peer Connection ──
    checkPeerConnection: (pubkey: string): Promise<PeerConnectionStatus> =>
      request(`/console/peers/check/${encodeURIComponent(pubkey)}`),
    connectToPeer: (pubkey: string): Promise<{ connected: boolean }> =>
      request('/console/peers/connect', { method: 'POST', body: JSON.stringify({ pubkey }) }),

    // ── Fiber Node Info ──
    getFiberNodeInfo: (): Promise<FiberNodeInfoResponse> =>
      request('/console/fiber-node-info'),

    // ── Config ──
    getRuntimeConfig: (): Promise<RuntimeConfig> =>
      request('/console/runtime-config'),
    updateRuntimeConfig: (body: Partial<RuntimeConfig>): Promise<RuntimeConfig> =>
      request('/console/runtime-config', { method: 'PUT', body: JSON.stringify(body) }),
    resetRuntimeConfig: (): Promise<RuntimeConfig> =>
      request('/console/runtime-config/reset', { method: 'POST' }),

    // ── Scheduler ──
    getSchedulerStatus: (since = 0): Promise<SchedulerStatusResponse> =>
      request(`/console/scheduler/status?since=${since}`),

    // ── Server Info ──
    getServerInfo: (): Promise<ServerInfo> =>
      request('/console/server-info'),

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
  sparklines: Record<string, number[]>
  scheduler: SchedulerStatusResponse
}

export interface KpiTrendItem {
  key: string
  current: number
  previous: number
  delta_pct: number
  delta_label: string
}
