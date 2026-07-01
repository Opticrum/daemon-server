import { ref, type Component, type Ref } from 'vue'

export interface ModalOptions {
  title: string
  message?: string
  content?: Component
  contentProps?: Record<string, any>
  confirmText?: string | null
  cancelText?: string | null
  danger?: boolean
  wide?: boolean
  extra?: Component
  onConfirm?: () => void | Promise<void>
  onCancel?: () => void
}

export function useModal() {
  const visible = ref(false)
  const title = ref('')
  const message = ref('')
  const content: Ref<Component | undefined> = ref(undefined)
  const contentProps: Ref<Record<string, any> | undefined> = ref(undefined)
  const confirmText = ref<string | null>('确定')
  const cancelText = ref<string | null>('取消')
  const danger = ref(false)
  const wide = ref(false)
  const extra: Ref<Component | undefined> = ref(undefined)
  const loading = ref(false)

  let pendingConfirm: (() => void | Promise<void>) | undefined

  function show(opts: ModalOptions) {
    title.value = opts.title
    message.value = opts.message || ''
    content.value = opts.content
    contentProps.value = opts.contentProps
    confirmText.value = opts.confirmText !== undefined ? opts.confirmText : '确定'
    cancelText.value = opts.cancelText !== undefined ? opts.cancelText : '取消'
    danger.value = opts.danger || false
    wide.value = opts.wide || false
    extra.value = opts.extra
    loading.value = false
    pendingConfirm = opts.onConfirm
    visible.value = true
  }

  function hide() {
    visible.value = false
    loading.value = false
    pendingConfirm = undefined
  }

  async function onConfirm() {
    if (pendingConfirm) {
      loading.value = true
      try {
        await pendingConfirm()
      } finally {
        loading.value = false
      }
    }
    hide()
  }

  function onCancel() {
    hide()
  }

  /**
   * Convenience: show a confirm dialog and return a Promise
   */
  function confirm(message: string, opts?: Partial<ModalOptions>): Promise<boolean> {
    return new Promise((resolve) => {
      show({
        title: opts?.title || '确认操作',
        message,
        danger: opts?.danger || false,
        confirmText: opts?.confirmText || '确定',
        cancelText: opts?.cancelText || '取消',
        onConfirm: () => {
          resolve(true)
        },
        onCancel: () => {
          resolve(false)
        },
      })
    })
  }

  return { visible, title, message, content, contentProps, confirmText, cancelText, danger, wide, extra, loading, show, hide, onConfirm, onCancel, confirm }
}
