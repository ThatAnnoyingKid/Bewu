<script>
  import NavBar from "./components/NavBar.svelte";
  import Router from "svelte-spa-router";

  const routes = {};
</script>

<NavBar />
<Router {routes} />

<style>
</style>
