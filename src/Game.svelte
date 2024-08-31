<script lang="ts">
  import { writable } from "svelte/store";
  import { Game } from "./lib/libKnuckleBones";
  import { arrayBufferToBase64 } from "./lib/util";
  import Dice1 from "./icons/dices/Dice1.svelte";
  import Dice2 from "./icons/dices/Dice2.svelte";
  import Dice3 from "./icons/dices/Dice3.svelte";
  import Dice4 from "./icons/dices/Dice4.svelte";
  import Dice5 from "./icons/dices/Dice5.svelte";
  import Dice6 from "./icons/dices/Dice6.svelte";
  import Dice0 from "./icons/dices/Dice0.svelte";
  import { base64ToArrayBuffer } from "./lib/util";

  const boardSize = {
    width: 3,
    height: 3,
  };

  const keyType = "Ed25519";

  const seedgen = () => (Math.random() * 2 ** 32) >>> 0;

  const seed = 573537897831321;
  // const seed = seedgen()

  const myId = 1;

  let game: Game;

  // const kid = new Game(
  //   player2 as CryptoKeyPair,
  //   player1 as CryptoKeyPair,
  //   boardSize,
  //   serverData,
  //   myId + 10
  // );

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

  let state = writable<ReturnType<Game["getBoardData"]>>();

  const icons = {
    0: Dice0,
    1: Dice1,
    2: Dice2,
    3: Dice3,
    4: Dice4,
    5: Dice5,
    6: Dice6,
  };

  let ws: WebSocket;
  let peerConnection: RTCPeerConnection;
  let dataChannel: RTCDataChannel;

  // startChatButton.addEventListener("click", startChat);
  // sendMessageButton.addEventListener("click", sendMessage);

  function startChat() {
    ws = new WebSocket("ws://localhost:8080");

    ws.onopen = () => {
      ws.send(JSON.stringify({ type: "join" }));
    };

    ws.onmessage = async (event) => {
      let message;

      try {
        message = JSON.parse(event.data);
      } catch (e) {
        const data = new TextDecoder().decode(await event.data.arrayBuffer());
        message = JSON.parse(data);
      }
      console.log(message);
      switch (message.type) {
        case "paired":
          const diceholder = (await crypto.subtle.generateKey(
            {
              name: keyType,
            },
            true,
            ["sign", "verify"]
          )) as CryptoKeyPair;
          let signature = await crypto.subtle.sign(
            keyType,
            diceholder.privateKey,
            new TextEncoder().encode(`${myId}:${seed}`)
          );

          let primary = message.initiator;

          const serverData = {
            starter: myId,
            seed: seed,
            signature: arrayBufferToBase64(signature),
          };
          // startChatButton.style.display = "none";
          // chatArea.style.display = "block";

          const myPublic = await crypto.subtle.importKey(
            "raw",
            base64ToArrayBuffer(message.public_key),
            { name: "Ed25519" },
            true,
            ["verify"]
          );
          const myPrivate = await crypto.subtle.importKey(
            "raw",
            base64ToArrayBuffer(message.private_key),
            { name: "Ed25519" },
            true,
            ["sign"]
          );
          const otherPublic = await crypto.subtle.importKey(
            "raw",
            base64ToArrayBuffer(message.partner_key),
            { name: "Ed25519" },
            true,
            ["verify"]
          );

          game = new Game(
            {
              privateKey: myPrivate,
              publicKey: myPublic,
            },
            {
              publicKey: otherPublic,
            },
            boardSize,
            serverData,
            primary ? myId : myId + 10
          );
          $state = game.getBoardData();
          initializePeerConnection(message.initiator);
          break;
        case "offer":
          await handleOffer(message);
          break;
        case "answer":
          await peerConnection.setRemoteDescription(
            new RTCSessionDescription(message.answer)
          );
          break;
        case "ice-candidate":
          await peerConnection.addIceCandidate(
            new RTCIceCandidate(message.candidate)
          );
          break;
        case "disconnected":
          // displayMessage("Your chat partner disconnected.");
          // resetChat();
          break;
      }
    };
  }

  function initializePeerConnection(isInitiator) {
    peerConnection = new RTCPeerConnection({
      iceServers: [{ urls: "stun:stun.l.google.com:19302" }],
    });

    peerConnection.onicecandidate = (event) => {
      if (event.candidate) {
        ws.send(
          JSON.stringify({
            type: "ice-candidate",
            candidate: event.candidate,
          })
        );
      }
    };

    if (isInitiator) {
      dataChannel = peerConnection.createDataChannel("chat");
      setupDataChannel();

      peerConnection
        .createOffer()
        .then((offer) => peerConnection.setLocalDescription(offer))
        .then(() => {
          ws.send(
            JSON.stringify({
              type: "offer",
              offer: peerConnection.localDescription,
            })
          );
        });
    } else {
      peerConnection.ondatachannel = (event) => {
        dataChannel = event.channel;
        setupDataChannel();
      };
    }
  }

  async function handleOffer(message) {
    await peerConnection.setRemoteDescription(
      new RTCSessionDescription(message.offer)
    );
    const answer = await peerConnection.createAnswer();
    await peerConnection.setLocalDescription(answer);
    ws.send(
      JSON.stringify({
        type: "answer",
        answer: peerConnection.localDescription,
      })
    );
  }

  function setupDataChannel() {
    dataChannel.onopen = () => {
      console.log("Opened");
    };

    dataChannel.onmessage = (event) => {
      console.log(event);
      game.addOpponentMove(JSON.parse(event.data));
    };
  }

  function resetChat() {
    if (peerConnection) {
      peerConnection.close();
    }
    if (dataChannel) {
      dataChannel.close();
    }
  }
</script>

<div class="flex gap-4 mx-auto">
  <div class="ml-auto">
    Next dice {$state?.nextDice} <br />
    Seq {$state?.seq}

    <button on:click={startChat}>Play</button>
  </div>
  <div class=" flex gap-8 flex-col mr-auto">
    <div class="grid grid-cols-3 gap-3">
      {#each $state?.decks.me ?? [] as row, index}
        <button
          class="size-10 flex justify-center bg-slate-600 text-white text-center text-3xl"
          on:click={async () => {
            // await kid.addOpponentMove(
            //   await boss.place(index % boardSize.width)
            // );

            // await boss.addOpponentMove(
            //   await kid.place(
            //     (index + Math.round(Math.random() * 3)) % boardSize.width
            //   )
            // );

            dataChannel.send(
              JSON.stringify(await game.place(index % boardSize.width))
            );

            $state = game.getBoardData();
          }}
        >
          <svelte:component this={icons[row]} />
        </button>
      {/each}
      {#each $state?.points.me ?? [] as row}
        <div
          class="size-10 flex justify-center bg-slate-900 text-white text-center text-3xl"
        >
          {row}
        </div>
      {/each}
    </div>
    <div class="grid grid-cols-3 gap-3 mx-auto">
      {#each $state?.decks.other ?? [] as row}
        <div
          class="flex justify-center text-white text-center p-3 bg-slate-800"
        >
          <svelte:component this={icons[row]} class="size-28" />
        </div>
      {/each}
      {#each $state?.points.other ?? [] as row}
        <div
          class="size-10 flex justify-center text-gray-900 text-3xl text-center w-full"
        >
          {row}
        </div>
      {/each}
    </div>
  </div>
</div>
