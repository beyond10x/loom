<script setup>
import {duration, money, number, time} from './format.js'
defineProps({run:Object})
</script>
<template>
  <section aria-labelledby="detail-title" data-address="pages/run/sections/detail">
    <header class="section-head"><h2 id="detail-title">Recorded activity</h2><span v-if="run" class="badge">{{ run.outcome }}</span></header>
    <p v-if="!run" class="empty">This run is outside the visible history.</p>
    <template v-else>
      <h3>{{ run.item_id }}</h3><code>{{ run.id }}</code>
      <dl class="details"><dt>Agent</dt><dd>{{ run.agent_id }}</dd><dt>Started</dt><dd>{{ time(run.started_at) }}</dd><dt>Ended</dt><dd>{{ time(run.ended_at) }}</dd>
      <dt>Elapsed</dt><dd>{{ duration(run.started_at && run.ended_at ? Date.parse(run.ended_at)-Date.parse(run.started_at) : null) }}</dd>
      <dt>Stop reason</dt><dd>{{ run.stop_reason || 'None recorded' }}</dd>
      <dt>Metered cost</dt><dd>{{ money(run.cost, 'USD') }}</dd><dt>Model calls</dt><dd>{{ number(run.model_calls) }}</dd><dt>Tokens in / out</dt><dd>{{ number(run.input_tokens) }} / {{ number(run.output_tokens) }}</dd></dl>
      <h3>Events</h3>
      <ol v-if="run.events?.length" class="steps"><li v-for="(event,i) in run.events" :key="i"><strong>{{ event.kind }}</strong><span>{{ event.action || event.outcome }}</span><small>{{ time(event.time) }}</small></li></ol>
      <p v-else class="empty">No events recorded.</p>
      <h3>Recorded steps</h3>
      <ol v-if="run.steps?.length" class="steps"><li v-for="(step,i) in run.steps" :key="i"><strong>{{ step.action }}</strong><span class="badge">{{ step.outcome }}</span>
      <small>Steps left {{ number(step.steps_left) }} · Model calls left {{ number(step.model_calls_left) }}</small></li></ol>
      <p v-else class="empty">No steps recorded.</p>
    </template>
  </section>
</template>
