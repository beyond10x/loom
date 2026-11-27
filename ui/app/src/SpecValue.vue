<script setup>
import {label, record} from './profile.js'
defineProps({value: null})
</script>

<template>
  <template v-if="Array.isArray(value)">
    <span v-if="!value.length" class="muted">None declared</span>
    <ul v-else-if="value.every(entry => entry === null || typeof entry !== 'object')" class="spec-tags">
      <li v-for="(entry,index) in value" :key="index">{{ entry === null ? 'null' : String(entry) }}</li>
    </ul>
    <ol v-else class="spec-list"><li v-for="(entry,index) in value" :key="index"><SpecValue :value="entry"/></li></ol>
  </template>
  <template v-else-if="record(value)">
    <dl v-if="Object.keys(value).length" class="spec-fields">
      <template v-for="(entry,key) in value" :key="key"><dt>{{ label(key) }}</dt><dd><SpecValue :value="entry"/></dd></template>
    </dl><span v-else class="muted">None declared</span>
  </template>
  <span v-else>{{ value === null ? 'null' : String(value) }}</span>
</template>
