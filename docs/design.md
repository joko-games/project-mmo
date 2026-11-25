# Game design

## Pitch

You are a probe, sent from very far away a very long time ago. You wake up automatically when you
reach a young star system, carrying a payload: the seed of life, to be planted on a viable world.
That system is yours. It is both your tutorial and your home base.

## The probe

Deliberately not realistic. Three parts nested one inside the other, sharing a center:

1. **The frame**, outside: the structure and the propulsion. Three rings at right angles and six
   thrusters, one at each end of each axis, so the probe pushes the same way in every direction;
   a thruster glows when it pushes.
2. **The core**, inside the frame: the probe's thinking heart, a faceted shell turning on itself.
   It will detach from the frame in some situations (it is already its own entity, `ProbeCore`).
3. **The payload**, at the center of the core: the seed of life, beating slowly.

## No up

Space has no up. In flight the view turns freely on every axis, rolls included, and nothing on
screen marks a vertical: the menu's sky rolls, the planet labels sit above the planets as the
camera sees them. Only the building mode to come will have an up.

## The false goal

Delivering the payload is the official end, reachable in about ten hours. Like the End in
Minecraft, it is optional: the real game is the sandbox around it. That means building ships block
by block, as in Avorion, and blocks with behavior (push, pull, signals, logic), as in early
StarMade.

## v0.1.0

The first slice: launch the game, pick a name (pre-filled at random) and connect to the local
galaxy server. You wake up as the probe, with a fade from black and the wake-up log, and fly
around your home system. There are no blocks, no mining and no payload delivery yet.
