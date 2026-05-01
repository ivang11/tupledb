/**
 * Smoke test — cancel query flow in the UI.
 *
 * QueryEditor uses getQueryCancelButtonState() to drive a Cancel button whose
 * visibility and label change as the query lifecycle progresses.  This test
 * mounts a lightweight component that uses the exact same pattern and verifies
 * the visual transitions without requiring CodeMirror or Pinia setup.
 */
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, ref, computed } from 'vue'
import { getQueryCancelButtonState, shouldSurfaceQueryError } from '@/lib/queryExecutionUi'

// Minimal component that mirrors the cancel-button rendering in QueryEditor.
const QueryCancelHarness = defineComponent({
  setup() {
    const isRunning = ref(false)
    const isCancelling = ref(false)
    const activeQueryId = ref<string | null>(null)
    const errorMessage = ref<string | null>(null)

    const cancelButtonState = computed(() =>
      getQueryCancelButtonState(isRunning.value, isCancelling.value, activeQueryId.value),
    )

    function startQuery(id: string) {
      isRunning.value = true
      isCancelling.value = false
      activeQueryId.value = id
      errorMessage.value = null
    }

    function requestCancel() {
      if (!cancelButtonState.value.canRequestCancel) return
      isCancelling.value = true
    }

    function finishQuery(err?: string) {
      if (err && !shouldSurfaceQueryError(isCancelling.value)) {
        // suppress backend-side cancellation errors
      } else if (err) {
        errorMessage.value = err
      }
      isRunning.value = false
      isCancelling.value = false
      activeQueryId.value = null
    }

    return { cancelButtonState, isRunning, errorMessage, startQuery, requestCancel, finishQuery }
  },
  template: `
    <div>
      <button
        v-if="isRunning"
        data-testid="cancel-btn"
        :disabled="cancelButtonState.disabled"
        @click="requestCancel"
      >{{ cancelButtonState.label }}</button>
      <button v-else data-testid="run-btn">Run</button>
      <span v-if="errorMessage" data-testid="error">{{ errorMessage }}</span>
    </div>
  `,
})

describe('Cancel query — UI smoke test', () => {
  it('run button is shown when no query is running', () => {
    const w = mount(QueryCancelHarness)
    expect(w.find('[data-testid="run-btn"]').exists()).toBe(true)
    expect(w.find('[data-testid="cancel-btn"]').exists()).toBe(false)
  })

  it('cancel button appears when a query starts', async () => {
    const w = mount(QueryCancelHarness)
    await w.vm.startQuery('q1')
    await w.vm.$nextTick()

    expect(w.find('[data-testid="cancel-btn"]').exists()).toBe(true)
    expect(w.find('[data-testid="run-btn"]').exists()).toBe(false)
    expect(w.find('[data-testid="cancel-btn"]').text()).toBe('Cancel')
    expect((w.find('[data-testid="cancel-btn"]').element as HTMLButtonElement).disabled).toBe(false)
  })

  it('cancel button shows "Cancelling..." and is disabled after clicking', async () => {
    const w = mount(QueryCancelHarness)
    await w.vm.startQuery('q1')
    await w.vm.$nextTick()

    await w.find('[data-testid="cancel-btn"]').trigger('click')
    await w.vm.$nextTick()

    const btn = w.find('[data-testid="cancel-btn"]')
    expect(btn.text()).toBe('Cancelling...')
    expect((btn.element as HTMLButtonElement).disabled).toBe(true)
  })

  it('run button reappears after query finishes', async () => {
    const w = mount(QueryCancelHarness)
    await w.vm.startQuery('q1')
    await w.vm.finishQuery()
    await w.vm.$nextTick()

    expect(w.find('[data-testid="run-btn"]').exists()).toBe(true)
    expect(w.find('[data-testid="cancel-btn"]').exists()).toBe(false)
  })

  it('suppresses backend cancellation error when user intentionally cancelled', async () => {
    const w = mount(QueryCancelHarness)
    await w.vm.startQuery('q1')
    await w.find('[data-testid="cancel-btn"]').trigger('click')  // user clicks cancel
    await w.vm.$nextTick()

    // Backend reports "Query was killed" — should be silenced
    await w.vm.finishQuery('Query execution was interrupted')
    await w.vm.$nextTick()

    expect(w.find('[data-testid="error"]').exists()).toBe(false)
    expect(w.find('[data-testid="run-btn"]').exists()).toBe(true)
  })

  it('surfaces backend errors when query failed without user cancelling', async () => {
    const w = mount(QueryCancelHarness)
    await w.vm.startQuery('q1')
    await w.vm.$nextTick()

    await w.vm.finishQuery('Table not found: users')
    await w.vm.$nextTick()

    expect(w.find('[data-testid="error"]').text()).toBe('Table not found: users')
  })
})
