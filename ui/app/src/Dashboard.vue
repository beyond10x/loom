<script setup>
import { computed, nextTick, ref, watch } from 'vue'
import AgentSpec from './AgentSpec.vue'
import RunDetail from './RunDetail.vue'
import Completions from './Completions.vue'
import Statistics from './Statistics.vue'
import {useSnapshot} from './snapshot.js'
import {duration, itemLabel, number, time} from './format.js'

const {snapshot, filters, state, busy, refresh, readAgentSpec, connect, disconnect, href, navigate} = useSnapshot()
const credential = ref(''), from = ref(''), to = ref(''), intervalError = ref('')
const selected = computed(() => snapshot.value?.agents.find(a => a.id === filters.value.agent))
const selectedRun = computed(() => snapshot.value && [...snapshot.value.runs, ...snapshot.value.agents.flatMap(a => a.current)].find(r => r.id === filters.value.run))
const historyFilter = ref('all'), historyTable = ref()
const outcomes = {completed:['proposed','proposed_case'], failed:['failed','stopped'], rejected:['declined','unclassified']}
const historyRows = computed(() => (snapshot.value?.runs || []).filter(run => {
  if (historyFilter.value === 'all') return true
  if (historyFilter.value === 'budget') return run.stop_reason === 'budget exhausted'
  if (historyFilter.value === 'refused') return run.steps.some(step => step.outcome === 'refused')
  if (historyFilter.value.startsWith('day:')) return outcomes.completed.includes(run.outcome) && run.ended_at && new Date(run.ended_at).toISOString().startsWith(historyFilter.value.slice(4))
  return outcomes[historyFilter.value]?.includes(run.outcome)
}))
async function drillDown(filter) {
  historyFilter.value = filter
  await nextTick()
  historyTable.value?.focus()
}
function dateInput(value) {
  const date = Date.parse(value)
  return Number.isFinite(date) ? new Date(date).toISOString().slice(0,19) : ''
}
watch(filters, query => { historyFilter.value = 'all'; from.value = dateInput(query.from); to.value = dateInput(query.to); intervalError.value = '' }, {immediate:true})
function applyInterval() {
  if (from.value && to.value && from.value >= to.value) {
    intervalError.value = 'End must be after start.'
    return
  }
  navigate({from: from.value && new Date(from.value + 'Z').toISOString(), to: to.value && new Date(to.value + 'Z').toISOString(), run:''})
}
function login() { if (credential.value.trim()) { connect(credential.value.trim()); credential.value = '' } }
</script>
<template>
  <a class="skip" href="#main">Skip to content</a>
  <header class="mast"><div class="brand"><span class="mark" aria-hidden="true">≋</span> Loom</div>
    <span class="mast-label">Agent dashboard</span><a class="api-link" href="/api/openapi.json">API design</a>
    <button v-if="snapshot" class="mast-end" @click="disconnect">Sign out</button>
  </header>
  <main id="main">
    <nav aria-label="Breadcrumb" v-if="snapshot"><a :href="href({agent:'',run:''})" @click.prevent="navigate({agent:'',run:''})">Server overview</a>
      <template v-if="filters.agent"><span aria-hidden="true"> / </span><a :href="href({run:''})" @click.prevent="navigate({run:''})">{{ selected?.name || filters.agent }}</a></template>
      <template v-if="filters.run"><span aria-hidden="true"> / </span> Run detail</template></nav>
    <div class="headline"><div><h1>{{ filters.run ? 'Run detail' : filters.agent ? (selected?.name || 'Agent detail') : 'Server overview' }}</h1>
      <p v-if="snapshot" class="muted">{{ snapshot.server }} · Observed {{ time(snapshot.time) }}</p></div>
      <button v-if="snapshot" :disabled="busy" @click="refresh">Refresh</button></div>
    <form v-if="['login','denied'].includes(state)" class="access" @submit.prevent="login">
      <h2>{{ state === 'denied' ? 'Access denied' : 'Viewer access' }}</h2>
      <p>Use the viewer token file named at agent startup, or your scoped viewer token. For remote access, open the dashboard through the server’s tunnel or authenticated ingress.</p><label for="token">Viewer token</label>
      <input id="token" type="password" v-model="credential" required autocomplete="off" spellcheck="false"><button>Connect</button></form>
    <div v-if="['invalid','missing'].includes(state)" class="notice" role="status">{{ state === 'invalid' ? 'Invalid reporting interval.' : 'This agent is unavailable to this viewer.' }} <a :href="href({from:'',to:'',agent:'',run:''})" @click.prevent="navigate({from:'',to:'',agent:'',run:''})">Return to server overview</a></div>
    <p v-if="state === 'loading'" role="status">Loading agent activity…</p>
    <p v-if="state === 'stale'" class="notice" role="status">Connection lost. {{ snapshot ? 'Showing the last observation.' : 'Waiting for the server.' }} <button @click="refresh">Retry</button></p>
    <template v-if="snapshot">
      <template v-if="filters.run"><RunDetail :run="selectedRun" /></template>
      <template v-else>
        <section aria-labelledby="fleet-title" data-address="pages/overview/sections/agents">
          <header class="section-head"><h2 id="fleet-title">{{ filters.agent ? 'Agent' : 'Agent fleet' }}</h2><span class="count">{{ snapshot.agents.length }}</span></header>
          <div v-if="!snapshot.agents.length" class="empty">No visible agents.</div>
          <div v-else class="fleet">
          <article v-for="agent in snapshot.agents" :key="agent.id" :data-address="'pages/overview/sections/agents/rows/'+agent.id">
            <div class="section-head"><a class="agent-name" :data-address="'pages/overview/sections/agents/rows/'+agent.id+'/row_actions/open'" :href="href({agent:agent.id,run:''})" @click.prevent="navigate({agent:agent.id,run:''})">{{ agent.name }}</a>
              <span class="badge" :class="{working:agent.status === 'working',warn:agent.status === 'offline'}">{{ agent.status }}</span></div>
            <dl class="details" :data-address="filters.agent ? 'pages/agent/sections/detail' : undefined"><dt v-if="filters.agent">Agent</dt><dd v-if="filters.agent">{{ agent.name }}</dd><dt>Instance</dt><dd>{{ agent.instance }}</dd><dt>Last seen</dt><dd>{{ time(agent.last_seen) }}</dd>
              <template v-if="filters.agent">
                <dt>Specification</dt><dd>{{ agent.spec_id }}</dd>
                <dt>Spec revision</dt><dd><code>{{ agent.spec_revision || 'Unknown' }}</code></dd>
                <dt>Protocol</dt><dd>{{ agent.protocol || 'Unknown' }}</dd>
                <dt>A2A protocol</dt><dd>{{ agent.a2a_protocol || 'Unknown' }}</dd>
                <dt>Poll interval</dt><dd>{{ agent.poll_interval_seconds == null ? 'Unknown' : number(agent.poll_interval_seconds) + ' s' }}</dd>
                <dt>Model</dt><dd>{{ agent.model || 'Unknown' }}</dd>
              </template></dl>
            <div v-if="agent.current.length"><h3>Current work</h3><div v-for="run in agent.current" :key="run.id" class="current">
              <a :title="run.item_id" :href="href({run:run.id})" @click.prevent="navigate({run:run.id})">{{ itemLabel(run.item_id) }}</a><p>{{ run.action }}</p>
              <small>Started {{ time(run.started_at) }} · Elapsed {{ duration(Date.parse(snapshot.time) - Date.parse(run.started_at)) }}</small>
              <small>Steps left {{ number(run.steps_left) }} · Model calls left {{ number(run.model_calls_left) }}</small></div></div>
            <AgentSpec v-if="filters.agent && state === 'live'" :key="[agent.id,agent.instance,agent.spec_revision].join(':')" :agent="agent" :read="readAgentSpec"/>
          </article></div></section>
        <form class="interval" @submit.prevent="applyInterval"><h2>Reporting interval</h2><label>From (UTC)<input type="datetime-local" step="1" v-model="from"></label><label>To (UTC, exclusive)<input type="datetime-local" step="1" v-model="to"></label>
          <button>Apply</button><button v-if="filters.from || filters.to" type="button" @click="navigate({from:'',to:''})">All time</button><p v-if="intervalError" role="alert">{{ intervalError }}</p></form>
        <Statistics :stats="snapshot.stats" :partial="snapshot.partial" @history="drillDown"/>
        <Completions :runs="snapshot.runs" @select="day => drillDown('day:'+day)"/>
        <section data-address="pages/overview/sections/history"><header class="section-head"><h2 ref="historyTable" tabindex="-1">Run history</h2><span class="count">{{ historyRows.length }}</span></header>
          <label v-if="snapshot.runs.length" class="history-filter">Show turns<select v-model="historyFilter">
            <option value="all">All outcomes</option><option value="completed">Completed proposals</option><option value="failed">Failed or stopped</option><option value="rejected">Declined or unclassified</option><option value="refused">With refused steps</option><option value="budget">Budget exhausted</option>
            <option v-if="historyFilter.startsWith('day:')" :value="historyFilter">{{ historyFilter.slice(4) }} completions</option>
          </select></label>
          <p v-if="!historyRows.length" class="empty">{{ historyFilter === 'all' ? 'No runs in this interval.' : 'No matching turns.' }}</p>
          <div v-else class="table-wrap"><table><thead><tr><th>Item</th><th>Agent</th><th>Outcome</th><th>Ended (UTC)</th></tr></thead>
            <tbody><tr v-for="run in historyRows" :key="run.id" :data-address="'pages/overview/sections/history/rows/'+run.id">
              <td><a :data-address="'pages/overview/sections/history/rows/'+run.id+'/row_actions/open'" :title="run.item_id" :href="href({run:run.id})" @click.prevent="navigate({run:run.id})">{{ itemLabel(run.item_id) }}</a></td><td>{{ snapshot.agents.find(a=>a.id === run.agent_id)?.name || run.agent_id }}</td><td><span class="badge">{{ run.outcome }}</span></td><td>{{ time(run.ended_at) }}</td>
            </tr></tbody></table></div></section>
      </template>
    </template>
  </main>
</template>
