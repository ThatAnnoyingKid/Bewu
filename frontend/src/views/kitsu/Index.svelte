<script>
  import { link, location, querystring, push } from "svelte-spa-router";
  import Api from "../../api.js";

  let searchParams = new URLSearchParams($querystring);
  let searchText = searchParams.get("text");

  let searchResultsPromise =
    searchText === null ? null : Api.searchKitsu(searchText);

  let inputValue = searchText || "";
  function handleKeyDown(event) {
    if (event.keyCode == 13) {
      searchParams.set("text", inputValue);
      push($location + "?" + searchParams.toString());
      searchText = inputValue;
      searchResultsPromise = Api.searchKitsu(searchText);
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
        <ol class="search-results">
          {#each results as entry}
            <li>
              <a href="/kitsu/{entry.id}" use:link>
                <img
                  src={entry.poster_large}
                  alt="{entry.title} cover image"
                  width="550"
                  height="780"
                />
                <div class="search-entry-title-container">
                  <h2>{entry.title}</h2>
                </div>
              </a>
            </li>
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

  .search-results {
    display: flex;
    flex-direction: row;
    flex-wrap: wrap;
    justify-content: center;
    list-style-type: none;
    margin-block-start: 0;
    margin-block-end: 0;
    padding-inline-start: 0;
  }

  .search-results li a {
    align-items: center;
    color: var(--main-text-color);
    display: flex;
    flex-direction: column;
    padding: 0.2em;
    text-decoration: none;
    width: 10em;
  }

  .search-results img {
    height: auto;
    text-align: center;
    width: 10em;
  }

  .search-entry-title-container h2 {
    font-weight: 100;
    margin: 0 0;
    text-align: center;
  }
</style>
