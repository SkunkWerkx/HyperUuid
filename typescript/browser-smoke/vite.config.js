// The WASI loader uses top-level await, so the bundle targets esnext. Relative asset paths
// let the built page be served from any directory, as CI's python http.server does.
export default {
  base: './',
  build: { target: 'esnext' },
}
