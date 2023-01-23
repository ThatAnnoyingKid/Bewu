import requests
import json

ids = [
    99605,
    89,
]

def main():
    for id in ids:
        response = requests.get(f'https://kitsu.io/api/edge/episodes/{id}')
        response.raise_for_status()
        
        response_json = response.json()
        with open(f'{id}.json', 'wb') as f:
            f.write(json.dumps(response_json, indent=4).encode('utf-8'))
    
if __name__ == "__main__":
    main()
    