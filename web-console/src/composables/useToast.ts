import { ref } from 'vue'

export type ToastType = 'success' | 'error' | 'warning' | 'info'

export interface ToastMessage {
  id: number
  type: ToastType
  message: string
}

let nextId = 0

export function useToast() {
  const messages = ref<ToastMessage[]>([])

  function show(type: ToastType, message: string, duration = 3000) {
    const id = nextId++
    messages.value.push({ id, type, message })
    if (duration > 0) {
      setTimeout(() => remove(id), duration)
    }
  }

  function remove(id: number) {
    messages.value = messages.value.filter((m) => m.id !== id)
  }

  function success(message: string, duration?: number) {
    show('success', message, duration)
  }
  function error(message: string, duration?: number) {
    show('error', message, duration)
  }
  function warning(message: string, duration?: number) {
    show('warning', message, duration)
  }
  function info(message: string, duration?: number) {
    show('info', message, duration)
  }

  return { messages, show, remove, success, error, warning, info }
}
