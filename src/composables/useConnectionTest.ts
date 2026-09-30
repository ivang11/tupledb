import { computed, onUnmounted, ref } from 'vue'
import { useConnectionStore } from '@/stores/connections'
import type { Connection, ConnectionTestField, ConnectionTestProgress } from '@/types/connection'

export function useConnectionTest() {
  const store = useConnectionStore()
  const isTesting = ref(false)
  const testResult = ref<{ ok: boolean; msg: string } | null>(null)
  const progressMessage = ref('')
  const elapsedSeconds = ref(0)
  const fieldStates = ref<Partial<Record<ConnectionTestField, ConnectionTestProgress['status']>>>({})
  const testMessage = computed(() => testResult.value?.msg ?? progressMessage.value)
  let generation = 0
  let clock: ReturnType<typeof setInterval> | undefined

  function stopClock() {
    clearInterval(clock)
    clock = undefined
  }

  function resetTest() {
    generation++
    stopClock()
    isTesting.value = false
    testResult.value = null
    progressMessage.value = ''
    elapsedSeconds.value = 0
    fieldStates.value = {}
  }

  async function testConnection(connection: Connection) {
    resetTest()
    const current = generation
    isTesting.value = true
    progressMessage.value = 'Starting connection test…'
    const started = Date.now()
    clock = setInterval(() => { elapsedSeconds.value = Math.floor((Date.now() - started) / 1000) }, 1000)
    try {
      const msg = await store.testConnection(connection, progress => {
        if (current !== generation) return
        for (const field of progress.fields) fieldStates.value[field] = progress.status
        progressMessage.value = progress.message
      })
      if (current === generation) testResult.value = { ok: true, msg: msg ?? 'Connection successful' }
    } catch (error) {
      if (current === generation) testResult.value = { ok: false, msg: String(error) }
    } finally {
      if (current === generation) {
        isTesting.value = false
        for (const field of Object.keys(fieldStates.value) as ConnectionTestField[]) {
          if (fieldStates.value[field] === 'checking') delete fieldStates.value[field]
        }
        stopClock()
      }
    }
  }

  function fieldAttrs(field: ConnectionTestField) {
    const status = fieldStates.value[field]
    const classes = {
      checking: 'connection-test-field focus-visible:border-amber-500 focus-visible:ring-amber-500/25',
      success: 'connection-test-field focus-visible:border-green-400/40 focus-visible:ring-green-400/15',
      error: 'connection-test-field focus-visible:border-destructive focus-visible:ring-destructive/25',
    }
    return {
      class: status ? classes[status] : undefined,
      'data-test-state': status,
      'aria-invalid': status === 'error' ? true : undefined,
    }
  }

  onUnmounted(resetTest)
  return { isTesting, testResult, testMessage, elapsedSeconds, testConnection, resetTest, fieldAttrs }
}
