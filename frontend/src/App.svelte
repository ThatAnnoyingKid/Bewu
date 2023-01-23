<script>
  import Index from "./views/Index.svelte";
  import Kitsu from "./views/kitsu/Index.svelte";
  import KitsuId from "./views/kitsu/{id}.svelte";
  import episode from "./views/kitsu/episode.svelte";

  import NavBar from "./components/NavBar.svelte";
  import Router from "svelte-spa-router";

  const routes = {
    "/": Index,
    "/kitsu": Kitsu,
    "/kitsu/:id": KitsuId,
	"/kitsu/:id/:episodeId": episode,
  };
</script>

<NavBar />
<Router {routes} />

<style>
</style>
