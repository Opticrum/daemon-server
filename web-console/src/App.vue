<script setup lang="ts">
import { ref, provide, onMounted } from 'vue'
import AppHeader from '@/components/layout/AppHeader.vue'
import AppSidebar from '@/components/layout/AppSidebar.vue'
import ToastContainer from '@/components/ui/ToastContainer.vue'
import BaseModal from '@/components/ui/BaseModal.vue'
import TxConfirmOverlay from '@/components/ui/TxConfirmOverlay.vue'
import { useToast } from '@/composables/useToast'
import { useModal } from '@/composables/useModal'
import { useTxConfirm } from '@/composables/useTxConfirm'
import { useChainCache } from '@/composables/useChainCache'
import { provideI18n } from '@/composables/useI18n'

provideI18n()

const { loadStatus, startPolling } = useChainCache()

const sidebarCollapsed = ref(false)
const sidebarMobileOpen = ref(false)

function toggleSidebar() {
  if (window.innerWidth <= 991) {
    sidebarMobileOpen.value = !sidebarMobileOpen.value
  } else {
    sidebarCollapsed.value = !sidebarCollapsed.value
  }
}

// Global toast & modal state
const toast = useToast()
const modal = useModal()
const txConfirm = useTxConfirm()
provide('toast', toast)
provide('modal', modal)
provide('txConfirm', txConfirm)

onMounted(async () => {
  await loadStatus()
  startPolling(15000)
})
</script>

<template>
  <div class="app-shell">
    <AppHeader
      :sidebar-collapsed="sidebarCollapsed"
      @toggle-sidebar="toggleSidebar"
    />
    <div class="app-body">
      <AppSidebar
        :collapsed="sidebarCollapsed"
        :mobile-open="sidebarMobileOpen"
        @toggle-collapse="toggleSidebar"
      />
      <main
        class="app-content"
        :class="{ 'content-expanded': sidebarCollapsed }"
      >
        <router-view />
      </main>
    </div>
    <ToastContainer
      :messages="toast.messages.value"
      @remove="toast.remove"
    />
    <BaseModal
      :visible="modal.visible.value"
      :title="modal.title.value"
      :confirm-text="modal.confirmText.value"
      :cancel-text="modal.cancelText.value"
      :danger="modal.danger.value"
      :wide="modal.wide.value"
      :extra-wide="modal.extraWide.value"
      :loading="modal.loading.value"
      :extra="modal.extra.value"
      @confirm="modal.onConfirm"
      @cancel="modal.onCancel"
    >
      <component
        :is="modal.content.value"
        v-if="modal.content.value"
        v-bind="modal.contentProps?.value || {}"
      />
      <p v-else>
        {{ modal.message.value }}
      </p>
    </BaseModal>
    <TxConfirmOverlay
      :visible="txConfirm.visible.value"
      :title="txConfirm.title.value"
      :message="txConfirm.message.value"
      :tx-hash="txConfirm.txHash.value"
      :explorer-url="txConfirm.explorerUrl.value"
    />
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
}

.app-body {
  display: flex;
  flex: 1;
  padding-top: var(--header-height);
}

.app-content {
  flex: 1;
  margin-left: var(--sidebar-expanded);
  padding: var(--space-xl);
  min-height: calc(100vh - var(--header-height));
  background: var(--bg-page);
  transition: margin-left var(--transition-slow);
}

.app-content.content-expanded {
  margin-left: var(--sidebar-collapsed);
}
</style>
