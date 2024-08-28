export function sfc32(seed: number) {
  let state0 = seed;
  let state1 = ~seed; // Complementary seed for the second state

  function next() {
    let s1 = state0;
    let s0 = state1;
    state0 = s0;
    s1 ^= s1 << 23; // a
    state1 = (s1 ^ s0 ^ (s1 >> 17) ^ (s0 >> 26)) >>> 0;
    return (state1 + s0) >>> 0;
  }

  function rollDice() {
    return (next() % 6) + 1;
  }

  return rollDice;
}
