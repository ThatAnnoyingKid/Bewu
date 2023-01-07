class Api {
  constructor() {}

  async searchKitsu(text) {
    let response = await fetch(`/api/kitsu/anime?text=${text}`);
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
