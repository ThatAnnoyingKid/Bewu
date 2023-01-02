<script>
  import Index from "./views/Index.svelte";

  import NavBar from "./components/NavBar.svelte";
  import Router from "svelte-spa-router";
  import Api from "./Api.js";

  const routes = {
    "/": Index,
  };
</script>

<NavBar />
<Router {routes} />

<style>
</style>
