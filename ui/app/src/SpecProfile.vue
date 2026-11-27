<script setup>
import {computed,ref} from 'vue'
import SpecValue from './SpecValue.vue'
import SpecFlow from './SpecFlow.vue'
import SpecRouting from './SpecRouting.vue'
import {label} from './profile.js'
const props=defineProps({spec:Object,routing:Object,routingState:String})
const showFields=ref(false)
const auxiliary = [['channel','Channel'],['writes','Writes'],['policy','Policy'],['session','Session'],['runtime','Runtime'],['deploy','Deployment']]
const generic = computed(() => {
  const owned = new Set(['format','agent','model','loops','states','bindings','expose',...auxiliary.map(([key])=>key)])
  return [...auxiliary, ...Object.keys(props.spec).filter(key=>!owned.has(key)).map(key=>[key,label(key)])].filter(([key])=>props.spec[key]!=null)
})
const a2a=computed(()=>props.spec.expose?.a2a)
const bindingFields = binding => Object.fromEntries(Object.entries(binding).filter(([key]) => key !== 'name' && key !== 'kind'))
</script>

<template>
  <article aria-label="Agent profile" class="agent-profile">
    <header class="profile-banner">
      <div class="profile-id"><span>{{ spec.format }}</span><code>{{ spec.agent.id }}</code></div>
      <h2>{{ spec.agent.name || spec.agent.id }}</h2>
      <p v-if="spec.agent.brief" class="profile-brief">{{ spec.agent.brief }}</p>
    </header>
    <SpecRouting v-if="spec.model || routing" :model="spec.model" :routing="routing" :routing-state="routingState"/>
    <SpecFlow :spec="spec"/>
    <section v-if="spec.bindings?.length" aria-labelledby="bindings-heading" class="profile-section">
      <h3 id="bindings-heading">Bindings</h3>
      <div class="binding-grid"><article v-for="(binding,index) in spec.bindings" :key="index" class="binding">
        <header><h4>{{ binding.name }}</h4><span class="badge">{{ binding.kind }}</span></header>
        <SpecValue :value="bindingFields(binding)"/>
      </article></div>
    </section>
    <section v-if="a2a" aria-labelledby="a2a-heading" class="profile-section">
      <h3 id="a2a-heading">A2A exposure</h3>
      <article class="a2a-card"><h4 v-if="a2a.card?.name">{{ a2a.card.name }}</h4><p v-if="a2a.card?.description">{{ a2a.card.description }}</p>
        <dl class="spec-fields"><dt>Listener</dt><dd>Port <code>{{ a2a.port }}</code> · <code>{{ a2a.path }}</code></dd>
        <template v-if="a2a.card?.skills"><dt>Skills</dt><dd><SpecValue :value="a2a.card.skills"/></dd></template></dl>
      </article>
    </section>
    <div class="profile-settings"><section v-for="[key,title] in generic" :key="key" class="profile-section"><h3>{{ title }}</h3><SpecValue :value="spec[key]"/></section></div>
    <details class="field-index" @toggle="showFields=$event.currentTarget.open"><summary>All specification fields</summary>
      <SpecValue v-if="showFields" :value="spec"/>
    </details>
  </article>
</template>
