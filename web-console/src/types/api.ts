// ═══════════════════════════════════════════
// API Response Types — mirrors backend JSON
// ═══════════════════════════════════════════

export interface ApiErrorResponse {
  error: string
  message: string
}

export class ApiError extends Error {
  constructor(
    public status: number,
    public kind: string,
    message: string,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

// ── Admin ──
export interface StatsResponse {
  matches: {
    total: number
    live: number
    exhausted: number
    destroyed: number
  }
  total_extracted_shannons: number
}

export interface AutoMatchConfig {
  enabled: boolean
  min_capacity_shannons: number
  max_escrow_blocks: number
  interval_secs: number
}

// ── Server Info ──
export interface ServerInfo {
  network: string
  ckb_rpc_url: string
  ckb_indexer_url: string
  fiber_rpc_url: string
  fee_rate: number
  version: string
}

// ── Wallets ──
export interface WalletResponse {
  id: number
  label: string
  lock_hash: string
  ckb_address: string
  created_at: string
  parent_wallet_id: number | null
  derivation_path: string | null
  derivation_index: number | null
  wallet_type: 'imported' | 'hd_child'
}

export interface ImportWalletRequest {
  label: string
  private_key_hex: string
  password?: string
}

// ── HD Wallet ──
export interface CreateHdWalletRequest {
  label: string
  password: string
  address_count?: number
}

export interface ImportMnemonicRequest {
  mnemonic: string
  label: string
  password: string
  address_count?: number
}

export interface CreateHdWalletResponse {
  keystore: any
  mnemonic: string
  children: WalletResponse[]
  address_count: number
}

export interface UnlockWalletRequest {
  password: string
}

export interface UnlockWalletResponse {
  keystore: any
  children: WalletResponse[]
}

export interface HdStatusResponse {
  keystore_exists: boolean
  label: string | null
  address_count: number
}

export interface WalletBalanceResponse {
  total_balance_shannons: number
}

export interface AddressBalanceItem {
  wallet: WalletResponse
  balance_shannons: number
}

export interface RefreshHdWalletResponse {
  keystore: {
    label: string
    address_count: number
  }
  children: WalletResponse[]
  total_balance_shannons: number
  address_balances: AddressBalanceItem[]
}

// ── Orders ──
export interface OrderScanItem {
  tx_hash: string
  output_index: number
  fiber_pubkey: string
  buyer_lock_hash: string
  xudt_amount: number
  channel_capacity: number
  shannons_per_block: number
  ckb_capacity: number
}

export interface MatchOrderRequest {
  order_output_index: number
  seller_address: string
  channel_outpoint_tx_hash: string
  channel_outpoint_index: number
}

export interface MatchOrderResult {
  tx_hash: string
  output_index: number
  match_id: number
}

// ── Matches ──
export interface TrackedMatch {
  id: number
  tx_hash: string
  output_index: number
  order_tx_hash: string
  order_output_index: number
  seller_address: string
  shannons_per_block: number
  last_extraction_block: number
  xudt_amount: number
  status: string
  created_at: string
}

export interface MatchScanItem {
  tx_hash: string
  output_index: number
  order_tx_hash: string
  order_output_index: number
  fiber_pubkey: string
  buyer_lock_hash: string
  seller_lock_hash: string
  channel_outpoint_tx_hash: string
  channel_outpoint_index: number
  xudt_amount: number
  shannons_per_block: number
  last_extraction_block: number
  ckb_capacity: number
  match_current_block: number
}

export interface ExtractRentResult {
  tx_hash: string
  extracted_amount: number
  is_exhausted: boolean
}

// ── Fiber Channels ──
export interface FiberChannelInfo {
  channel_id: string
  counterparty_fiber_key: string
  tx_hash: string
  output_index: number
  capacity: number
  local_balance: number
  remote_balance: number
  state_name: string
  is_public: boolean
  enabled: boolean
}

export interface ChannelMatchInfo {
  match_tx_hash: string
  match_output_index: number
  xudt_amount: number
  shannons_per_block: number
  last_extraction_block: number
  ckb_capacity: number
  seller_lock_hash: string
}

export interface ChannelWithMatch extends FiberChannelInfo {
  match_info: ChannelMatchInfo | null
  match_status: 'matched' | 'not_found'
}

// ── Fiber Node Info (matches backend proxy response) ──
export interface FiberNodeInfoResponse {
  rpc_url: string
  node_info: FiberNodeInfo | null
}

export interface FiberNodeInfo {
  version: string
  commit_hash: string
  pubkey: string
  node_name?: string
  addresses: string[]
  chain_hash: string
  channel_count: string    // hex
  pending_channel_count: string  // hex
  peers_count: string      // hex
  tlc_expiry_delta: string // hex
  tlc_min_value: string    // hex
  udt_cfg_infos: Record<string, unknown>[]
}

// ── Unsigned Transactions ──
export interface UnsignedTx {
  id: string
  operation: string
  tx_data_json: string
  status: string
  signed_witnesses_json: string | null
  tx_hash: string | null
  created_at: string
}

// ═══════════════════════════════════════════
// Mock Data Types (for Dashboard charts)
// ═══════════════════════════════════════════

export interface TrendDataPoint {
  date: string
  value: number
}

export interface DistributionItem {
  label: string
  value: number
  color: string
}

export interface MonthlyDataPoint {
  month: string
  matches: number
  revenue: number
}

export interface RankingItem {
  address: string
  label: string
  extracted: number
  rating: number
}
