import assert from 'node:assert/strict'
import test from 'node:test'
import {
  getQueryCancelButtonState,
  shouldSurfaceQueryError,
} from '../src/lib/queryExecutionUi.js'

test('getQueryCancelButtonState exposes cancel affordance only for an active running query', () => {
  assert.deepEqual(getQueryCancelButtonState(false, false, null), {
    visible: false,
    disabled: false,
    label: 'Cancel',
    canRequestCancel: false,
  })

  assert.deepEqual(getQueryCancelButtonState(true, false, 'query-1'), {
    visible: true,
    disabled: false,
    label: 'Cancel',
    canRequestCancel: true,
  })

  assert.deepEqual(getQueryCancelButtonState(true, false, null), {
    visible: true,
    disabled: false,
    label: 'Cancel',
    canRequestCancel: false,
  })
})

test('getQueryCancelButtonState disables repeat cancellation while cancellation is in-flight', () => {
  assert.deepEqual(getQueryCancelButtonState(true, true, 'query-1'), {
    visible: true,
    disabled: true,
    label: 'Cancelling...',
    canRequestCancel: false,
  })
})

test('shouldSurfaceQueryError hides backend cancellation errors during user cancellation', () => {
  assert.equal(shouldSurfaceQueryError(false), true)
  assert.equal(shouldSurfaceQueryError(true), false)
})
