import { ref, shallowRef } from 'vue'
import { useApi } from './useApi'
import type { FiberNodeInfo, FiberNodeInfoResponse } from '@/types/api'

/**
 * Fetch Fiber node metadata through the backend proxy (`GET /api/console/fiber-node-info`).
 *
 * The backend handles the JSON-RPC call to the Fiber node, avoiding CORS issues.
 *
 * Usage:
 *   const { rpcUrl, nodeInfo, connected, loading, error, fetchNodeInfo } = useFiber()
 *   await fetchNodeInfo()
 */
export function useFiber() {
  const api = useApi()

  const rpcUrl = ref('')
  const nodeInfo = shallowRef<FiberNodeInfo | null>(null)
  const connected = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function fetchNodeInfo(): Promise<void> {
    loading.value = true
    error.value = null
    connected.value = false

    try {
      const resp: FiberNodeInfoResponse = await api.getFiberNodeInfo()
      rpcUrl.value = resp.rpc_url
      if (resp.node_info) {
        nodeInfo.value = resp.node_info
        connected.value = true
      } else {
        nodeInfo.value = null
        connected.value = false
      }
    } catch (e: any) {
      console.error('Failed to fetch fiber node info:', e)
      error.value = e?.message || String(e)
      connected.value = false
      nodeInfo.value = null
    } finally {
      loading.value = false
    }
  }

  return {
    rpcUrl,
    nodeInfo,
    connected,
    loading,
    error,
    fetchNodeInfo,
  }
}
