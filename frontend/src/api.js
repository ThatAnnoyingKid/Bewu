class Api {
  constructor() {}

  async searchKitsu(text) {
    let params = new URLSearchParams();
    if (text !== null && text !== undefined) params.set("text", text);

    let response = await fetch(`/api/kitsu/anime?${params}`);
    let json = await response.json();
    if (response.status != 200) throw convertToError(json);
    return json;
  }

  async getKitsuAnime(id) {
    let response = await fetch(`/api/kitsu/anime/${id}`);
    let json = await response.json();
    if (response.status != 200) throw convertToError(json);
    return json;
  }

  async getKitsuEpisodes(id) {
    let response = await fetch(`/api/kitsu/anime/${id}/episodes`);
    let json = await response.json();
    if (response.status != 200) throw convertToError(json);
    return json;
  }

  async getKitsuEpisode(id) {
    let response = await fetch(`/api/kitsu/episodes/${id}`);
    let json = await response.json();
    if (response.status != 200) throw convertToError(json);
    return json;
  }

  async *downloadVidstreamingEpisode(id) {
    let source = new EventSource(`/api/vidstreaming/${id}`);
    let store = {
      resolve: () => {},
      reejct: () => {},
    };
    let shouldExit = false;

    source.addEventListener("message", (event) => {
      let data = JSON.parse(event.data);
      store.resolve(data);
    });
    source.addEventListener("error", (event) => {
      console.error(event);
      store.reject(event);
    });
    source.addEventListener("close", (event) => {
      shouldExit = true;
      source.close();
    });

    while (!shouldExit) {
      yield new Promise((resolve, reject) => {
        store.resolve = resolve;
        store.reject = reject;
      });
    }
  }
}

function convertToError(json) {
  let error = null;
  for (let i = json.messages.length - 1; i >= 0; i--) {
    if (error == null) {
      error = new Error(json.messages[i]);
    } else {
      error = new Error(json.messages[i], { cause: error });
    }
  }

  return error;
}

let api = new Api();
window.api = api;
export default api;
