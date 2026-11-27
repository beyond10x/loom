<script setup>
import {onBeforeUnmount, ref} from 'vue'

const props = defineProps({agent: Object, read: Function})
const open = ref(false)
const source = ref('')
const revision = ref('')
const message = ref('')
const loading = ref(false)
let pending

function close() {
  pending?.abort()
  pending = undefined
  open.value = false
  source.value = ''
  revision.value = ''
  message.value = ''
  loading.value = false
}

async function show() {
  close()
  open.value = true
  loading.value = true
  const request = new AbortController()
  pending = request
  const timeout = setTimeout(() => request.abort(), 10000)
  try {
    const response = await props.read(props.agent.id, request.signal)
    if (request !== pending) return
    if (!response.ok) {
      const reasons = {
        401: 'Your viewer session has expired. Sign in again.',
        403: 'Only a server operator can view the AgentSpec source.',
        404: 'No AgentSpec source is available for this agent.',
      }
      message.value = reasons[response.status] || 'The server could not read the AgentSpec source.'
      return
    }
    const bytes = await response.arrayBuffer()
    if (request !== pending) return
    source.value = new TextDecoder('utf-8', {fatal:true, ignoreBOM:true}).decode(bytes)
    revision.value = response.headers.get('ETag')?.match(/^"([0-9a-f]{64})"$/)?.[1] || ''
  } catch {
    if (request === pending) message.value = 'Could not load the AgentSpec source.'
  } finally {
    clearTimeout(timeout)
    if (request === pending) loading.value = false
  }
}
onBeforeUnmount(close)
</script>

<template>
  <div class="spec" data-address="pages/agent/sections/source">
    <button type="button" :aria-expanded="open" aria-controls="agent-spec-source" @click="open ? close() : show()">
      {{ open ? 'Hide AgentSpec' : 'View AgentSpec' }}
    </button>
    <div v-if="open" id="agent-spec-source" role="region" aria-label="AgentSpec source">
      <p v-if="loading" role="status">Loading AgentSpec…</p>
      <p v-else-if="message" role="status">{{ message }} <button type="button" @click="show">Retry</button></p>
      <template v-else>
        <p>Source revision <code>{{ revision || 'Unknown' }}</code></p>
        <pre tabindex="0" aria-label="AgentSpec YAML"><code v-text="source"></code></pre>
      </template>
    </div>
  </div>
</template>
