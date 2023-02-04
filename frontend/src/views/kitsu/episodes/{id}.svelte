<script>
  import Api from "@/Api.js";
  import { afterUpdate } from "svelte";

  export let params = {};

  let episodeId = params.id;

  let videoUp = false;

  async function performVidstreamingDownload() {
    for await (const event of Api.downloadVidstreamingEpisode(episodeId)) {
      console.log(event);
    }
  }

  let kitsuEpisodeData = Api.getKitsuEpisode(episodeId);
  let vidstreamingEpisodeDownload = performVidstreamingDownload();

  let episodeData = Promise.all([kitsuEpisodeData]).then((result) => {
    requestAnimationFrame(() => {
      if (!videoUp) {
        videojs(document.querySelector(".video-js"));
        videoUp = true;
      }
    });
    return result;
  });

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
    <h1>{episodeData[0].title || `Episode ${episodeId}`}</h1>
    <video-js
      controls
      poster={episodeData[0].thumbnail_original}
      width="1920"
      height="1080"
      class="video-js"
    >
      <source src={episodeData[1].best_source} />
    </video-js>
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

  /*
    video {
        width: auto;
        height: 50vh;
    }
    */

  video-js {
    width: 70%;
    height: 50vh;
  }
</style>
