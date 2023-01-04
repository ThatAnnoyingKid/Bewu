<script>
  import Api from "../api.js";

  let searchResultsPromise = null;

  let inputValue = "";
  function handleKeyDown(event) {
    if (event.keyCode == 13) {
      searchResultsPromise = Api.searchKitsu(inputValue);
    }
  }
</script>

<div class="container">
  <div class="input-container">
    <input
      autocomplete="off"
      maxlength="128"
      name="search"
      placeholder="Search"
      type="search"
      bind:value={inputValue}
      on:keydown={handleKeyDown}
    />
  </div>
  {#if searchResultsPromise !== null}
    {#await searchResultsPromise}
      Loading...
    {:then results}
      {#if results.length == 0}
        No Results
      {:else}
        <ol>
          {#each results as entry}
            <li>{JSON.stringify(entry)}</li>
          {/each}
        </ol>
      {/if}
    {:catch error}
      {error.message}
    {/await}
  {/if}
</div>

<style>
  .container {
    padding: 0.5em;
  }

  .input-container {
    display: flex;
    justify-content: center;
    margin-top: 2em;
    margin-bottom: 2em;
  }

  .input-container input {
    appearance: none;
    background-color: var(--main-bg-color);
    border-color: var(--secondary-bg-color);
    border-radius: 0.2em;
    border-style: solid;
    color: var(--main-text-color);
    flex-grow: 0.5;
    font-size: 1.5em;
    font-family: "sans-serif";
    padding: 0.2em;
    outline: none;
  }

  .input-container input::placeholder {
    color: var(--main-text-color);
    filter: brightness(85%);
  }

  .input-container input::-webkit-search-cancel-button {
    display: none;
  }

  .input-container input:focus {
    background-color: var(--secondary-bg-color);
    border-color: var(--secondary-bg-color);
  }

  .input-container input:hover {
    background-color: var(--secondary-bg-color);
    border-color: var(--secondary-bg-color);
  }
</style>
