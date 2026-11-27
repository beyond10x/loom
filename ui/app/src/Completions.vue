<script setup>
import {computed} from 'vue'
const props=defineProps({runs:Array})
defineEmits(['select'])
const days = computed(() => {
  const counts=new Map()
  for (const run of props.runs) {
    if (!['proposed','proposed_case'].includes(run.outcome) || !run.ended_at) continue
    const day = new Date(run.ended_at).toISOString().slice(0,10)
    counts.set(day,(counts.get(day)||0)+1)
  }
  return [...counts].sort(([a],[b]) => a.localeCompare(b)).reverse()
})
const max = computed(()=>Math.max(1,...days.value.map(([,n])=>n)))
</script>
<template>
  <details class="chart"><summary>Completions by day (UTC)</summary>
    <p v-if="!days.length" class="empty">No timestamped completions in this interval.</p>
    <ul v-else><li v-for="[day,count] in days" :key="day">
      <button class="link-button" @click="$emit('select',day)">{{ day }}</button>
      <meter :max="max" min="0" :value="count" :aria-label="day + ' completions'"></meter><span>{{ count }}</span>
    </li></ul>
  </details>
</template>
