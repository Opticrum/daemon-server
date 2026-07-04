<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useApi } from '@/composables/useApi'
import { formatChainCacheAge, useChainCache } from '@/composables/useChainCache'
import { useI18n } from '@/composables/useI18n'

defineProps<{
  sidebarCollapsed: boolean
}>()

defineEmits<{
  'toggle-sidebar': []
}>()

const { t, toggle, localeLabel } = useI18n()
const api = useApi()
const {
  status,
  isRefreshing,
  refreshCache,
} = useChainCache()

const cacheAgeText = computed(() => {
  const ms = status.value?.updated_at_ms ?? 0
  return t('header.chainDataAge', { age: formatChainCacheAge(ms, t) })
})

const network = ref('testnet')

function onRefreshChainData() {
  void refreshCache().catch((e) => {
    console.error('Chain cache refresh failed:', e)
  })
}

onMounted(async () => {
  try {
    const serverInfo = await api.getServerInfo()
    network.value = serverInfo.network
  } catch (e) {
    console.warn('Failed to load server info in header:', e)
  }
})
</script>

<template>
  <header class="app-header">
    <div class="header-left">
      <button
        class="hamburger-btn"
        data-testid="sidebar-toggle"
        aria-label="Toggle sidebar"
        @click="$emit('toggle-sidebar')"
      >
        <span class="hamburger-line" />
        <span class="hamburger-line" />
        <span class="hamburger-line" />
      </button>
      <div class="brand">
        <span class="brand-icon">⚡</span>
        <span
          class="brand-name"
          data-testid="brand-name"
        >{{ t('app.title') }}</span>
      </div>
    </div>

    <div
      class="header-status"
      data-testid="chain-cache-status"
    >
      <span
        class="chain-cache-dot"
        :class="{ active: !isRefreshing, spinning: isRefreshing }"
      />
      <span class="chain-cache-age">{{ cacheAgeText }}</span>
      <button
        type="button"
        class="chain-cache-refresh"
        data-testid="chain-cache-refresh"
        :disabled="isRefreshing"
        :title="isRefreshing ? t('header.chainDataRefreshing') : t('header.chainDataRefresh')"
        @click="onRefreshChainData"
      >
        <span
          class="chain-cache-refresh-icon"
          :class="{ spinning: isRefreshing }"
        >↻</span>
        <span class="chain-cache-refresh-label">{{ isRefreshing ? t('header.chainDataRefreshing') : t('header.chainDataRefresh') }}</span>
      </button>
    </div>

    <div class="header-right">
      <button
        class="lang-toggle"
        data-testid="lang-toggle"
        :title="localeLabel"
        @click="toggle"
      >
        {{ localeLabel }}
      </button>
      <span
        class="network-badge"
        :class="network"
        data-testid="network-badge"
      >
        {{ network === 'mainnet' ? 'Mainnet' : 'Testnet' }}
      </span>
    </div>
  </header>
</template>

<style scoped>
.app-header {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 100;
  height: var(--header-height);
  background: var(--bg-header);
  border-bottom: 1px solid var(--border-light);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 var(--space-xl);
  gap: var(--space-md);
}

.header-left {
  display: flex;
  align-items: center;
  gap: var(--space-md);
  min-width: 0;
  flex-shrink: 0;
}

.header-status {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  flex: 1;
  justify-content: center;
  padding: 4px 12px;
  border-radius: var(--radius-md);
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid var(--border-light);
  max-width: 560px;
  margin: 0 auto;
}

.header-right {
  display: flex;
  align-items: center;
  gap: var(--space-md);
  flex-shrink: 0;
}

.hamburger-btn {
  display: none;
  flex-direction: column;
  justify-content: center;
  gap: 4px;
  width: 32px;
  height: 32px;
  padding: 6px;
  border: none;
  background: transparent;
  cursor: pointer;
  border-radius: var(--radius-md);
  transition: background var(--transition-base);
}
.hamburger-btn:hover {
  background: var(--gray-100);
}
.hamburger-line {
  display: block;
  width: 100%;
  height: 2px;
  background: var(--text-primary);
  border-radius: 1px;
}

.brand {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  min-width: 0;
}
.brand-icon {
  font-size: 20px;
  flex-shrink: 0;
}
.brand-name {
  font-size: var(--fs-body-l);
  font-weight: var(--fw-h3);
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chain-cache-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--text-disabled, #bfbfbf);
  flex-shrink: 0;
}
.chain-cache-dot.active {
  background: #52c41a;
  box-shadow: 0 0 0 2px rgba(82, 196, 26, 0.18);
}
.chain-cache-dot.spinning {
  background: var(--primary-500);
  animation: pulse-dot 1s ease-in-out infinite;
}

.chain-cache-age {
  font-size: var(--fs-caption);
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}

.chain-cache-refresh {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  padding: 0 10px;
  margin-left: 2px;
  border: 1px solid var(--border-dark);
  border-radius: var(--radius-sm);
  background: var(--bg-card);
  color: var(--text-secondary);
  font-size: var(--fs-caption);
  font-weight: 500;
  cursor: pointer;
  transition: all var(--transition-base);
  white-space: nowrap;
  flex-shrink: 0;
}
.chain-cache-refresh:hover:not(:disabled) {
  color: var(--primary-500);
  border-color: var(--primary-500);
  background: rgba(24, 144, 255, 0.04);
}
.chain-cache-refresh:disabled {
  opacity: 0.65;
  cursor: not-allowed;
}

.chain-cache-refresh-icon {
  display: inline-block;
  font-size: 14px;
  line-height: 1;
}
.chain-cache-refresh-icon.spinning {
  animation: spin 0.9s linear infinite;
}

.lang-toggle {
  padding: 3px 10px;
  border: 1px solid var(--border-dark);
  border-radius: var(--radius-sm);
  background: var(--bg-card);
  color: var(--text-secondary);
  font-size: var(--fs-small);
  font-weight: 600;
  cursor: pointer;
  transition: all var(--transition-base);
}
.lang-toggle:hover {
  color: var(--primary-500);
  border-color: var(--primary-500);
}

.network-badge {
  font-size: var(--fs-caption);
  padding: 3px 10px;
  border-radius: var(--radius-sm);
  font-weight: 600;
  font-family: var(--font-mono);
  text-transform: capitalize;
  border: 1px solid transparent;
}
.network-badge.mainnet {
  background: rgba(255, 77, 79, 0.1);
  color: #ff4d4f;
  border-color: rgba(255, 77, 79, 0.3);
}
.network-badge.testnet {
  background: rgba(82, 196, 26, 0.1);
  color: #52c41a;
  border-color: rgba(82, 196, 26, 0.3);
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

@keyframes pulse-dot {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.45; }
}

@media (max-width: 991px) {
  .hamburger-btn {
    display: flex;
  }
  .header-status {
    flex: 0 1 auto;
    max-width: none;
    padding: 3px 8px;
    gap: 6px;
  }
  .brand-name {
    display: none;
  }
}

@media (max-width: 640px) {
  .chain-cache-age {
    display: none;
  }
  .chain-cache-refresh-label {
    display: none;
  }
  .chain-cache-refresh {
    width: 26px;
    padding: 0;
    justify-content: center;
  }
}
</style>
