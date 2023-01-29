<script>
  import Api from "@/Api.js";

  export let params = {};

  let episodeId = params.id;

  let episodeData = Api.getKitsuEpisode(episodeId);

    /*
  function myFunction() {
    alert("YOU'RE DONE BUCKO!!!");
  }
  */
</script>

<div class="container">
  {#await episodeData}
    <p>Loading...</p>
  {:then episodeData}
    <h1>{episodeData.title}</h1>
    <video
      controls
      src="https://www.w3schools.com/tags/movie.mp4"
      poster="{episodeData.thumbnail_original}"
      width="1920"
      height="1080"
    />
  {/await}
  <!--
  <a href="https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    ><button on:click={myFunction}>Download</button></a
  >-->
</div>

<style>
    .container {
        align-items: center;
        display: flex;
        flex-direction: column;
        margin: 1em;
    }
    
    video {
        width: auto;
        height: 50vh;
    }
</style>
