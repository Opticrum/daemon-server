<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from '@/composables/useI18n'

defineProps<{
  collapsed: boolean
  mobileOpen?: boolean
}>()

defineEmits<{
  'toggle-collapse': []
}>()

const route = useRoute()
const router = useRouter()
const { t } = useI18n()

interface NavItem {
  hash: string
  labelKey: string
  icon: string
}

interface NavGroup {
  titleKey: string
  items: NavItem[]
}

const navGroupDefs: NavGroup[] = [
  {
    titleKey: 'nav.dataCenter',
    items: [
      { hash: 'dashboard', labelKey: 'nav.overview', icon: '📊' },
    ],
  },
  {
    titleKey: 'nav.orderData',
    items: [
      { hash: 'orders', labelKey: 'nav.onChainOrders', icon: '📋' },
      { hash: 'matches', labelKey: 'nav.matchRecords', icon: '🔗' },
    ],
  },
  {
    titleKey: 'nav.fundMgmt',
    items: [
      { hash: 'wallets', labelKey: 'nav.walletMgmt', icon: '💼' },
      { hash: 'channels', labelKey: 'nav.fiberChannels', icon: '🌐' },
    ],
  },
  {
    titleKey: 'nav.sysSettings',
    items: [
      { hash: 'settings', labelKey: 'nav.autoMatchSigning', icon: '⚙️' },
    ],
  },
]

function isActive(hash: string): boolean {
  return route.path === `/${hash}`
}

function navigate(hash: string) {
  router.push(`/${hash}`)
}
</script>

<template>
  <aside
    class="app-sidebar"
    :class="{ collapsed, 'mobile-open': mobileOpen }"
  >
    <nav class="sidebar-nav">
      <template
        v-for="group in navGroupDefs"
        :key="group.titleKey"
      >
        <div class="nav-group-title">
          {{ t(group.titleKey) }}
        </div>
        <button
          v-for="item in group.items"
          :key="item.hash"
          class="nav-item"
          :class="{ active: isActive(item.hash) }"
          :data-testid="`nav-${item.hash}`"
          @click="navigate(item.hash)"
        >
          <span class="nav-icon">{{ item.icon }}</span>
          <span class="nav-label">{{ t(item.labelKey) }}</span>
        </button>
      </template>
    </nav>
    <button
      class="collapse-btn"
      data-testid="sidebar-collapse-btn"
      :title="collapsed ? t('nav.expand') : t('nav.collapse')"
      @click="$emit('toggle-collapse')"
    >
      <span
        class="collapse-arrow"
        :class="{ flipped: collapsed }"
      >◀</span>
      <span
        v-if="!collapsed"
        class="collapse-text"
      >{{ t('nav.collapse') }}</span>
    </button>
  </aside>
</template>

<style scoped>
.app-sidebar {
  position: fixed;
  top: var(--header-height);
  left: 0;
  bottom: 0;
  width: var(--sidebar-expanded);
  background: var(--bg-sidebar);
  border-right: 1px solid var(--border-light);
  z-index: 90;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  transition: width var(--transition-slow);
}
.app-sidebar.collapsed {
  width: var(--sidebar-collapsed);
}

.sidebar-nav {
  flex: 1;
  padding: var(--space-sm) 0;
}

.nav-group-title {
  font-size: var(--fs-small);
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  padding: var(--space-md) var(--space-xl) var(--space-xs);
  white-space: nowrap;
  overflow: hidden;
}
.collapsed .nav-group-title {
  text-align: center;
  padding: var(--space-md) var(--space-xs) var(--space-xs);
  font-size: 10px;
}

.nav-item {
  display: flex;
  align-items: center;
  width: 100%;
  height: 44px;
  padding: 0 var(--space-xl);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: var(--fs-body);
  cursor: pointer;
  transition: all var(--transition-base);
  position: relative;
  text-align: left;
  white-space: nowrap;
}
.nav-item:hover {
  background: var(--gray-50);
  color: var(--text-primary);
}
.nav-item.active {
  background: var(--primary-50);
  color: var(--primary-500);
  font-weight: 500;
}
.nav-item.active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 50%;
  transform: translateY(-50%);
  width: 3px;
  height: 20px;
  background: var(--primary-500);
  border-radius: 0 2px 2px 0;
}

.nav-icon {
  font-size: 18px;
  width: 24px;
  text-align: center;
  flex-shrink: 0;
}
.nav-label {
  margin-left: var(--space-sm);
  overflow: hidden;
  text-overflow: ellipsis;
}

.collapsed .nav-item {
  justify-content: center;
  padding: 0;
}
.collapsed .nav-label {
  display: none;
}

.collapse-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-xs);
  height: 44px;
  border: none;
  border-top: 1px solid var(--border-light);
  background: transparent;
  color: var(--text-muted);
  font-size: var(--fs-caption);
  cursor: pointer;
  transition: all var(--transition-base);
  flex-shrink: 0;
}
.collapse-btn:hover {
  background: var(--gray-50);
  color: var(--text-secondary);
}
.collapse-arrow {
  font-size: 10px;
  transition: transform var(--transition-slow);
}
.collapse-arrow.flipped {
  transform: rotate(180deg);
}
.collapse-text {
  white-space: nowrap;
}
.collapsed .collapse-text {
  display: none;
}

@media (max-width: 1199px) {
  .app-sidebar {
    width: var(--sidebar-collapsed);
  }
  .nav-group-title {
    text-align: center;
    padding: var(--space-md) var(--space-xs) var(--space-xs);
    font-size: 10px;
  }
  .nav-item {
    justify-content: center;
    padding: 0;
  }
  .nav-label,
  .collapse-text {
    display: none;
  }
}

@media (max-width: 991px) {
  .app-sidebar {
    transform: translateX(-100%);
    z-index: 200;
    box-shadow: var(--shadow-lg);
  }
  .app-sidebar.mobile-open {
    transform: translateX(0);
  }
}
</style>
