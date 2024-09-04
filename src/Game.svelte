<script lang="ts">
  import { Game } from "./lib/wasm/lib_knuckle";
  import Dice1 from "./icons/dices/Dice1.svelte";
  import Dice2 from "./icons/dices/Dice2.svelte";
  import Dice3 from "./icons/dices/Dice3.svelte";
  import Dice4 from "./icons/dices/Dice4.svelte";
  import Dice5 from "./icons/dices/Dice5.svelte";
  import Dice6 from "./icons/dices/Dice6.svelte";
  import Dice0 from "./icons/dices/Dice0.svelte";

  const boardSize = {
    width: 3,
    height: 3,
  };

  const seed = 573537897831321;
  let game: Game;

  // let state = writable<any>();

  let state = $state()

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
  let starting: boolean = $state(false);

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

      switch (message.type) {
        case "paired":
          starting = message.initiator
          game = new Game(
            message.public_key,
            message.private_key,
            message.partner_key,
            boardSize.width,
            boardSize.height,
            message.initiator,
            BigInt(seed)
          );
          state = await game.w_get_board_data();
          initializePeerConnection(message.initiator);

          // ws.close()
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
      console.log("Received Data", new Uint8Array(event.data))
      game.w_add_opponent_move(new Uint8Array(event.data));
     state = game.w_get_board_data();
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
    $effect( () => {
      console.log({...state})
    })
</script>

<svelte:options runes={true} ></svelte:options>

{#snippet diceLayout(deck, points, onclick)}
{#each deck ?? [] as row, index}
<button
  class="size-10 flex justify-center bg-slate-600 text-white text-center text-3xl"
  onclick={() => onclick(index)}
>
  <svelte:component this={icons[row]} />
</button>
{/each}
{#each points ?? [] as row}
<div
  class="size-10 flex justify-center bg-slate-900 text-white text-center text-3xl"
>
  {row}
</div>
{/each}
{/snippet}

<div class="flex gap-4 mx-auto">
  <div class="ml-auto">
    Next dice {state?.next_dice}<br />
    Seq {state?.seq}<br />
    Starting: {starting}<br />
    your turn: {state?.your_turn}<br />

    {#if state?.your_turn}

      <!--TODO: fix this {@render icons[state?.next_dice]({
        class: "size-28 p-4"
      })} -->
      <svelte:component this={icons[state?.next_dice]} class="size-28 p-4" />
    {/if}

    <button onclick={startChat}>Play</button>
  </div>
  <div class=" flex gap-8 flex-col mr-auto">
    <div class="grid grid-cols-3 gap-3">
      {@render diceLayout(state?.decks.me, state?.points?.me, (index:number) => {
          if(!state?.your_turn) {alert("Not your turn");return}
            if(state?.is_completed) {alert("Game is over!");return}
            const sending =   game.w_place(index % boardSize.width);
            console.log("sending",sending)
            
            dataChannel.send(
             sending
            );

            state = game.w_get_board_data();
      })}
      
    </div>
    <span class="text-1xl font-semibold">Opponents layout: </span>
    <div class="grid grid-cols-3 gap-3 mx-auto">
      {@render diceLayout(state?.decks.other, state?.points?.other, (index:number) => {
        console.log("Tried clicking on ither dice ", index)
       })}
    </div>
  </div>
</div>
