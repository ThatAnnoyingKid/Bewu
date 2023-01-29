<script>
  import Api from "../../Api.js";

  export let params = {};

  let animeId = params.id;
  let animeTitle = null;
  let animePosterLarge = null;

  let animeData = Api.getKitsuAnime(animeId).then((anime) => {
    animeTitle = anime.title;
    animePosterLarge = anime.poster_large;
    return anime;
  });
  let episodeData = Api.getKitsuEpisodes(animeId);
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
  <div class="episodes-container">
    <h2>Episodes</h2>
    {#await episodeData}
      Loading...
    {:then episodes}
      <ol>
        {#each episodes as episode}
          <li>
            <a href="https://www.youtube.com/watch?v=dQw4w9WgXcQ">
              <!-- make it go to episode page thingy -->
              <!-- 
              Thumbnail data is not returned from the api.
              While most are 400 x 300, some are not.
              However, 400 x 300 is a good guess while loading, 
              and we can fix it up with css if we are wrong.
            -->
              <img
                src={episode.thumbnail_original || animePosterLarge}
                alt="thumbnail for {episode.title || animeTitle}"
                width="400"
                height="300"
              />
              <div class="episode-info-container">
                <h3>{episode.title || animeTitle}</h3>
              </div>
            </a>
          </li>
        {/each}
      </ol>
    {:catch error}
      {error.message}
    {/await}
  </div>
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
    text-align: center;
  }

  .anime-info {
    padding-left: 0.5em;
  }

  h1 {
    margin: 0;
    font-weight: 100;
  }

  .episodes-container {
    padding-top: 3em;
  }

  h2 {
    margin: 0;
    font-weight: 100;
  }

  .episodes-container ol {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
  }

  .episodes-container ol li {
    display: flex;
    list-style-type: none;
    padding: 0.5em;
  }

  .episode-info-container {
    align-items: center;
    display: flex;
    flex-grow: 1;
    justify-content: center;
    padding-left: 0.5em;
  }

  h3 {
    font-weight: 100;
    margin: 0;
    padding: 0;
  }
</style>
