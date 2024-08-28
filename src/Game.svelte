<script lang="ts" context="module">
  import { writable } from "svelte/store";
  import { Game } from "./lib/libKnuckleBones";
  import { arrayBufferToBase64 } from "./lib/util";

  const boardSize = {
    width: 3,
    height: 3,
  };

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
    boardSize,
    serverData,
    myId
  );

  const kid = new Game(
    player2 as CryptoKeyPair,
    player1 as CryptoKeyPair,
    boardSize,
    serverData,
    myId + 10
  );

  //   await kid.addOpponentMove(await boss.place(0));
  //   await boss.addOpponentMove(await kid.place(0));
  //   await kid.addOpponentMove(await boss.place(0));
  //   await boss.addOpponentMove(await kid.place(0));
  //   await kid.addOpponentMove(await boss.place(0));
  //   await boss.addOpponentMove(await kid.place(0));
  //   await kid.addOpponentMove(await boss.place(1));
  //   await boss.addOpponentMove(await kid.place(1));
  //   await kid.addOpponentMove(await boss.place(1));
  //   await boss.addOpponentMove(await kid.place(1));
  //   await kid.addOpponentMove(await boss.place(1));
  //   await boss.addOpponentMove(await kid.place(1));
  //   await kid.addOpponentMove(await boss.place(1));
  //   await boss.addOpponentMove(await kid.place(1));
  //   await kid.addOpponentMove(await boss.place(1));
  //   await boss.addOpponentMove(await kid.place(1));

  //   kid.debugPrint();

  let state = writable(boss.getBoardData());
</script>

<div class="flex gap-4 mx-auto">
  <div class="ml-auto">
    Next dice {$state.nextDice} <br />
    Seq {$state.seq}
  </div>
  <div class="max-w-40 flex gap-8 flex-col mr-auto">
    <div class="grid grid-cols-3 gap-3">
      {#each $state.decks.me as row, index}
        <button
          class="size-10 flex justify-center bg-slate-600 text-white text-center text-3xl"
          on:click={async () => {
            await kid.addOpponentMove(
              await boss.place(index % boardSize.width)
            );

            await boss.addOpponentMove(
              await kid.place(
                (index + Math.round(Math.random() * 3)) % boardSize.width
              )
            );

            state.set(boss.getBoardData());
          }}
        >
          {row}
        </button>
      {/each}
      {#each $state.points.me as row}
        <div
          class="size-10 flex justify-center bg-slate-900 text-white text-center text-3xl"
        >
          {row}
        </div>
      {/each}
    </div>
    <div class="grid grid-cols-3 gap-3 mx-auto">
      {#each $state.decks.other as row}
        <div
          class="size-10 flex justify-center bg-slate-600 text-white text-center text-3xl"
        >
          {row}
        </div>
      {/each}
      {#each $state.points.other as row}
        <div
          class="size-10 flex justify-center bg-slate-900 text-white text-center text-3xl"
        >
          {row}
        </div>
      {/each}
    </div>
  </div>
</div>
