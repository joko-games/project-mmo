# Game design

## Pitch

You are a probe, sent from very far away a very long time ago. You wake up automatically when you
reach a young star system, carrying a payload: the seed of life, to be planted on a viable world.
That system is yours. It is both your tutorial and your home base.

## The probe

Deliberately not realistic. Three parts nested one inside the other, and the whole is not a
sphere: it is a drop pulled backwards, round at the nose and drawn out to a point at the tail.

1. **The structure**, outside: six ribs swept back from the nose to the tail, with hoops across
   them. The main engine is at the tail, and smaller thrusters ring the nose and the tail and sit at
   the nose's tip. A thruster glows, and its flame grows, when it pushes.
2. **The core**, inside the structure: the probe's thinking heart, a smaller translucent drop that
   turns on its long axis.
3. **The payload**, at the center of the core: the seed of life, beating slowly.

## Two ways to fly

- **Structure** (the default): the whole probe, with inertia. The view turns freely on every axis
  and rolls; the probe turns its nose where it is pushed.
- **Core** (Tab): the core detaches and the player flies it alone, with a simpler navigation, the
  one building will use. There is no inertia (it stops when nothing is pressed), the view turns
  and tilts without rolling, and the structure gives the reference: forward and sideways in the
  plane of its axes, Space and Ctrl along its up. The core stays within 250 units of the structure,
  which drifts to a stop meanwhile. Tab again, and the core flies itself back into place.

## No up

Space has no up. Flying the structure, the view turns freely on every axis, rolls included, and
nothing on screen marks a vertical: the menu's sky rolls, the planet labels sit above the planets
as the camera sees them. The only up is the structure's own, used when flying the core, which is
where building will happen.

## The false goal

Delivering the payload is the official end, reachable in about ten hours. Like the End in
Minecraft, it is optional: the real game is the sandbox around it. That means building ships block
by block, as in Avorion, and blocks with behavior (push, pull, signals, logic), as in early
StarMade.

## v0.1.0

The first slice: launch the game, pick a name (pre-filled at random) and connect to the local
galaxy server. You wake up as the probe, with a fade from black and the wake-up log, and fly
around your home system. There are no blocks, no mining and no payload delivery yet.
