# Galaxies

## One server, one galaxy

A server process is one galaxy. It is identified by a stable `galaxy-id` and generated from a
`seed`. Every probe that connects for the first time gets the next **home system**. Home systems
lie along four spiral arms, at least 60 light years apart (`HOME_SPACING_LY`), so a new player
meets nobody until they choose to travel. That distance is what protects new players.

Each home system has 4 to 6 planets, and exactly one of them is **viable**: the world where, at the
end of the long game, the payload can be delivered.

## Linking galaxies (planned)

Galaxies will be linked, so that a player can travel from one server to another. What is already
in place for it:

- A galaxy has a stable id, sent to the client in `Welcome`.
- Players are keyed by name *within a galaxy*; a galaxy's file holds only its own probes.
- `ServerMessage::Transfer { address, galaxy_id, ticket }` is in the protocol: the server tells the
  client to reconnect to another server, which recognizes the player by a one-time ticket.

What is left:

- An identity that spans galaxies (accounts, or a key pair per player).
- Handing the player's ship and cargo from one server to the other, either signed by the sending
  galaxy or through a shared store.
- The in-game gateway that triggers the transfer.
