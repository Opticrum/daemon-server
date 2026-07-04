import { ref, computed } from 'vue'
import { useApi } from '@/composables/useApi'
import type { ChainCacheStatusResponse } from '@/types/api'

const status = ref<ChainCacheStatusResponse | null>(null)
const refreshing = ref(false)
let pollHandle: ReturnType<typeof setInterval> | null = null

export function formatChainCacheAge(updatedAtMs: number, t: (key: string) => string): string {
  if (!updatedAtMs) return t('header.chainDataJustNow')
  const secs = Math.max(0, Math.floor((Date.now() - updatedAtMs) / 1000))
  if (secs < 5) return t('header.chainDataJustNow')
  if (secs < 60) return `${secs}s`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `${mins}m`
  return `${Math.floor(mins / 60)}h`
}

/** Shared chain-cache status for the global header bar. Pages load fresh data on mount only. */
export function useChainCache() {
  const api = useApi()

  const isRefreshing = computed(
    () => refreshing.value || status.value?.refreshing === true,
  )

  async function loadStatus() {
    try {
      status.value = await api.getChainCacheStatus()
    } catch (e) {
      console.warn('Failed to load chain cache status:', e)
    }
  }

  async function refreshCache() {
    if (refreshing.value) return
    refreshing.value = true
    try {
      status.value = await api.refreshChainCache()
    } finally {
      refreshing.value = false
    }
  }

  function startPolling(intervalMs = 15000) {
    stopPolling()
    pollHandle = setInterval(() => {
      void loadStatus()
    }, intervalMs)
  }

  function stopPolling() {
    if (pollHandle) {
      clearInterval(pollHandle)
      pollHandle = null
    }
  }

  return {
    status,
    refreshing,
    isRefreshing,
    loadStatus,
    refreshCache,
    startPolling,
    stopPolling,
  }
}
