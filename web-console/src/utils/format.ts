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
