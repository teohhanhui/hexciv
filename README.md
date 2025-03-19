# Hexciv

[Civ VI] inspired game, written in [Rust] using the [Bevy] game engine.

[Bevy]: https://bevyengine.org/
[Civ VI]: https://civilization.fandom.com/wiki/Civilization_VI
[Rust]: https://www.rust-lang.org/

## Setup

### Setup matchbox_server

```
cargo install matchbox_server
```

### Setup wasm-server-runner

Install and setup [wasm-server-runner].

[wasm-server-runner]: https://github.com/jakobhellermann/wasm-server-runner

## Run

### Run matchbox_server

```
matchbox_server
```

### Serve the game

```
WASM_SERVER_RUNNER_ADDRESS=0.0.0.0 cargo run --target wasm32-unknown-unknown
```

## Roadmap

(We're targeting only the base game without expansion packs and DLCs for now.)

- [x] Map generation
    - [x] Generate elevation map using <https://github.com/TadaTeruki/fastlem>

        Known issues:
        * This is not ideal as it's not designed for "world map" style terrain generation, but it's a quick and easy way
            to have something. See <https://github.com/TadaTeruki/fastlem/issues/3>

    - [x] [Base terrain](https://civilization.fandom.com/wiki/Terrain_(Civ6)#Base_terrain)
        - [x] [Plains](https://civilization.fandom.com/wiki/Plains_(Civ6))
        - [x] [Grassland](https://civilization.fandom.com/wiki/Grassland_(Civ6))
        - [x] [Desert](https://civilization.fandom.com/wiki/Desert_(Civ6))

            Known issues:
            * Desert should be contiguous regions, not just scattered all over the place.
        - [x] [Tundra](https://civilization.fandom.com/wiki/Tundra_(Civ6))
        - [x] [Snow](https://civilization.fandom.com/wiki/Snow_(Civ6))
        - [x] [Hills](https://civilization.fandom.com/wiki/Hills_(Civ6))
        - [x] [Mountains](https://civilization.fandom.com/wiki/Mountains_(Civ6))

            Known issues:
            * High elevation areas tend to clump together. See <https://github.com/TadaTeruki/fastlem/issues/3>
        - [x] [Coast](https://civilization.fandom.com/wiki/Coast_(Civ6))

            Known issues:
            * Currently any water tile adjacent to any land tile is treated as coast.
        - [ ] [Lake](https://civilization.fandom.com/wiki/Lake_(Civ6))

            We need a way to detect enclosed bodies of water.
        - [x] [Ocean](https://civilization.fandom.com/wiki/Ocean_(Civ6))

    - [ ] [Terrain features](https://civilization.fandom.com/wiki/Terrain_(Civ6)#Terrain_features)
        - [x] [Woods](https://civilization.fandom.com/wiki/Woods_(Civ6))
        - [x] [Rainforest](https://civilization.fandom.com/wiki/Rainforest_(Civ6))
        - [ ] [Marsh](https://civilization.fandom.com/wiki/Marsh_(Civ6))
        - [ ] [Floodplains](https://civilization.fandom.com/wiki/Floodplains_(Civ6))
        - [x] [Oasis](https://civilization.fandom.com/wiki/Oasis_(Civ6))
        - [ ] [Cliffs](https://civilization.fandom.com/wiki/Cliffs_(Civ6))
        - [x] [Ice](https://civilization.fandom.com/wiki/Ice_(Civ6))
        - [x] [River](https://civilization.fandom.com/wiki/River_(Civ6))

            Known issues:
            * This needs a lot more work to have rivers that feel right. See for example <https://en.wikipedia.org/wiki/Stream_order>

    - [ ] [Resources]
        - [ ] [Bonus](https://civilization.fandom.com/wiki/Resource_(Civ6)#Bonus)
        - [ ] ~~[Luxury](https://civilization.fandom.com/wiki/Resource_(Civ6)#Luxury)~~
        - [ ] [Strategic](https://civilization.fandom.com/wiki/Resource_(Civ6)#Strategic)

    - [ ] ~~[Natural wonders]~~

- [ ] Spawning of [starting units](https://civilization.fandom.com/wiki/Era_(Civ6)#Starting_units_and_statistics)
    - [ ] Space out the starting positions for different civs
    - [ ] Spawn civs with [starting bias](https://civilization.fandom.com/wiki/Starting_bias_(Civ6))

- [ ] Unit movement with pathfinding
    - [x] Pathfinding using A* search algorithm
    - [ ] Limit pathfinding to partial knowledge (i.e. "fog of war")
        1. Only tiles already explored by the current player would have a known movement cost.
        2. Only tiles already explored by the current player would have known presence / absence of neighboring tiles.
            If the neighboring tile positions have never been inside any unit's sight range, they must be assumed to
            exist.
        3. If there are any changes allowing / denying movement since the last seen time, the changes must NOT be taken
            into consideration. Pathfinding must be based on the last known map by the current player.
    - [ ] Queue movement for next turns when there's not enough movement points
    - [ ] Show indication if there is no path for a move
    - [ ] Conditionally allow units to [embark](https://civilization.fandom.com/wiki/Movement_(Civ6)#Embarking)

- [x] Simultaneous turns

    Known issues:
    * TBD

- [ ] [Founding of new cities](https://civilization.fandom.com/wiki/City_(Civ6)#Founding_a_City)

- [ ] [City population](https://civilization.fandom.com/wiki/Population_(Civ6))
    - [ ] [Food]
    - [ ] ~~[Housing]~~
    - [ ] ~~[Amenities]~~

- [ ] [City production](https://civilization.fandom.com/wiki/City_(Civ6)#City_Production)
    - [ ] [Districts]
    - [ ] [Buildings]
    - [ ] [Units]
    - [ ] [Wonders]
    - [ ] [Purchasing](https://civilization.fandom.com/wiki/City_(Civ6)#Purchasing) with [Gold]
        - [ ] [Buildings]
        - [ ] [Units]

- [ ] [Territorial expansion](https://civilization.fandom.com/wiki/Borders_(Civ6)#Territorial_expansion)
  - [ ] [By cultural influence](https://civilization.fandom.com/wiki/Borders_(Civ6)#By_cultural_influence)
  - [ ] [By purchasing](https://civilization.fandom.com/wiki/Borders_(Civ6)#By_purchasing)

- [ ] Basic [combat](https://civilization.fandom.com/wiki/Combat_(Civ6)) mechanics

- [ ] Saving and loading of a game

- [ ] Rejoining a game, e.g. after a disconnection / reconnection

- [ ] [Tech](https://civilization.fandom.com/wiki/Technology_(Civ6)) tree

- [ ] ~~[Civic](https://civilization.fandom.com/wiki/Civic_(Civ6)) tree~~

[Amenities]: https://civilization.fandom.com/wiki/Amenities_(Civ6)
[Buildings]: https://civilization.fandom.com/wiki/Building_(Civ6)
[Districts]: https://civilization.fandom.com/wiki/District_(Civ6)
[Food]: https://civilization.fandom.com/wiki/Food_(Civ6)
[Gold]: https://civilization.fandom.com/wiki/Gold_(currency)_(Civ6)
[Housing]: https://civilization.fandom.com/wiki/Housing_(Civ6)
[Natural wonders]: https://civilization.fandom.com/wiki/Natural_wonder_(Civ6)
[Resources]: https://civilization.fandom.com/wiki/Resource_(Civ6)
[Units]: https://civilization.fandom.com/wiki/Unit_(Civ6)
[Wonders]: https://civilization.fandom.com/wiki/Wonder_(Civ6)

## FAQs

* Will there be a single-player mode?

    No.

* How are multiplayer games hosted?

    There is a matchmaking server which connects players directly (peer-to-peer). There is no hosted server where game
    sessions are stored.

    Technical explanation:
    * Peer-to-peer WebRTC using <https://github.com/johanhelsing/matchbox>
    * All game logic processed on the host's game world
        * To make a move, other players must send requests to the host
        * Host broadcasts events to other players, which are then synced / updated in their own game world

* How do I join a game?

    The host needs to share the game session ID. You can join a game session by entering the ID.

    Note: Not implemented yet. Currently any 2 successive players who connect to the matchmaking server would be paired
    up with each other.

* How many players can we have in a game?

    2-4 players.

    Note: Not implemented yet. Currently any 2 successive players who connect to the matchmaking server would be paired
    up with each other.

* How do I rejoin a game if I got disconnected?

    You can rejoin an active game session by entering the ID. It's not possible to rejoin a game if the host leaves.

    Note: Not implemented yet. Currently any 2 successive players who connect to the matchmaking server would be paired
    up with each other.

* What happens if the host or another player disconnects?

    The game will be paused until all players reconnect.

    Note: Not implemented yet.

* Can I pause the game?

    Only the host can pause the game (by selecting "Pause" from the in-game menu, or by pressing `P` on the keyboard).

    Note: Not implemented yet.

* How does a player win a game? / How does a player achieve victory?

    This is currently not planned. Just keep playing for as long as you'd like (or you have wiped out everybody else).

## License

Licensed under either of

* Apache License, Version 2.0
    ([LICENSE-APACHE] or <https://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
    ([LICENSE-MIT] or <https://opensource.org/license/MIT>)

at your option.

[LICENSE-APACHE]: LICENSE-APACHE
[LICENSE-MIT]: LICENSE-MIT

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
