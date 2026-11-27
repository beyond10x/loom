<script setup>
import {computed} from 'vue'
import SpecValue from './SpecValue.vue'
const props = defineProps({spec:Object})
const outer = computed(() => props.spec.loops?.outer)
const inner = computed(() => props.spec.loops?.inner)
const metrics = computed(() => [
  ['Poll interval',outer.value?.poll_interval_seconds,'s'],
  ['Minimum item age',outer.value?.min_age_minutes,'min'],
  ['Lookback',outer.value?.lookback_minutes,'min'],
  ['Step budget',inner.value?.step_budget,'steps'],
  ['Model budget',inner.value?.model_budget,'calls'],
  ['Classification threshold',inner.value?.classify_threshold,''],
].filter(([,v])=>v!=null))
</script>

<template>
  <section v-if="spec.loops || spec.states?.length" class="profile-section" aria-labelledby="flow-heading">
    <h3 id="flow-heading">Turn flow</h3>
    <div v-if="spec.loops" class="flow-grid">
      <div v-if="outer" class="flow-node">
        <h4>Outer loop</h4>
        <p v-if="spec.channel?.binding">Channel binding <strong>{{ spec.channel.binding }}</strong></p>
        <p v-else class="muted">No channel poller declared</p>
        <div v-if="outer.sources?.length"><small>Source bindings</small><SpecValue :value="outer.sources"/></div>
      </div>
      <div v-if="inner" class="flow-node" :class="{'linked-node':outer && spec.channel}">
        <h4>Inner loop</h4><p>Turn protocol</p><strong class="protocol-name">{{ inner.protocol }}</strong>
      </div>
    </div>
    <dl v-if="metrics.length" class="budget-grid"><div v-for="[title,value,unit] in metrics" :key="title">
      <dt>{{ title }}</dt><dd>{{ String(value) }} <small v-if="unit">{{ unit }}</small></dd>
    </div></dl>
    <template v-if="spec.states?.length">
      <h4 class="subheading">Action states</h4>
      <div class="state-list"><article v-for="(state,index) in spec.states" :key="index" class="state">
        <div class="state-condition"><span class="field-label">When intent is</span><strong>{{ state.when?.intent }}</strong></div>
        <div class="state-body"><h5>{{ state.name }}</h5>
          <div v-if="state.actions"><span class="field-label">Allowed actions</span><SpecValue :value="state.actions"/></div>
          <div v-if="state.sources"><span class="field-label">Source bindings</span><SpecValue :value="state.sources"/></div>
        </div>
      </article></div>
    </template>
  </section>
</template>
