<script setup>
import {computed, onBeforeUnmount, ref, watch} from 'vue'
import {agentKind, harnessName} from './format.js'

const props=defineProps({agents:Array,initialAgent:String,request:Function,href:Function})
const emit=defineEmits(['navigate'])
const mode=ref('send'), target=ref(''), task=ref(''), bearer=ref('')
const templates=ref([]), catalogState=ref('idle'), templateId=ref(''), harness=ref('')
const sending=ref(false), problem=ref(''), catalogProblem=ref(''), receipt=ref(null)
const eligible=computed(()=>props.agents.filter(a=>a.status!=='offline' && a.a2a_protocol))
const agent=computed(()=>eligible.value.find(a=>a.id===target.value))
const harnesses=computed(()=>[...new Set(templates.value.flatMap(t=>t.harnesses))])
const compatible=computed(()=>templates.value.filter(t=>t.harnesses.includes(harness.value)))
const template=computed(()=>templates.value.find(t=>t.id===templateId.value))
const bytes=computed(()=>new TextEncoder().encode(task.value).length)
const alive=new AbortController()
let attemptId
const errors={
  unauthorized:'Sign in again with your operator token.',
  forbidden:'This action requires a server operator token.',
  agent_unauthorized:'The agent did not accept the A2A bearer.',
  not_found:'The selected agent is unavailable.',
  agent_unavailable:'This agent is offline or has no A2A listener.',
  dispatch_unavailable:'The agent may still be handling this task. Retrying uses the same message ID.',
  invalid_request:'The server refused the request. Check the selected options and input.',
  template_not_found:'The selected template is no longer available.',
  request_conflict:'This request ID belongs to another launch.',
  state_conflict:'An incompatible host is already using this template’s state directory.',
  template_configuration_unavailable:'The server could not read its launch templates.',
  launch_unavailable:'The launch has no confirmed result. Retrying uses the same request ID.',
}
async function checked(response) {
  const body=await response.json().catch(()=>null)
  if (!response.ok) throw Error(errors[body?.error] || (response.status===403 ? errors.forbidden : 'The server could not complete this request.'))
  if (!body) throw Error('The server returned an unreadable receipt.')
  return body
}
function resetAttempt() {
  attemptId=undefined
  receipt.value=null
  problem.value=''
}
watch(eligible, choices=>{
  if (sending.value || receipt.value) return
  if (!choices.some(a=>a.id===target.value)) {
    const preferred=choices.find(a=>a.id===props.initialAgent)
    target.value=preferred?.id || (choices.length===1 ? choices[0].id : '')
  }
},{immediate:true})
watch(target,()=>{bearer.value=''})
watch(compatible, options=>{
  if (!options.some(t=>t.id===templateId.value)) templateId.value=options.length===1 ? options[0].id : ''
})
watch([mode,target,task,templateId,harness],resetAttempt)
watch(mode, value=>{
  bearer.value=''
  if(value==='start' && catalogState.value==='idle') loadTemplates()
})
async function loadTemplates() {
  catalogState.value='loading'
  catalogProblem.value=''
  try {
    const data=await checked(await props.request('/api/launch-templates',{signal:alive.signal}))
    if (alive.signal.aborted) return
    templates.value=data.templates
    harness.value=harnesses.value.length===1 ? harnesses.value[0] : ''
    catalogState.value='ready'
  } catch(error) {
    if (!alive.signal.aborted) {
      catalogState.value='failed'
      catalogProblem.value=error.message || 'Could not read launch templates.'
    }
  }
}
function newId() {
  const random=crypto.getRandomValues(new Uint8Array(16))
  return [...random].map(byte=>byte.toString(16).padStart(2,'0')).join('')
}
const canSubmit=computed(()=>!sending.value && !receipt.value && (mode.value==='send'
  ? Boolean(agent.value && task.value.trim() && bearer.value.trim() && bytes.value<=16384)
  : Boolean(template.value && template.value.harnesses.includes(harness.value))))
