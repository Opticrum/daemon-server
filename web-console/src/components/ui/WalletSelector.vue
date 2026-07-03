<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
import type { SignerWalletItem } from '@/types/api'
import { useI18n } from '@/composables/useI18n'
import { truncateAddress, formatCKB } from '@/utils/format'

const { t } = useI18n()

const props = withDefaults(defineProps<{
  modelValue: string
  wallets: SignerWalletItem[]
  placeholder?: string
  disabled?: boolean
  loading?: boolean
}>(), {
  placeholder: '',
  disabled: false,
  loading: false,
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

// ── State ──
const isOpen = ref(false)
const searchQuery = ref('')
const highlightedIndex = ref(-1)
const panelStyle = ref<Record<string, string>>({})

// Internal selected value — updated immediately on user selection so the
// trigger always reflects the current choice, even when the v-model prop
// has not yet flowed back down through the parent reactivity chain.
const selectedValue = ref(props.modelValue)

// ── Refs ──
const triggerRef = ref<HTMLElement | null>(null)
const panelRef = ref<HTMLElement | null>(null)
const searchInputRef = ref<HTMLInputElement | null>(null)
const listRef = ref<HTMLElement | null>(null)
const isAnimating = ref(false)

// ── Computed ──
const selectedWallet = computed(() =>
  props.wallets.find(w => w.ckb_address === selectedValue.value) ?? null,
)

// Sync internal value when the prop changes externally
watch(() => props.modelValue, (val) => {
  selectedValue.value = val
})

const filteredWallets = computed(() => {
  if (!searchQuery.value.trim()) return props.wallets
  const q = searchQuery.value.toLowerCase().trim()
  return props.wallets.filter(w =>
    w.ckb_address.toLowerCase().includes(q)
    || (w.label && w.label.toLowerCase().includes(q))
    || String(w.derivation_index ?? '').includes(q),
  )
})

// ── Positioning ──
function recalcPosition() {
  if (!triggerRef.value) return
  const rect = triggerRef.value.getBoundingClientRect()
  panelStyle.value = {
    position: 'fixed',
    top: `${rect.bottom + 4}px`,
    left: `${rect.left}px`,
    width: `${Math.max(rect.width, 300)}px`,
  }
}

// ── Open / Close ──
function open() {
  if (props.disabled || isOpen.value) return
  isOpen.value = true
  searchQuery.value = ''
  const idx = props.wallets.findIndex(w => w.ckb_address === selectedValue.value)
  highlightedIndex.value = idx >= 0 ? idx : -1
  recalcPosition()
  nextTick(() => {
    searchInputRef.value?.focus()
  })
}

function close() {
  if (!isOpen.value || isAnimating.value) return
  isAnimating.value = true
  panelRef.value?.classList.remove('open')
  const onDone = () => {
    isOpen.value = false
    isAnimating.value = false
    triggerRef.value?.focus()
  }
  const el = panelRef.value
  if (el) {
    el.addEventListener('transitionend', onDone, { once: true })
    // Fallback in case transitionend doesn't fire
    setTimeout(() => {
      if (isAnimating.value) onDone()
    }, 250)
  } else {
    onDone()
  }
}

function toggle() {
  if (isOpen.value) {
    close()
  } else {
    open()
  }
}

// ── Wait for v-show to render DOM, then add .open for animation ──
watch(isOpen, async (val) => {
  if (val) {
    await nextTick()
    recalcPosition()
    await nextTick()
    panelRef.value?.classList.add('open')
  }
})

// ── Selection ──
function selectWallet(w: SignerWalletItem) {
  selectedValue.value = w.ckb_address
  emit('update:modelValue', w.ckb_address)
  close()
}

// ── Scroll highlighted into view ──
function scrollToHighlighted() {
  nextTick(() => {
    const item = listRef.value?.querySelector('.option-item--highlighted')
    item?.scrollIntoView({ block: 'nearest' })
  })
}

// ── Keyboard ──
function onTriggerKeydown(e: KeyboardEvent) {
  switch (e.key) {
    case 'ArrowDown':
    case 'Enter':
      e.preventDefault()
      open()
      break
    case 'ArrowUp':
      e.preventDefault()
      open()
      break
  }
}

function onSearchKeydown(e: KeyboardEvent) {
  const len = filteredWallets.value.length
  switch (e.key) {
    case 'ArrowDown':
      e.preventDefault()
      highlightedIndex.value = len > 0
        ? (highlightedIndex.value + 1) % len
        : -1
      scrollToHighlighted()
      break
    case 'ArrowUp':
      e.preventDefault()
      highlightedIndex.value = len > 0
        ? (highlightedIndex.value - 1 + len) % len
        : -1
      scrollToHighlighted()
      break
    case 'Enter':
      e.preventDefault()
      if (highlightedIndex.value >= 0 && highlightedIndex.value < len) {
        selectWallet(filteredWallets.value[highlightedIndex.value])
      }
      break
    case 'Escape':
      e.preventDefault()
      close()
      break
  }
}

// ── Click outside ──
function onDocumentClick(e: MouseEvent) {
  if (!isOpen.value) return
  const target = e.target as HTMLElement
  if (triggerRef.value?.contains(target)) return
  if (panelRef.value?.contains(target)) return
  close()
}

// ── Resize ──
let resizeTimer: ReturnType<typeof setTimeout> | null = null
function onWindowResize() {
  if (!isOpen.value) return
  if (resizeTimer) clearTimeout(resizeTimer)
  resizeTimer = setTimeout(recalcPosition, 100)
}

// ── Lifecycle ──
onMounted(() => {
  // Bubble phase — so option @click handlers fire first, then we check "outside"
  document.addEventListener('click', onDocumentClick)
  window.addEventListener('resize', onWindowResize)
})

onUnmounted(() => {
  document.removeEventListener('click', onDocumentClick)
  window.removeEventListener('resize', onWindowResize)
  if (resizeTimer) clearTimeout(resizeTimer)
})

// ── Helper for badge display ──
function badgeLabel(w: SignerWalletItem): string {
  if (w.derivation_index != null) return `#${w.derivation_index}`
  if (w.derivation_path) {
    // Extract last segment of derivation path as short label
    const parts = w.derivation_path.split('/')
    return parts[parts.length - 1] || '#--'
  }
  return '#--'
}
</script>

<template>
  <div class="wallet-selector">
    <!-- Trigger button -->
    <button
      ref="triggerRef"
      type="button"
      class="selector-trigger"
      :class="{ 'selector-trigger--open': isOpen, 'selector-trigger--disabled': disabled }"
      :disabled="disabled"
      :aria-expanded="isOpen"
      aria-haspopup="listbox"
      role="combobox"
      @click="toggle"
      @keydown="onTriggerKeydown"
    >
      <template v-if="selectedWallet">
        <span class="trigger-address font-mono">
          {{ truncateAddress(selectedWallet.ckb_address, 14, 8) }}
        </span>
        <span class="trigger-balance">{{ formatCKB(selectedWallet.balance_shannons) }}</span>
      </template>
      <span
        v-else-if="loading"
        class="trigger-placeholder"
      >
        <span class="balance-spinner" />
        <span>{{ t('walletSelector.loading') }}</span>
      </span>
      <span
        v-else
        class="trigger-placeholder"
      >{{ placeholder || t('orders.selectSellerAddr') }}</span>
      <svg
        class="trigger-chevron"
        :class="{ 'trigger-chevron--open': isOpen }"
        width="12"
        height="12"
        viewBox="0 0 12 12"
        fill="none"
      >
        <path
          d="M3 4.5L6 7.5L9 4.5"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </button>

    <!-- Teleported dropdown panel -->
    <Teleport to="body">
      <div
        v-show="isOpen"
        ref="panelRef"
        class="dropdown-panel"
        :style="panelStyle"
        role="listbox"
        :aria-label="placeholder || t('orders.selectSellerAddr')"
        @keydown="onSearchKeydown"
      >
        <!-- Search -->
        <div class="search-wrap">
          <svg
            class="search-icon"
            width="14"
            height="14"
            viewBox="0 0 14 14"
            fill="none"
          >
            <circle
              cx="6"
              cy="6"
              r="4.5"
              stroke="currentColor"
              stroke-width="1.5"
            />
            <path
              d="M9.5 9.5L12.5 12.5"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            />
          </svg>
          <input
            ref="searchInputRef"
            v-model="searchQuery"
            type="text"
            class="search-input"
            :placeholder="t('walletSelector.searchPlaceholder')"
            @keydown="onSearchKeydown"
          >
        </div>

        <!-- Options list -->
        <div
          ref="listRef"
          class="options-list"
        >
          <template v-if="filteredWallets.length > 0">
            <div
              v-for="(w, idx) in filteredWallets"
              :key="w.id"
              class="option-item"
              :class="{
                'option-item--selected': w.ckb_address === modelValue,
                'option-item--highlighted': idx === highlightedIndex,
              }"
              role="option"
              :aria-selected="w.ckb_address === modelValue"
              @click="selectWallet(w)"
              @mouseenter="highlightedIndex = idx"
            >
              <span class="option-badge">{{ badgeLabel(w) }}</span>
              <div class="option-body">
                <span
                  class="option-address font-mono"
                  :title="w.ckb_address"
                >{{ truncateAddress(w.ckb_address, 14, 8) }}</span>
                <span
                  v-if="w.label"
                  class="option-label"
                >{{ w.label }}</span>
              </div>
              <span class="option-balance">{{ formatCKB(w.balance_shannons) }}</span>
            </div>
          </template>
          <!-- Loading skeletons -->
          <template v-else-if="loading">
            <div
              v-for="n in 3"
              :key="'skel-' + n"
              class="option-item option-item--skeleton"
            >
              <span class="skeleton-badge" />
              <div class="option-body">
                <span class="skeleton-line skeleton-line--long" />
                <span class="skeleton-line skeleton-line--short" />
              </div>
              <span class="option-balance option-balance--loading">
                <span class="balance-spinner" />
              </span>
            </div>
          </template>
          <!-- Empty / no results -->
          <div
            v-else
            class="empty-state"
          >
            <span class="empty-icon">🔍</span>
            <span>{{ t('walletSelector.noResults') }}</span>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
/* ── Wrapper (in normal flow) ── */
.wallet-selector {
  position: relative;
  width: 100%;
}

/* ── Trigger ── */
.selector-trigger {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  width: 100%;
  height: 36px;
  padding: 0 var(--space-sm);
  border: 1px solid var(--border-dark);
  border-radius: var(--radius-md);
  font-size: var(--fs-body);
  font-family: inherit;
  color: var(--text-primary);
  background: var(--bg-card);
  outline: none;
  cursor: pointer;
  transition: border-color var(--transition-base), box-shadow var(--transition-base);
  text-align: left;
}
.selector-trigger:hover:not(:disabled) {
  border-color: var(--primary-400);
}
.selector-trigger:focus-visible {
  border-color: var(--primary-500);
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}
.selector-trigger--open {
  border-color: var(--primary-500);
  box-shadow: 0 0 0 2px rgba(24, 144, 255, 0.2);
}
.selector-trigger--disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.trigger-address {
  font-size: var(--fs-caption);
  color: var(--text-primary);
  flex: 0 0 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.trigger-balance {
  font-size: var(--fs-caption);
  color: var(--primary-600);
  font-weight: 600;
  margin-left: auto;
  white-space: nowrap;
}
.trigger-placeholder {
  font-size: var(--fs-body);
  color: var(--text-disabled);
  flex: 1;
}
.trigger-chevron {
  flex-shrink: 0;
  color: var(--text-muted);
  transition: transform var(--transition-base);
}
.trigger-chevron--open {
  transform: rotate(180deg);
}

/* ── Panel (teleported, positioned via inline style) ── */
.dropdown-panel {
  z-index: 1050;
  background: var(--bg-card);
  border: 1px solid var(--border-base);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  display: flex;
  flex-direction: column;
  overflow: hidden;

  /* Animation start state */
  opacity: 0;
  transform: translateY(-4px) scale(0.98);
  transition: opacity var(--transition-base), transform var(--transition-base);
  pointer-events: none;
}
.dropdown-panel.open {
  opacity: 1;
  transform: translateY(0) scale(1);
  pointer-events: auto;
}

/* ── Search ── */
.search-wrap {
  display: flex;
  align-items: center;
  gap: var(--space-xs);
  padding: var(--space-xs) var(--space-sm);
  border-bottom: 1px solid var(--border-light);
}
.search-icon {
  flex-shrink: 0;
  color: var(--text-muted);
}
.search-input {
  flex: 1;
  border: none;
  outline: none;
  background: transparent;
  font-size: var(--fs-body);
  font-family: inherit;
  color: var(--text-primary);
  padding: 4px 0;
}
.search-input::placeholder {
  color: var(--text-disabled);
}

/* ── Options list ── */
.options-list {
  max-height: 240px;
  overflow-y: auto;
  overflow-x: hidden;
}

/* ── Option item ── */
.option-item {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  padding: var(--space-xs) var(--space-sm);
  cursor: pointer;
  border-left: 3px solid transparent;
  transition: background var(--transition-base), border-color var(--transition-base);
}
.option-item:hover,
.option-item--highlighted {
  background: var(--primary-50);
}
.option-item--selected {
  background: var(--primary-50);
  border-left-color: var(--primary-500);
}

/* Badge */
.option-badge {
  flex-shrink: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 36px;
  height: 22px;
  padding: 0 6px;
  border-radius: var(--radius-sm);
  background: var(--primary-500);
  color: #fff;
  font-size: var(--fs-small);
  font-weight: 600;
  font-family: var(--font-mono);
  white-space: nowrap;
}

/* Body (address + label) */
.option-body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.option-address {
  font-size: var(--fs-caption);
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.option-label {
  font-size: var(--fs-small);
  color: var(--text-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Balance */
.option-balance {
  flex-shrink: 0;
  font-size: var(--fs-caption);
  color: var(--primary-600);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

/* ── Empty state ── */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-xs);
  padding: var(--space-xl) var(--space-md);
  color: var(--text-muted);
  font-size: var(--fs-caption);
}
.empty-icon {
  font-size: 20px;
  opacity: 0.6;
}

/* ── Loading skeletons ── */
.option-item--skeleton {
  cursor: default;
  pointer-events: none;
}

.skeleton-badge {
  width: 36px;
  height: 22px;
  border-radius: var(--radius-sm);
  background: var(--gray-200);
  animation: skeleton-pulse 1.4s ease-in-out infinite;
  flex-shrink: 0;
}

.skeleton-line {
  height: 12px;
  border-radius: var(--radius-sm);
  background: var(--gray-200);
  animation: skeleton-pulse 1.4s ease-in-out infinite;
}
.skeleton-line--long {
  width: 140px;
}
.skeleton-line--short {
  width: 60px;
}

.balance-spinner {
  display: inline-block;
  width: 14px;
  height: 14px;
  border: 2px solid var(--gray-300);
  border-top-color: var(--primary-500);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@keyframes skeleton-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

</style>
