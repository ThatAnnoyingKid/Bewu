<script>
  import NavBar from "./components/NavBar.svelte";
  import Router from "svelte-spa-router";
  import Api from "./Api.js";

  let inputValue = "";
  function handleClick() {
    Api.searchKitsu(inputValue);
  }

  const routes = {};
</script>

<NavBar />
<Router {routes} />

<label for="kitsu-search">Search Kitsu</label><input
  type="text"
  name="kitsu-search"
  bind:value={inputValue}
/><button on:click={handleClick}>Search</button>

<style>
</style>
