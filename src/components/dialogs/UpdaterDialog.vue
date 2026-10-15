<template>
  <UpdateNotice :open="dialogOpen" :version="availableUpdate?.version ?? null" :status="status" :busy="isInstalling"
    :downloaded-bytes="downloadedBytes" :total-bytes="totalBytes" :download-percent="downloadPercent" :error-message="errorMessage"
    @dismiss="dismissUpdate" @install="installUpdate" @restart="restartToUpdate" />
</template>
<script setup lang="ts">
import UpdateNotice from '@/components/UpdateNotice.vue'
import { useUpdater } from '@/composables/useUpdater'
const { availableUpdate, dialogOpen, status, isInstalling, downloadedBytes, totalBytes, downloadPercent, errorMessage, dismissUpdate, installUpdate, restartToUpdate } = useUpdater()
</script>
