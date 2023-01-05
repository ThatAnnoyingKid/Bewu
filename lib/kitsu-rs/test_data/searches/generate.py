import requests
import json

searches = [
    "3-gatsu_no_Lion_2nd_Season",
    "cowboy bebop",
]

def main():
    for search in searches:
        response = requests.get(f'https://kitsu.io/api/edge/anime?filter[text]={search}')
        response.raise_for_status()
        
        response_json = response.json()
        with open(f'{search}.json', 'wb') as f:
            f.write(json.dumps(response_json, indent=4).encode('utf-8'))
    
if __name__ == "__main__":
    main()
    