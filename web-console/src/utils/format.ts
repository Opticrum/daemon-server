/**
 * Format a number with thousands separators (1,234,567)
 */
export function formatNumber(n: number, decimals = 0): string {
  return n.toLocaleString('zh-CN', {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  })
}

/**
 * Convert shannons to CKB and format
 */
export function formatCKB(shannons: number): string {
  return (shannons / 100_000_000).toFixed(2) + ' CKB'
}

/** CKB blocks per year (~12s block time). Matches opticrum-calculator config. */
const BLOCKS_PER_YEAR = 2_629_800

/**
 * Convert shannons per block to CKB per block, with 2 decimal places
 */
export function formatCKBPerBlock(shannons: number): string {
  return (shannons / 100_000_000).toFixed(2) + ' CKB/block'
}

/**
 * Calculate annualized yield rate from shannons_per_block and channel capacity (both in shannons).
 * Matches opticrum-calculator: rent_per_block_to_annual_yield
 * Formula: (shannonsPerBlock × BLOCKS_PER_YEAR) / channelCapacity × 100%
 */
export function formatAPY(shannonsPerBlock: number, channelCapacity: number): string {
  if (!channelCapacity || channelCapacity <= 0 || !shannonsPerBlock) return '—'
  const apy = (shannonsPerBlock * BLOCKS_PER_YEAR) / channelCapacity * 100
  if (apy < 0.01) return apy.toFixed(6) + '%'
  return apy.toFixed(2) + '%'
}

/**
 * Truncate hex address for display: "0x1234...abcd"
 */
export function truncateAddress(addr: string, prefix = 6, suffix = 6): string {
  if (!addr || addr.length <= prefix + suffix + 3) return addr || '—'
  return addr.slice(0, prefix) + '...' + addr.slice(-suffix)
}

/**
 * Format channel age in days from a Unix millisecond timestamp.
 * Returns "—" for null/undefined/0 values.
 * Shows 1 decimal for < 1 day, whole number for ≥ 1 day.
 */
export function formatChannelAge(ms: number | null | undefined): string {
  if (ms == null || ms === 0) return '—'
  const days = (Date.now() - ms) / (1000 * 60 * 60 * 24)
  if (days < 1) return days.toFixed(1) + 'd'
  return Math.floor(days) + 'd'
}

/**
 * Build a blockchain explorer URL for a transaction hash.
 * Hardcoded base URLs for CKB mainnet and testnet explorers.
 */
export function explorerTxUrl(txHash: string, network: string): string {
  const base =
    network === 'mainnet'
      ? 'https://explorer.nervos.org/transaction/'
      : 'https://testnet.explorer.nervos.org/transaction/'
  const prefixed = txHash.startsWith('0x') ? txHash : '0x' + txHash
  return base + prefixed
}

// fiber-json-types CloseFlags bitmask values (src: fiber-json-types channel.rs:80-85)
export const CF_COOPERATIVE = 1 // 1 << 0
export const CF_UNCOOPERATIVE_LOCAL = 2 // 1 << 1
export const CF_ABANDONED = 4 // 1 << 2
export const CF_FUNDING_ABORTED = 8 // 1 << 3
export const CF_UNCOOPERATIVE_REMOTE = 16 // 1 << 4
export const CF_WAITING_ONCHAIN = 32 // 1 << 5

/**
 * Map status string to i18n key (for StatusTag component).
 */
export function statusLabelKey(status: string): string {
  const map: Record<string, string> = {
    live: 'status.live',
    exhausted: 'status.exhausted',
    timeout: 'status.timeout',
    destroyed: 'status.destroyed',
    forceClosing: 'status.forceClosing',
    forceClosed: 'status.forceClosed',
    closing: 'status.closing',
    closed: 'status.closed',
    pending: 'status.pending',
    signed: 'status.signed',
    broadcast: 'status.broadcast',
    failed: 'status.failed',
    open: 'status.open',
  }
  return map[status] || status
}

/**
 * Map status string to tag color variant
 */
export function statusColor(status: string): 'green' | 'red' | 'yellow' | 'blue' | 'gray' {
  const map: Record<string, 'green' | 'red' | 'yellow' | 'blue' | 'gray'> = {
    live: 'green',
    exhausted: 'gray',
    timeout: 'red',
    destroyed: 'red',
    forceClosing: 'yellow',
    forceClosed: 'red',
    closing: 'yellow',
    closed: 'gray',
    pending: 'yellow',
    signed: 'blue',
    broadcast: 'blue',
    failed: 'red',
    open: 'green',
  }
  return map[status] || 'gray'
}
