import {
  arrayBufferToBase64,
  base64ToArrayBuffer,
  shiftZerosDown,
} from "./util.ts";

import { Game } from "./libKnuckleBones.ts";
import { xorshift } from "./rng.ts";

const keyType = "Ed25519";

const diceholder = (await crypto.subtle.generateKey(
  {
    name: keyType,
  },
  true,
  ["sign", "verify"]
)) as CryptoKeyPair;

const player1 = (await crypto.subtle.generateKey(
  {
    name: keyType,
  },
  true,
  ["sign", "verify"]
)) as CryptoKeyPair;

const player2 = (await crypto.subtle.generateKey(
  {
    name: keyType,
  },
  true,
  ["sign", "verify"]
)) as CryptoKeyPair;

const seedgen = () => (Math.random() * 2 ** 32) >>> 0;

const seed = 573537897831321;
// const seed = seedgen()

const myId = 1;

let signature = await crypto.subtle.sign(
  keyType,
  diceholder.privateKey,
  new TextEncoder().encode(`${myId}:${seed}`)
);

const serverData = {
  starter: myId,
  seed: seed,
  signature: arrayBufferToBase64(signature),
};

const boss = new Game(
  player1 as CryptoKeyPair,
  player2 as CryptoKeyPair,
  {
    height: 3,
    width: 3,
  },
  serverData,
  myId
);

const kid = new Game(
  player2 as CryptoKeyPair,
  player1 as CryptoKeyPair,
  {
    height: 3,
    width: 3,
  },
  serverData,
  myId + 10
);

await kid.addOpponentMove(await boss.place(0));
await boss.addOpponentMove(await kid.place(0));
await kid.addOpponentMove(await boss.place(0));
await boss.addOpponentMove(await kid.place(0));
await kid.addOpponentMove(await boss.place(0));
await boss.addOpponentMove(await kid.place(0));
await kid.addOpponentMove(await boss.place(1));
await boss.addOpponentMove(await kid.place(1));
await kid.addOpponentMove(await boss.place(1));
await boss.addOpponentMove(await kid.place(1));
await kid.addOpponentMove(await boss.place(1));
await boss.addOpponentMove(await kid.place(1));
await kid.addOpponentMove(await boss.place(1));
await boss.addOpponentMove(await kid.place(1));
await kid.addOpponentMove(await boss.place(1));
await boss.addOpponentMove(await kid.place(1));

kid.debugPrint();
