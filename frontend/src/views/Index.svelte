<script>
  import Api from "../api.js";
  
  let raw = '';

  let inputValue = "";  
  function handleKeyDown(event) {
    if(event.keyCode == 13) {
        Api.searchKitsu(inputValue)
            .then((json) => {
                raw += JSON.stringify(json);
            }).catch((error) => {
                alert(error);
            });
    }
  }
</script>

<div class="container">
  <div class="input-container">
    <input
      autocomplete="off"
      maxlength="256"
      name="search"
      placeholder="Search"
      type="search"
      bind:value={inputValue}
      on:keydown={handleKeyDown}
    />
  </div>
  {raw}
</div>

<style>
  .container {
    padding: 0.5em;
  }

  .input-container {
    display: flex;
    justify-content: center;
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
