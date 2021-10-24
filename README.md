### Urban card game recreation

This is full recreation of the mechanics of the Urban card game in node.js, with typescript, originally build in javascript.

## Setup

Initialise the app with

```bash
npm init
```

Or with Yarn

```bash
yarn init
```

## Usage

You can create a new game and define the hands of the 2 players with

```javascript
// Don't take stdin inputs
const game = Game.create(false);

game.select(0, 3, false); // Play the first (index 0) card with 3 pillz (min 0) and no fury.

game.select(1, 0); // Opponent selects second card with 0 pillz on it, (no fury by default)

game.select(0); // Opponent starts next round by playing his first card, with 0 pillz by default.
```
