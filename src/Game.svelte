<script lang="ts">
  import {
      Game,
      sign_message,
  } from "./lib/wasmdev/lib_knuckle";
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

  let game: Game;
  let gameInfo =$state()

  let state = $state()

  let backendUrl = import.meta.env.DEV ? "http://localhost:8083" : ``;


  const icons = [
      Dice0,
      Dice1,
      Dice2,
      Dice3,
      Dice4,
      Dice5,
      Dice6,
  ]

  let ws: WebSocket;
  let peerConnection: RTCPeerConnection;
  let dataChannel: RTCDataChannel;
  let dialog:HTMLDialogElement= $state();
  let disconnectedDialog=$state();

  let status:string|undefined =$state()

  let pub_key:string;
  let priv_key:string

  function startChat() {
    status = "Starting Connection"

      if (
          import.meta.env.DEV) {
          ws = new WebSocket("ws://localhost:8083/ws");
      } else {
          ws = new WebSocket(`${window.origin.replace("http", "ws")}/ws`);
      }

      ws.onopen = () => {

      };

      ws.onmessage = async (event) => {
          let message;

          try {
              message = JSON.parse(event.data);
          } catch (e) {
              const data = new TextDecoder().decode(await event.data.arrayBuffer());
              message = JSON.parse(data);
          }
          console.log("WS MSG", message);
          switch (message.type) {
              case "verify":
                let data = await fetch(`${backendUrl}/signup`)
                let json = await data.json();
                const private_key = json.priv_key;
                const response = await sign_message(private_key, message.verify_time);
               ws.send(JSON.stringify({
                    type: "join",
                    signature: response,
                    pub_key: json.pub_key
                }));
                pub_key = json.pub_key;
                priv_key = private_key;
                break;
              case "paired":
                status = 'Verified'
                  game = new Game(
                      pub_key,
                      priv_key,
                      message.partner_key,
                      boardSize.width,
                      boardSize.height,
                      message.initiator,
                      BigInt(message.seed)
                  );
                  state = await game.w_get_board_data();
                  initializePeerConnection(message.initiator);

                  gameInfo = message;

                  window.game = game;

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
async  function initializePeerConnection(isInitiator) {
    console.log("INit peer connection")
      peerConnection = new RTCPeerConnection({
          iceServers: [{
                    'urls': [
                        'stun:stun.l.google.com:19302',
                        'stun:stun1.l.google.com:19302',
                        'stun:stun2.l.google.com:19302',
                    ]
                }]
      });

      peerConnection.onicecandidate = (event) => {
        status ="Getting further (ICE Candidate)"
        console.log("Sending icecandidate!!!")
        console.log(event)
          if (event.candidate) {
              ws.send(
                  JSON.stringify({
                      type: "ice-candidate",
                      candidate: event.candidate,
                  })
              );
          }
      };


      ;
      window.pc = peerConnection;
      peerConnection.addEventListener("connectionstatechange", (event) => {
        console.log("connectionstatechange", event);
      })
      if (isInitiator) {
        console.log("Creating datachannel")
        dataChannel = peerConnection.createDataChannel("chat");
        setupDataChannel();

        const offer = await peerConnection.createOffer();
        console.log("Offer", offer)
        const answer = await peerConnection.setLocalDescription(offer);

        ws.send(
                JSON.stringify({
                    type: "offer",
                    offer: peerConnection.localDescription,
                })
            );
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
          ws.close()
          status =null;
      };

      dataChannel.onclose = () => {
        if (state == undefined || state?.is_completed) return;
          status = "Connection closed"
          disconnectedDialog.showModal()
          console.log("Datachannel closed")
      }

      dataChannel.onmessage = (event) => {
        let data = new Uint8Array(event.data);
          console.log(event);
          console.log("Received Data", data)
          console.log(game.w_add_opponent_move(data));
          state = game.w_get_board_data();
      };
  }

  function resetChat() {
    if(state.decks){
        state.decks.me = undefined
        state.decks.other = undefined

    }
    if(state) {
        state.decks = undefined
        state.points = undefined;

    }
    try {
        game.free()
    } catch(e) {
        // oh well
    }

        state = undefined;
        gameInfo = undefined;
        dataChannel.close()
        peerConnection.close()
        peerConnection = null!
        dataChannel = null!

  }
  $inspect(gameInfo)

  $effect(() => {
    if(state?.is_completed) {
      dialog.showModal()
    }
  })
</script>

<svelte:options runes={true} ></svelte:options>

{#snippet diceLayout(deck, points, onclick)}
  {#each deck ?? [] as row, index}
    <button
        class="size-10 flex justify-center bg-slate-600 text-white text-center text-3xl"
        onclick={() => onclick(index)}
        ondrop={() => onclick(index)}
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

<dialog bind:this={dialog}>
    Game Completed:<br>
    Your score: {state?.points?.me?.reduce((a, b) => a + b, 0)}<br>
    Opponent score: {state?.points?.other?.reduce((a, b) => a + b, 0)}<br>
    <button onclick={() => {
       resetChat();
       dialog.close();
    }}>Return to start</button>
</dialog>

<dialog bind:this={disconnectedDialog}>
    Your opponent disconnected, Please start a new game.
    <button onclick={() => {
       resetChat();
       disconnectedDialog.close();
    }}>Return to start</button>
</dialog>

{#if gameInfo}



{#if status}
    <span>{status}</span>
{:else}

<div class="flex gap-4 mx-auto">
    <div class="ml-auto">
        Next dice {state?.next_dice}<br />
        Seq {state?.seq}<br />
        Starting: {gameInfo?.initiator}<br />
        your turn: {state?.your_turn}<br />
        Completed: {state?.is_completed}<br />
        Your id {gameInfo?.public_key?.slice(0,5)}<br/>
        Partner id {gameInfo?.partner_key?.slice(0,5)}<br/>

        {#if state?.your_turn}

        <!--TODO: fix this {@render icons[state?.next_dice]({
    class: "size-28 p-4"
        })} -->
        <svelte:component this={icons[state?.next_dice]} class="size-28 p-4" />
        {/if}

    </div>
    <div class=" flex gap-8 flex-col mr-auto">
        <div class="grid grid-cols-3 gap-3">
            {@render diceLayout(state?.decks.me, state?.points?.me, (index:number) => {
            if(!state?.your_turn) {alert("Not your turn");return}
            if(state?.is_completed) {alert("Game is over!");return}
            let pos = index % boardSize.width;
            let error = game.w_test_place(pos);
            if(error) {
                alert(error);
                return;
            }
            const sending = game.w_place(index % boardSize.width);
            console.log("Sending Bytes", sending)
            dataChannel.send(sending);
            state = game.w_get_board_data();
            })}

        </div>
        <span class="text-1xl font-semibold">Opponents layout: </span>
        <div class="grid grid-cols-3 gap-3 mx-auto">
            {@render diceLayout(state?.decks.other, state?.points?.other, (index:number) => {
            console.log("Tried clicking on other dice ", index)
            })}
        </div>
    </div>
</div>
{/if}

{:else}
        <button onclick={startChat}>Play</button>

{/if}