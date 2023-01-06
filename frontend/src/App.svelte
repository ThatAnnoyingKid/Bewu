<script>
  import Index from "./views/Index.svelte";
  import Kitsu from "./views/kitsu/Index.svelte";
  import KitsuId from "./views/kitsu/{id}.svelte";

  import NavBar from "./components/NavBar.svelte";
  import Router from "svelte-spa-router";
  import Api from "./Api.js";

  const routes = {
    "/": Index,
    "/kitsu": Kitsu,
    "/kitsu/:id": KitsuId,
  };
</script>

<NavBar />
<Router {routes} />

<style>
</style>
