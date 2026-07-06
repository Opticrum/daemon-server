import { ref } from 'vue'
import { useI18n } from '@/composables/useI18n'

export interface TxConfirmRunOptions<T> {
  title?: string
  message?: string
  action: () => Promise<T>
  onSuccess?: (result: T) => void | Promise<void>
}

export function useTxConfirm() {
  const { t } = useI18n()
  const visible = ref(false)
  const title = ref('')
  const message = ref('')

  async function run<T>(opts: TxConfirmRunOptions<T>): Promise<T> {
    title.value = opts.title ?? t('txConfirm.title')
    message.value = opts.message ?? t('txConfirm.message')
    visible.value = true
    try {
      const result = await opts.action()
      if (opts.onSuccess) {
        await opts.onSuccess(result)
      }
      return result
    } finally {
      visible.value = false
    }
  }

  return { visible, title, message, run }
}
