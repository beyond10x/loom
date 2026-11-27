import { computed, onMounted, onUnmounted, ref } from 'vue'

const authKey = 'loom.dashboard.viewer'
const parameters = () => new URLSearchParams(location.search)
const readFilters = () => {
  const query = parameters()
  return {view: query.get('view') || '', tab: query.get('tab') || '', agent: query.get('agent') || '', from: query.get('from') || '', to: query.get('to') || '', run: query.get('run') || ''}
}

export function useSnapshot() {
  const filters = ref(readFilters())
  const token = ref(sessionStorage.getItem(authKey) || '')
  const snapshot = ref(null)
  const state = ref('loading')
  const busy = ref(false)
  const url = computed(() => {
    const query = new URLSearchParams()
    if (filters.value.view === 'dispatch') return '/api/snapshot'
    for (const field of ['agent','from','to']) {
      if (filters.value[field]) query.set(field, filters.value[field])
    }
    return '/api/snapshot' + (query.size ? '?' + query : '')
  })
  let tag = '', timer, active
  function clear() {
    clearTimeout(timer)
    active?.abort()
    active = null
    snapshot.value = null
    state.value = 'loading'
    tag = ''
  }
  async function refresh() {
    clearTimeout(timer)
    active?.abort()
    const request = new AbortController()
    active = request
    busy.value = true
    const timeout = setTimeout(() => request.abort(), 10000)
    try {
      const headers = {}
      if (token.value) headers.Authorization = 'Bearer ' + token.value
      if (tag) headers['If-None-Match'] = tag
      const response = await fetch(url.value, {headers, signal: request.signal, cache:'no-store', redirect:'error'})
      if (request !== active) return
      if ([401,403].includes(response.status)) {
        const attempted = Boolean(token.value)
        snapshot.value = null
        tag = ''
        sessionStorage.removeItem(authKey)
        token.value = ''
        state.value = response.status === 403 || attempted ? 'denied' : 'login'
        return
      }
      if ([400,404].includes(response.status)) {
        snapshot.value = null
        tag = ''
        state.value = response.status === 404 ? 'missing' : 'invalid'
        return
      }
      if (response.status !== 304) {
        if (!response.ok) throw Error('unavailable')
        const next = await response.json()
        if (active !== request) return
        snapshot.value = next
        tag = response.headers.get('ETag') || ''
      }
      state.value = 'live'
    } catch {
      if (active === request) state.value = 'stale'
    } finally {
      clearTimeout(timeout)
      if (active === request) {
        busy.value = false
        if (!['denied','login','missing','invalid'].includes(state.value)) timer = setTimeout(refresh, 3000)
      }
    }
  }
  function operatorRequest(path, options={}) {
    return fetch(path, {...options, headers:{...options.headers, Authorization:'Bearer ' + token.value}, cache:'no-store', redirect:'error'})
  }
  function readAgentDocument(agent, document, signal) {
    return fetch('/api/agents/' + encodeURIComponent(agent) + '/' + document, {
      signal,
      headers: {Authorization: 'Bearer ' + token.value, Accept: document === 'spec' ? 'application/yaml' : 'application/json'},
      cache: 'no-store',
      redirect: 'error',
    })
  }
  function connect(credential) {
    clear()
    token.value = credential
    sessionStorage.setItem(authKey, credential)
    refresh()
  }
  function disconnect() {
    clear()
    busy.value = false
    sessionStorage.removeItem(authKey)
    token.value = ''
    state.value = 'login'
  }
  function restore() {
    const previous = url.value
    filters.value = readFilters()
    if (previous !== url.value) {
      clear()
      refresh()
    }
  }
  function href(changes) {
    const next = new URL(location.href)
    if (('agent' in changes && changes.agent !== filters.value.agent) || changes.run) next.searchParams.delete('tab')
    for (const [field, value] of Object.entries(changes)) {
      if (value) next.searchParams.set(field, value)
      else next.searchParams.delete(field)
    }
    return next.pathname + next.search
  }
  function navigate(changes) {
    history.pushState(null, '', href(changes))
    restore()
  }
  onMounted(() => {
    window.addEventListener('popstate', restore)
    refresh()
  })
  onUnmounted(() => {
    clear()
    window.removeEventListener('popstate', restore)
  })
  return {snapshot, filters, state, busy, refresh, readAgentDocument, operatorRequest, connect, disconnect, href, navigate}
}
