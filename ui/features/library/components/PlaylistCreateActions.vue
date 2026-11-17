<script setup lang="ts">
import type {PageCallbacks} from '../../../app/root-controls.ts';
defineProps<{callbacks:PageCallbacks}>();
</script>
<template>
            <div class="playlist-create-actions">
              <button id="import-external-playlist-button" class="secondary-button" type="button" @click="callbacks.importExternalPlaylist()">导入外部歌单</button>
              <button id="import-playlist-button" class="secondary-button" type="button" @click="callbacks.importPlaylist()">导入 B站收藏夹</button>
              <button id="create-playlist-button" class="secondary-button" type="button" @click="callbacks.createPlaylist()">新建歌单</button>
            </div>

</template>
