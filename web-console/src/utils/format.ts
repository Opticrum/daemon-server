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

/** Approximate CKB blocks per year (365.25 days × 24h × 3600s / ~10s block time) */
const BLOCKS_PER_YEAR = 3_155_760

/**
 * Convert shannons per block to CKB per block, with 2 decimal places
 */
export function formatCKBPerBlock(shannons: number): string {
  return (shannons / 100_000_000).toFixed(2) + ' CKB/block'
}

/**
 * Calculate annualized yield rate from shannons_per_block and channel capacity (both in shannons)
 * APY = (CKB_per_block × blocks_per_year) / CKB_principal × 100%
 * First converts both values from shannons to CKB (÷ 100,000,000)
 */
export function formatAPY(shannonsPerBlock: number, channelCapacity: number): string {
  if (!channelCapacity || channelCapacity <= 0 || !shannonsPerBlock) return '—'
  const ckbPerBlock = shannonsPerBlock / 100_000_000
  const ckbPrincipal = channelCapacity / 100_000_000
  if (ckbPrincipal <= 0) return '—'
  const apy = (ckbPerBlock * BLOCKS_PER_YEAR) / ckbPrincipal * 100
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
 * Map status string to i18n key (for StatusTag component).
 */
export function statusLabelKey(status: string): string {
  const map: Record<string, string> = {
    live: 'status.live',
    exhausted: 'status.exhausted',
    destroyed: 'status.destroyed',
    pending: 'status.pending',
    signed: 'status.signed',
    broadcast: 'status.broadcast',
    failed: 'status.failed',
    open: 'status.open',
    closing: 'status.closing',
    closed: 'status.closed',
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
    destroyed: 'red',
    pending: 'yellow',
    signed: 'blue',
    broadcast: 'blue',
    failed: 'red',
    open: 'green',
    closing: 'yellow',
    closed: 'gray',
  }
  return map[status] || 'gray'
}
