<template>
  <div id="app">
    <HelloWorld />
  </div>
</template>

<script>
import HelloWorld from './components/HelloWorld.vue'
import axios from 'axios';

const test = window.location.hostname;
axios.defaults.baseURL = 'http://' + test + ':3030';

export default {
  name: 'App',
  components: {
    HelloWorld
  }
}
</script>

<style>
</style>
