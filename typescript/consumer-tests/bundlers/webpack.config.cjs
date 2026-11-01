// The webpack half of the bundler scenario. run.sh sets BUNDLE_ENTRY and BUNDLE_OUT, and
// BUNDLE_EXTERNAL=1 for the form that leaves @skunkwerkx/hyperuuid to be installed beside the bundle.
'use strict'
const path = require('node:path')

module.exports = {
  mode: 'production',
  target: 'node',
  entry: path.resolve(process.env.BUNDLE_ENTRY),
  output: {
    path: path.dirname(path.resolve(process.env.BUNDLE_OUT)),
    filename: path.basename(process.env.BUNDLE_OUT),
  },
  externals:
    process.env.BUNDLE_EXTERNAL === '1'
      ? { '@skunkwerkx/hyperuuid': 'commonjs @skunkwerkx/hyperuuid' }
      : {},
  // A bundle that silently drops a module it could not resolve proves nothing: unresolved
  // requires stay errors, and run.sh moves to the external form when the plain one fails.
  stats: 'errors-warnings',
}
