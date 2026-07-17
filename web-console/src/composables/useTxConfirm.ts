import { ref } from 'vue'
import { useI18n } from '@/composables/useI18n'
import { useApi } from '@/composables/useApi'
import { explorerTxUrl } from '@/utils/format'

// ── Module-level cache for the chain network (lazy, one RPC at startup) ──

let networkPromise: Promise<string> | null = null

function ensureNetwork(): Promise<string> {
  if (!networkPromise) {
    const api = useApi()
    networkPromise = api
      .getServerInfo()
      .then((i) => i.network)
      .catch(() => {
        networkPromise = null // retry next time
        return 'testnet'
      })
  }
  return networkPromise
}

// ── Options ──

export interface TxConfirmRunOptions<T> {
  title?: string
  message?: string
  /** The async action that sends the tx and (normally) resolves on confirmation. */
  action: () => Promise<T>
  /** Called after confirmation succeeds, before the overlay closes. */
  onSuccess?: (result: T) => void | Promise<void>
  /** Transaction kind — driver for filtering the pending-tx poll response. */
  kind?: string
  /** Context string the backend pending-tx registry will use for this tx
   *  (e.g. order tx hash for match; "{match_tx}:{idx}" for extract/destroy). */
  context?: string
  /** Extra info to `console.log` alongside the hash (operation + target params). */
  logInfo?: Record<string, unknown>
}

const POLL_MS = 2000

// ── Composable ──

export function useTxConfirm() {
  const { t } = useI18n()
  const api = useApi()

  const visible = ref(false)
  const title = ref('')
  const message = ref('')
  const txHash = ref('')
  const explorerUrl = ref('')

  let pollTimer: ReturnType<typeof setInterval> | null = null

  async function run<T>(opts: TxConfirmRunOptions<T>): Promise<T> {
    // Reset overlay state
    title.value = opts.title ?? t('txConfirm.title')
    message.value = opts.message ?? t('txConfirm.message')
    txHash.value = ''
    explorerUrl.value = ''
    visible.value = true

    const startedAt = Date.now()
    let sentLogged = false

    // Start polling for the pending-tx entry if we have enough context
    // to identify it from the backend registry.
    if (opts.kind && opts.context) {
      let inFlight = false

      const poll = async () => {
        if (inFlight) return
        inFlight = true
        try {
          const items = await api.getPendingTransactions()
          const match = items.find(
            (e) =>
              e.kind === opts.kind &&
              e.context === opts.context &&
              e.sent_at_ms >= startedAt - 60_000, // 60 s skew margin
          )
          if (match) {
            txHash.value = match.tx_hash
            explorerUrl.value = explorerTxUrl(
              match.tx_hash,
              await ensureNetwork(),
            )
            if (!sentLogged) {
              console.log('[tx] sent', {
                operation: opts.kind,
                context: opts.context,
                ...(opts.logInfo || {}),
                tx_hash: match.tx_hash,
                explorer: explorerUrl.value,
              })
              sentLogged = true
            }
            // Hash won't change — stop polling.
            if (pollTimer) {
              clearInterval(pollTimer)
              pollTimer = null
            }
          }
        } catch {
          // A failed poll is silently skipped — the main action is the
          // source of truth; polling is cosmetic.
        } finally {
          inFlight = false
        }
      }

      pollTimer = setInterval(poll, POLL_MS)
    }

    try {
      const result = await opts.action()
      // Log confirmation to browser console (covers the destroy fast-path
      // where polling may never observe a pending entry).
      if (
        opts.kind &&
        !sentLogged &&
        result &&
        typeof result === 'object' &&
        'tx_hash' in (result as any)
      ) {
        console.log('[tx] confirmed', {
          operation: opts.kind,
          context: opts.context,
          ...(opts.logInfo || {}),
          tx_hash: (result as any).tx_hash,
        })
      }
      if (opts.onSuccess) {
        await opts.onSuccess(result)
      }
      return result
    } finally {
      if (pollTimer) {
        clearInterval(pollTimer)
        pollTimer = null
      }
      visible.value = false
      txHash.value = ''
      explorerUrl.value = ''
    }
  }

  return { visible, title, message, txHash, explorerUrl, run }
}
