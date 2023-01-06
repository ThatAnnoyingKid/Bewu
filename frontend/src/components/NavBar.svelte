<script>
  import { link } from "svelte-spa-router";
</script>

<nav>
  <ul>
    <li class="home">
      <a href="/" use:link>Bewu</a>
    </li>
    <li>
      <a href="/kitsu" use:link>Kitsu</a>
    </li>
  </ul>
</nav>

<style>
  nav {
    background-color: var(--secondary-bg-color);
    display: flex;
    height: 3em;
  }

  ul {
    align-items: center;
    display: flex;
    flex-direction: row;
    list-style-type: none;
    margin: 0;
    margin-block-start: 0;
    margin-block-end: 0;
    padding: 0.4em;
  }

  li {
    display: block;
    font-size: 1.5em;
    padding: 0.2em;
  }

  a {
    color: var(--main-text-color);
    text-decoration: none;
  }

  .home {
    font-size: 2em;
  }
</style>