async function submit() {
  if(!canSubmit.value) return
  const send=mode.value==='send'
  const requestedAgent=target.value
  attemptId ||= newId()
  const payload=send ? {message_id:attemptId,text:task.value} : {request_id:attemptId,template:templateId.value,harness:harness.value}
  const path=send ? '/api/agents/'+encodeURIComponent(target.value)+'/dispatch' : '/api/launch'
  const headers={'Content-Type':'application/json'}
  if(send) headers['X-Loom-Agent-Bearer']=bearer.value
  sending.value=true
  problem.value=''
  try {
    const result=await checked(await props.request(path,{method:'POST',headers,body:JSON.stringify(payload),signal:alive.signal}))
    if (!alive.signal.aborted) {
      receipt.value=send ? {...result, agent_id:requestedAgent} : result
      bearer.value=''
    }
  } catch(error) {
    if(!alive.signal.aborted) problem.value=error instanceof TypeError
      ? 'Connection lost. Retrying uses the same request ID.' : error.message
  } finally {sending.value=false}
}
const destination=computed(()=>receipt.value && {view:'',agent:receipt.value.agent_id || target.value,run:receipt.value.run_id || '',tab:'',from:'',to:''})
onBeforeUnmount(()=>{alive.abort();bearer.value='';task.value=''})
</script>

<template>
  <section class="dispatch-panel" data-address="pages/dispatch/sections/actions" aria-label="Dispatch controls">
    <fieldset class="dispatch-modes" :disabled="sending"><legend>Action</legend>
      <label><input type="radio" v-model="mode" value="send" name="dispatch-mode">Send task</label>
      <label><input type="radio" v-model="mode" value="start" name="dispatch-mode">Start agent</label>
    </fieldset>
    <form @submit.prevent="submit"><fieldset :disabled="sending">
      <template v-if="mode==='send'">
        <h2>Send task</h2>
        <label for="task-agent">Running agent</label>
        <select id="task-agent" v-model="target" required><option value="" disabled>Select an agent</option>
          <option v-for="a in eligible" :key="a.id" :value="a.id">{{ a.name }} · {{ agentKind(a.kind) }} · {{ harnessName(a.harness) }}</option>
        </select>
        <p v-if="!eligible.length" class="notice">No running A2A agents are visible.</p>
        <p v-else-if="agent" class="field-hint">Instance {{ agent.instance }} · {{ agent.status }} · {{ agent.a2a_protocol }}</p>
        <label for="task-text">Task</label><textarea id="task-text" v-model="task" rows="5" required></textarea>
        <p v-if="bytes>16384" role="alert">Task exceeds 16,384 UTF-8 bytes.</p>
        <label for="task-bearer">Agent A2A bearer</label>
        <input id="task-bearer" type="password" v-model="bearer" required autocomplete="off" spellcheck="false">
      </template>
      <template v-else>
        <h2>Start agent</h2>
        <p v-if="catalogState==='loading'" role="status">Loading launch templates…</p>
        <p v-else-if="catalogState==='failed'" role="alert">{{ catalogProblem }} <button type="button" @click="loadTemplates">Retry</button></p>
        <p v-else-if="!templates.length" class="notice">No launch templates are configured on this server.</p>
        <template v-else>
          <label for="launch-harness">Harness</label>
          <select id="launch-harness" v-model="harness" required><option value="" disabled>Select a harness</option><option v-for="h in harnesses" :key="h" :value="h">{{ harnessName(h) }}</option></select>
          <template v-if="harness">
            <label for="launch-template">Agent template</label>
            <select id="launch-template" required v-model="templateId"><option value="" disabled>Select a template</option><option v-for="t in compatible" :key="t.id" :value="t.id">{{ t.name }} · {{ agentKind(t.kind) }}</option></select>
            <p v-if="template" class="field-hint">Spec {{ template.spec_id }}<template v-if="template.protocol"> · {{ template.protocol }}</template></p>
          </template>
        </template>
      </template>
      <button type="submit" :disabled="!canSubmit">{{ sending ? 'Submitting…' : mode==='send' ? 'Send task' : 'Start agent' }}</button>
    </fieldset></form>
    <p v-if="problem" role="alert" class="notice">{{ problem }}</p>
    <div v-if="receipt" role="status" class="dispatch-receipt">
      <strong>{{ mode==='send' ? receipt.state : receipt.status==='already_running' ? 'Already running' : 'Agent started' }}</strong>
      <p v-if="receipt.instance">{{ agentKind(receipt.kind) }} · {{ harnessName(receipt.harness) }} · Instance {{ receipt.instance }}</p>
      <a :href="href(destination)" @click.prevent="emit('navigate',destination)">{{ mode==='send' ? 'View run' : 'View agent' }}</a>
    </div>
  </section>
</template>
