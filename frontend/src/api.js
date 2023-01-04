class Api {
  constructor() {}

  async searchKitsu(text) {
    let response = await fetch(`/api/kitsu/anime/search?text=${text}`);
    let json = await response.json();
    if (response.status != 200) {
      let error = null;
      for (let i = json.messages.length - 1; i >= 0; i--) {
        if (error == null) {
          error = new Error(json.messages[i]);
        } else {
          error = new Error(json.messages[i], { cause: error });
        }
      }

      throw error;
    }
    return json;
  }
}

export default new Api();
