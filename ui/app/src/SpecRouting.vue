<script setup>
import {computed} from 'vue'
import {label} from './profile.js'
const props = defineProps({model:Object, routing:Object, routingState:String})
const roles = computed(() => {
  const declared = new Map((props.model?.roles || []).map(row => [row.role, row.model]))
  const resolved = ['agent','classifier'].filter(role => props.routing?.[role])
  return [...new Set([...declared.keys(), ...resolved])].map(role => ({role, configured:declared.get(role), active:props.routing?.[role]}))
})
const fields = [['upstream_model','Upstream model'],['provider_id','Provider'],['endpoint_id','Endpoint'],['base_origin','Origin'],['protocol','Protocol'],['binding_revision','Binding revision']]
</script>

<template>
  <section aria-labelledby="models-heading" class="profile-section">
    <header class="profile-section-head"><h3 id="models-heading">Models and routing</h3><span v-if="model?.routes" class="spec-route">Routes file <code>{{ model.routes }}</code></span></header>
    <div class="role-grid">
      <article v-for="entry in roles" :key="entry.role" class="role-card">
        <div class="role-title">{{ label(entry.role) }}</div>
        <strong v-if="entry.active?.selection || entry.configured" class="model-name">{{ entry.active?.selection || entry.configured }}</strong>
        <dl class="spec-fields">
          <template v-if="entry.configured"><dt>Declared model</dt><dd><code>{{ entry.configured }}</code></dd></template>
          <template v-for="[key,title] in fields" :key="key"><template v-if="entry.active?.[key] != null"><dt>{{ title }}</dt><dd>{{ entry.active[key] }}</dd></template></template>
        </dl>
      </article>
    </div>
    <p v-if="routing" class="spec-caption">Route selection · {{ routing.source === 'launch_catalog' ? 'Launch catalog' : routing.source === 'spec_routes' ? 'AgentSpec routes file' : routing.source }}</p>
    <p v-else class="spec-caption" role="status">{{ routingState === 'loading' ? 'Reading launch routing…' : 'Launch routing unavailable.' }}</p>
  </section>
</template>
