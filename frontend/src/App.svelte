<script>
  import Index from "./views/Index.svelte";
  import Kitsu from "./views/kitsu/Index.svelte";
  import KitsuAnimeId from "./views/kitsu/anime/{id}.svelte";
  import KitsuEpisodeId from "./views/kitsu/episodes/{id}.svelte";

  import NavBar from "./components/NavBar.svelte";
  import Router from "svelte-spa-router";

  const routes = {
    "/": Index,
    "/kitsu": Kitsu,
    "/kitsu/anime/:id": KitsuAnimeId,
    "/kitsu/episodes/:id": KitsuEpisodeId,
  };
</script>

<NavBar />
<Router {routes} />

<style>
</style>
