<script>
  import Api from "@/Api.js";
  import { afterUpdate } from "svelte";

  export let params = {};

  let episodeId = params.id;
  
  let downloadState = null;

  let kitsuEpisodeData = Api.getKitsuEpisode(episodeId);
  let vidstreamingEpisodeData = Api.getVidstreamingEpisode(episodeId);
  
  async function performVidstreamingDownload() {
    downloadState = {};
    for await (const event of Api.downloadVidstreamingEpisode(episodeId)) {
      switch(event.type) {
        case 'progress':
            downloadState.progress = event.progress;
            break;
        default:
            console.log(event);
      }
    }
    downloadState = null;
    vidstreamingEpisodeData = Api.getVidstreamingEpisode(episodeId);
  }

  let episodeData = Promise.all([kitsuEpisodeData, vidstreamingEpisodeData]);

  /*
  function myFunction() {
    alert("YOU'RE DONE BUCKO!!!");
  }
  */
</script>

<div class="container">
  {#await episodeData}
    <p>Loading...</p>
  {:then kitsuEpisodeData}
    <h1>{kitsuEpisodeData.title || `Episode ${episodeId}`}</h1>
    {#await vidstreamingEpisodeData}
      Loading...
    {:then vidstreamingEpisodeData}
      {#if vidstreamingEpisodeData.url !== null}
        <video
          controls
          poster={kitsuEpisodeData.thumbnail_original}
          width="1920"
          height="1080"
          src={vidstreamingEpisodeData.url}
        />
      {:else}
        {#if downloadState === null}
            Video is not downloaded: 
            <button on:click={performVidstreamingDownload}>
                Download
            </button>
        {:else}
            Progress: {downloadState.progress}
        {/if}
      {/if}
    {/await}
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
    width: 100%;
    height: auto;
    max-height: 70vh;
  }
</style>
