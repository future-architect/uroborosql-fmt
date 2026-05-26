import test from 'ava'

import { runLanguageServer } from '../index.js'

test('exports language server entrypoint', (t) => {
  t.is(typeof runLanguageServer, 'function')
})
