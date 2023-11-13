import test from 'ava'

import { runfmtWithSettings } from '../index.js'

test('format with settings', (t) => {
  const src = 'select A from B'
  const settings = {
    keyword_case: 'upper',
    identifier_case: 'lower',
    complement_alias: false,
  }
  const dst = runfmtWithSettings(src, JSON.stringify(settings), null)
  t.is(dst, 'SELECT\n\ta\nFROM\n\tb\n')
})
