class Api {
  constructor() {}

  async searchKitsu(text) {
    let response = await fetch(`/api/kitsu/anime/search?text=${text}`);
    let json = await response.json();
    if (response.status != 200) {
      throw json;
    }
    return json;
  }
}

export default new Api();
