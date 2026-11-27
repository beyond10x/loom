<script setup>
import {nextTick,onBeforeUnmount,ref,shallowRef,watch} from 'vue'
import SpecProfile from './SpecProfile.vue'
import {parseProfile} from './profile.js'

const props=defineProps({agent:Object,read:Function,requested:Boolean})
const emit=defineEmits(['toggle'])
const source=ref(''), revision=ref(''), message=ref(''), loading=ref(false)
const profile=shallowRef(null), profileError=ref(''), routing=shallowRef(null), routingState=ref('loading')
const activeTab=ref('profile'), firstTab=ref(), secondTab=ref(), opener=ref()
let pending

function clear() {
  pending?.abort()
  pending=undefined
  source.value=''
  revision.value=''
  profile.value=null
  routing.value=null
  routingState.value='loading'
  profileError.value=''
  message.value=''
  activeTab.value='profile'
  loading.value=false
}
async function readRouting(request) {
  try {
    const response=await props.read(props.agent.id,'routing',request.signal)
    if (!response.ok) throw Error()
    const value=await response.json()
    if (request===pending) { routing.value=value; routingState.value='ready' }
  } catch {
    if (request===pending) routingState.value='unavailable'
  }
}
async function show() {
  clear()
  loading.value=true
  const request=new AbortController()
  pending=request
  const timeout=setTimeout(()=>request.abort(),10000)
  try {
    const response=await props.read(props.agent.id,'spec',request.signal)
    if (request!==pending) return
    if (!response.ok) {
      const reasons={401:'Your viewer session has expired. Sign in again.',403:'Only a server operator can view the AgentSpec source.',404:'No AgentSpec source is available for this agent.'}
      message.value=reasons[response.status] || 'The server could not read the AgentSpec source.'
      return
    }
    const bytes=await response.arrayBuffer()
    if (request!==pending) return
    source.value=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true}).decode(bytes)
    revision.value=response.headers.get('ETag')?.match(/^"([0-9a-f]{64})"$/)?.[1] || ''
    try { profile.value=parseProfile(source.value) }
    catch { profileError.value='This document cannot be displayed as a profile. Open Source YAML to inspect the exact file.' }
    loading.value=false
    await nextTick()
    if (request!==pending) return
    firstTab.value?.focus({preventScroll:true})
    firstTab.value?.scrollIntoView({block:'start'})
    await readRouting(request)
  } catch {
    if (request===pending) message.value='Could not load the AgentSpec source.'
  } finally {
    clearTimeout(timeout)
    if (request===pending) loading.value=false
  }
}
function switchTab(value) {
  activeTab.value=value
  ;(value==='profile'?firstTab.value:secondTab.value)?.focus()
}
function tabKey(event) {
  if (!['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) return
  event.preventDefault()
  switchTab(event.key==='Home'?'profile':event.key==='End'?'source':activeTab.value==='profile'?'source':'profile')
}
function toggle() {
  if (props.requested) opener.value?.focus()
  emit('toggle',!props.requested)
}
watch(()=>props.requested, want=>{if(want) show(); else clear()},{immediate:true})
onBeforeUnmount(clear)
</script>

<template>
  <div class="spec-inspector" data-address="pages/agent/sections/source">
    <button ref="opener" type="button" class="profile-open" :aria-expanded="requested" aria-controls="agent-spec-panel" @click="toggle">{{ requested ? 'Hide AgentSpec' : 'View AgentSpec' }}</button>
    <div v-if="requested" id="agent-spec-panel" role="region" aria-label="AgentSpec inspector">
      <p v-if="loading" role="status">Loading AgentSpec…</p>
      <p v-else-if="message" role="status">{{ message }} <button type="button" @click="show">Retry</button></p>
      <template v-else>
        <div class="spec-toolbar"><div class="spec-tabs" role="tablist" aria-label="AgentSpec views" @keydown="tabKey">
          <button ref="firstTab" id="profile-tab" role="tab" aria-controls="profile-panel" :tabindex="activeTab==='profile'?0:-1" :aria-selected="activeTab==='profile'" @click="switchTab('profile')">Profile</button>
          <button ref="secondTab" id="source-tab" role="tab" aria-controls="source-panel" :tabindex="activeTab==='source'?0:-1" :aria-selected="activeTab==='source'" @click="switchTab('source')">Source YAML</button>
        </div><span class="short-revision" v-if="revision" :title="revision">Revision <code>{{ revision.slice(0,12) }}</code></span></div>
        <div v-if="activeTab==='profile'" id="profile-panel" role="tabpanel" aria-labelledby="profile-tab" tabindex="0">
          <SpecProfile v-if="profile" :spec="profile" :routing="routing" :routing-state="routingState"/>
          <p v-else role="status">{{ profileError }}</p>
        </div>
        <div v-else id="source-panel" role="tabpanel" aria-labelledby="source-tab" tabindex="0">
          <p>Source revision <code>{{ revision || 'Unknown' }}</code></p>
          <pre tabindex="0" aria-label="AgentSpec YAML"><code v-text="source"></code></pre>
        </div>
      </template>
    </div>
  </div>
</template>
