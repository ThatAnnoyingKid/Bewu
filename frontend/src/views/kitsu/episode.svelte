<script>
  import Api from "../../Api.js";

  export let params = {};

  let animeId = params.id;

  let episodeData = Api.getKitsuEpisodes(animeId);
</script>

<div>
  {#await episodeData}
    <p>Loading...</p>
  {:then episodeData}
    <h1>{episodeData[0].title}</h1>
    <img src={episodeData[0].thumbnail_original} />
  {/await}
</div>
