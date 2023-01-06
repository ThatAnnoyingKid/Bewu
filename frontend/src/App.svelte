<script>
  import Index from "./views/Index.svelte";
  import KitsuIndex from "./views/kitsu/Index.svelte";

  import NavBar from "./components/NavBar.svelte";
  import Router from "svelte-spa-router";
  import Api from "./Api.js";

  const routes = {
    "/": Index,
    "/kitsu": KitsuIndex,
  };
</script>

<NavBar />
<Router {routes} />

<style>
</style>
