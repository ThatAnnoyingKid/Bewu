<script>
  import Api from "../../Api.js";

  export let params = {};

  let animeId = params.id;

  let episodeData = Api.getKitsuEpisodes(animeId);

  function myFunction() {
    alert("YOU'RE DONE BUCKO!!!");
  }
</script>

<div>
  {#await episodeData}
    <p>Loading...</p>
  {:then episodeData}
    <h1>{episodeData[0].title}</h1>
    <video
      controls
      src="https://www.w3schools.com/tags/movie.mp4"
      poster="https://www.bearmageddonnews.com/wp-content/uploads/2016/10/gatlingbear-700x445.jpg"
      width="400"
      height="300"
    />
  {/await}
  <a href="https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    ><button on:click={myFunction}>Download</button></a
  >
</div>
