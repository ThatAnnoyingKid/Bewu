<script>
  import Api from "../../Api.js";

  export let params = {};

  let animeId = params.id;

  let animeData = Api.getKitsuAnime(animeId);
</script>

<div class="container">
  {#await animeData}
    Loading...
  {:then anime}
    <div class="info-row">
      <img
        src={anime.poster_large}
        alt="cover image for {anime.title}"
        width="550"
        height="780"
      />
      <div class="anime-info">
        <h1>{anime.title}</h1>
        <div>
          {anime.synopsis}
        </div>
        {#if anime.rating !== null}
          <div>Rating: {anime.rating}/100</div>
        {/if}
      </div>
    </div>
  {:catch error}
    {error.message}
  {/await}
  <!--{#await}{/await}-->
</div>

<style>
  .container {
    padding: 0.5em;
  }

  .info-row {
    align-items: start;
    display: flex;
    flex-direction: row;
  }

  img {
    width: 10em;
    height: auto;
  }

  .anime-info {
    padding-left: 0.5em;
  }

  h1 {
    margin: 0;
  }
</style>
