import { ref, computed, inject, provide, type InjectionKey, type Ref } from 'vue'
import { messages, defaultLocale, type Locale } from '@/locales'

const STORAGE_KEY = 'opticrum-locale'

function loadLocale(): Locale {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored === 'zh-CN' || stored === 'en') return stored
  } catch { /* ignore */ }
  return defaultLocale
}

function saveLocale(locale: Locale) {
  try {
    localStorage.setItem(STORAGE_KEY, locale)
  } catch { /* ignore */ }
}

export const I18N_KEY: InjectionKey<ReturnType<typeof createI18n>> = Symbol('i18n')

function createI18n() {
  const locale = ref<Locale>(loadLocale())

  function t(key: string, params?: Record<string, string | number>): string {
    const keys = key.split('.')
    let current: any = messages[locale.value]
    for (const k of keys) {
      if (current == null) return key
      current = current[k]
    }
    let result = (current as string) ?? key
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        result = result.replace(`{${k}}`, String(v))
      }
    }
    return result
  }

  function setLocale(l: Locale) {
    locale.value = l
    saveLocale(l)
  }

  function toggle() {
    setLocale(locale.value === 'zh-CN' ? 'en' : 'zh-CN')
  }

  const localeLabel = computed(() => locale.value === 'zh-CN' ? '中文' : 'EN')

  return { locale, t, setLocale, toggle, localeLabel }
}

export function provideI18n() {
  const i18n = createI18n()
  provide(I18N_KEY, i18n)
  return i18n
}

export function useI18n() {
  const i18n = inject(I18N_KEY)
  if (!i18n) throw new Error('useI18n() must be used inside a component with provideI18n()')
  return i18n
}
