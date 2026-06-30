<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useApi } from '@/composables/useApi'
import { useI18n } from '@/composables/useI18n'

defineProps<{
  sidebarCollapsed: boolean
}>()

defineEmits<{
  'toggle-sidebar': []
}>()

const { t, toggle, localeLabel } = useI18n()
const api = useApi()
const network = ref('testnet')

onMounted(async () => {
  try {
    const serverInfo = await api.getServerInfo()
    network.value = serverInfo.network
  } catch (e) {
    console.warn('Failed to load server info in header:', e);
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
}

.header-left {
  display: flex;
  align-items: center;
  gap: var(--space-md);
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
}
.brand-icon {
  font-size: 20px;
}
.brand-name {
  font-size: var(--fs-body-l);
  font-weight: var(--fw-h3);
  color: var(--text-primary);
}

.header-right {
  display: flex;
  align-items: center;
  gap: var(--space-md);
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

@media (max-width: 991px) {
  .hamburger-btn {
    display: flex;
  }
}
</style>
