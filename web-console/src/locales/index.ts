import zhCN from './zh-CN'
import en from './en'

export type Locale = 'zh-CN' | 'en'

export const messages: Record<Locale, typeof zhCN> = {
  'zh-CN': zhCN,
  en,
}

export const defaultLocale: Locale = 'zh-CN'
