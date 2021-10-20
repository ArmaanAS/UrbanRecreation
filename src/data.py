import json

# read file
with open('data.json', 'r', encoding='utf-8') as myfile:
    data = myfile.read()

# parse file
obj = json.loads(data)

print((sorted([o['ability'] for o in obj if len(o['ability']) > 43])))