<script setup lang="ts">
defineProps<{
  label: string
}>()
</script>

<template>
  <span class="severity-badge">{{ label }}</span>
</template>
