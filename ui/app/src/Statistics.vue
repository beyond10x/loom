<script setup>
import {number, duration, money} from './format.js'
defineProps({stats:Object, partial:Boolean})
defineEmits(['history'])
const groups = [['completed','Completed proposals'], ['failed','Failed or stopped'], ['rejected','Declined or unclassified']]
const metered = [['model_calls','Model calls'],['input_tokens','Input tokens'],['output_tokens','Output tokens'],['cost','Metered cost']]
</script>
<template>
  <section data-address="pages/overview/sections/stats" aria-labelledby="stats-title">
    <header class="section-head"><h2 id="stats-title">Run statistics</h2><span v-if="partial" class="badge warn">Partial data</span></header>
    <div class="stats">
      <div><span>Active now</span><strong>{{ number(stats.active) }}</strong></div>
      <div><span>Terminal turns</span><strong>{{ number(stats.total) }}</strong></div>
      <div><span>Completions / hour</span><strong>{{ number(stats.throughput_per_hour) }}</strong></div>
      <div><span>Median elapsed</span><strong>{{ duration(stats.p50_ms) }}</strong></div>
    </div>
    <div class="outcomes"><button v-for="[key,label] in groups" :key="key" @click="$emit('history',key)">{{ label }} <b>{{ number(stats[key]) }} / {{ number(stats.total) }}</b></button></div>
    <dl class="details">
      <dt>P95 elapsed</dt><dd>{{ duration(stats.p95_ms) }} · {{ number(stats.latency_samples) }} timed turns</dd>
      <dt>Refused steps</dt><dd><button class="link-button" @click="$emit('history','refused')">{{ number(stats.refused_steps) }}</button></dd>
      <dt>Budget stops</dt><dd><button class="link-button" @click="$emit('history','budget')">{{ number(stats.budget_stops) }}</button></dd>
      <template v-for="[key,label] in metered" :key="key"><dt>{{ label }}</dt><dd>{{ key === 'cost' ? money(stats.cost,stats.currency) : number(stats[key]) }}
        <small v-if="stats.coverage">Measured in {{ number(stats.coverage[key]) }} / {{ number(stats.coverage.total) }} turns</small></dd></template>
    </dl>
  </section>
</template>
