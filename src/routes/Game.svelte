<svelte:options runes={true} />

<script lang="ts">
  import { onMount } from "svelte";
  import {
    Game,
    sign_message,
    random_uuid,
    init,
    type BoardData,
    type GameBody,
    type LeaderBoard,
  } from "$src/lib/game";
  import Peer, { type PeerSignalData } from "$src/lib/peer/lite";
  import Dice from "$lib/components/Dice.svelte";
  import nameImg from "$assets/name.png?url";

  const boardSize = {
    width: 3,
    height: 3,
  };
  let backendUrl = import.meta.env.DEV ? "http://localhost:8083" : "https://api.knucklebones.fyi";

  let game: Game;
  let gameInfo: Partial<GameBody> & { initiator: boolean } & {
    [key: string]: string;
  } = $state(null!);
  let gameState: BoardData = $state(null!);

  let ws: WebSocket;
  let peerConnection: Peer;
  let dialog: HTMLDialogElement = $state(null!);
  let disconnectedDialog: HTMLDialogElement = $state(null!);
  let waitingDialog: HTMLDialogElement = $state(null!);
  let kickedDialog: HTMLDialogElement = $state(null!);
  let userDialog: HTMLDialogElement = $state(null!);
  let queueId: string | undefined = $state();

  let status: string | undefined = $state();

  let pub_key: string;
  let priv_key: string;
  let ice_servers: RTCIceServer;
  let wasm = $state(true);
  let autoplay = $state(false);

  const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

  // uses the window crypto is available for faster performance
  // otherwise falls back to a rust call which is MUCH slower
  // i know randomUUID wont work in non localhost http so thats why i use the fallback
  function genUUID() {
    try {
      return window.crypto.randomUUID();
    } catch (e) {
      return random_uuid();
    }
  }

  onMount(async () => {
    queueId = location.hash.replace("#", "");
    // we have to await it otherwise autoplay wont work
    await init();
    if (localStorage.getItem("autoplay")) {
      autoplay = true;
      startChat();

      let waitingDialogOpen = false;
      let kickedDialogOpen = false;
      let disconnectedDialogOpen = false;
      setInterval(
        () => {
          if (waitingDialogOpen) {
            if (waitingDialog.open) {
              window.location.reload();
            }
          }
          waitingDialogOpen = waitingDialog.open;
        },
        3000 + Math.random() * 5000,
      );
      setInterval(
        () => {
          if (kickedDialogOpen) {
            if (kickedDialog.open) {
              window.location.reload();
            }
          }
          kickedDialogOpen = kickedDialog.open;
        },
        3000 + Math.random() * 5000,
      );
      setInterval(
        () => {
          if (disconnectedDialogOpen) {
            if (disconnectedDialog.open) {
              window.location.reload();
            }
          }
          disconnectedDialogOpen = disconnectedDialog.open;
        },
        9000 + Math.random() * 5000,
      );
    }
  });

  async function startChat() {
    window.history.pushState(null, "", location.origin);
    // call just inc ase not inited
    await init();
    waitingDialog.showModal();
    status = "Starting Connection";

    if (import.meta.env.DEV) {
      ws = new WebSocket("ws://localhost:8083/ws");
    } else {
      ws = new WebSocket(`${window.origin.replace("http", "ws")}/ws`);
    }

    ws.onopen = () => {};

    ws.onmessage = async (event) => {
      let message: Record<string, any>;

      try {
        message = JSON.parse(event.data);
      } catch (e) {
        const data = new TextDecoder().decode(await event.data.arrayBuffer());
        message = JSON.parse(data);
      }
      console.log("WS MSG", message);
      switch (message.type) {
        case "verify": {
          const userInfo = import.meta.env.DEV ? localStorage.getItem("userInfo") : null;
          let json = userInfo ? JSON.parse(userInfo) : await fetch(`${backendUrl}/signup`).then((r) => r.json());
          localStorage.setItem("userInfo", JSON.stringify(json));
          const private_key = json.priv_key;
          const response = await sign_message(private_key, message.verify_time);
          ws.send(
            JSON.stringify({
              type: "join",
              signature: response,
              pub_key: json.pub_key,
              queue: queueId || undefined,
            }),
          );
          pub_key = json.pub_key;
          priv_key = private_key;
          break;
        }
        case "paired":
          waitingDialog.close();
          status = "Verified";
          game = new Game(
            pub_key,
            priv_key,
            message.partner_key,
            boardSize.width,
            boardSize.height,
            message.initiator,
            BigInt(message.seed),
          );
          gameState = await game.w_get_board_data();
          ice_servers = message.ice_servers;
          initializePeerConnection(message.initiator);
          //@ts-ignore -
          gameInfo = message;
          window.game = game;

          // ws.close()
          break;
        case "disconnected":
          //TODO: handle error message
          if (message.name == "UserDoesNotExist") {
            if (localStorage.getItem("userinfo.backup")) {
              localStorage.setItem("userInfo1", localStorage.getItem("userInfo")!);
            } else {
              localStorage.setItem("userinfo.backup", localStorage.getItem("userInfo")!);
            }
            localStorage.removeItem("userInfo");
            resetChat();
            setTimeout(() => {
              startChat();
            }, 200);
            return;
          }
          status = message.reason;
          waitingDialog.close();
          kickedDialog.showModal();
          break;
        default:
          console.log("Signaling message :", message);
          peerConnection?.signal(message as unknown as PeerSignalData);
      }
    };

    ws.onclose = () => {
      if (gameState !== undefined) return;
      console.log("Closed Dialog");
      waitingDialog.close();
      if (!status) status = "Websocket closed";
      kickedDialog.showModal();
    };
  }
  async function initializePeerConnection(isInitiator: boolean) {
    console.log("INit peer connection");

    peerConnection = new Peer({
      initiator: isInitiator,
      channelName: "game",
      config: {
        iceServers: [ice_servers],
        sdpSemantics: "unified-plan",
      },
    });

    peerConnection.on("signal", (data) => {
      ws.send(JSON.stringify(data));
    });

    function playRandomMove() {
      if (!autoplay) {
        return;
      }
      if (((Math.random() * 200) | 0) == 5) {
        let result = game.w_forfeit();
        peerConnection.send(result);
        return;
      }
      let xs: Set<number> = new Set();
      for (const [index, element] of gameState.decks.me.entries()) {
        if (element == 0) {
          xs.add(index % boardSize.width);
          break;
        }
      }
      // pick a random item from xs and set it as x;
      let x = [...xs][Math.floor(Math.random() * xs.size)];
      console.log("Placing", x);
      const sending = game.w_place(x);
      peerConnection.send(sending);
    }

    let onMessage = async (event: MessageEvent) => {
      if (event.data instanceof ArrayBuffer) {
        let data = new Uint8Array(event.data);
        console.log("Length", data.length);
        console.log(data);
        console.log(game.w_add_opponent_move(data));
        gameState = game.w_get_board_data();
      } else if (event.data instanceof Blob) {
        let data = new Uint8Array(await event.data.arrayBuffer());
        console.log("Length", data.length);
        console.log(data);
        console.log(game.w_add_opponent_move(data));
        gameState = game.w_get_board_data();
      }

      if (autoplay) {
        if (gameState.is_completed) {
          await sleep(1000);
          resetChat();
          await sleep(1000);
          dialog.close();
          disconnectedDialog.close();
          await sleep(1000);
          await startChat();
          return;
        } else {
          if (!gameState.your_turn) {
            return;
          }

          playRandomMove();
          gameState = game.w_get_board_data();
          if (gameState?.is_completed) {
            await sleep(1000);
            resetChat();
            await sleep(1000);
            dialog.close();
            disconnectedDialog.close();
            await sleep(1000);
            await startChat();
          }
        }
      }
    };

    let onChannelClose = async () => {
      try {
        peerConnection?.destroy();
      } catch (e) {}
      if (gameState === undefined || gameState?.is_completed) {
        return;
      }
      status = "Connection closed";
      disconnectedDialog.showModal();
      console.log("Datachannel closed");
    };

    peerConnection.on("connect", () => {
      ws.close();
      console.log("Connected to peer");
      status = undefined;
      peerConnection._channel.onmessage = onMessage;
      peerConnection._channel.onclose = onChannelClose;

      if (autoplay && gameState.your_turn) {
        setTimeout(() => {
          playRandomMove();
          gameState = game.w_get_board_data();
        }, 250);
      }
    });
    peerConnection.on("close", () => {
      console.log("Closed");
    });

    peerConnection.on("error", (e) => {
      console.log("Error", e);
    });

    peerConnection.on("end", () => {
      console.log("End!");
    });

    peerConnection.on("disconnect", () => {
      console.log("Disconnected");
      if (gameState === undefined || gameState?.is_completed) return;
      status = "Connection closed";
      disconnectedDialog.showModal();
      console.log("Datachannel closed");
    });
    peerConnection.on("signalingStateChange", (ev) => {
      console.log("Signaling state change", ev);
    });
  }

  function resetChat() {
    if (gameState) {
      gameState.decks = undefined!;
      gameState.points = undefined!;
      if (gameState?.decks) {
        gameState.decks.me = undefined!;
        gameState.decks.other = undefined!;
      }
    }
    try {
      game.free();
    } catch (e) {
      // oh well
    }

    gameState = undefined!;
    gameInfo = undefined!;
    try {
      peerConnection.destroy();
    } catch (e) {}
    peerConnection = null!;
  }

  $effect(() => {
    if (gameState?.is_completed) {
      dialog.showModal();
      (async () => {
        const body: GameBody = {
          seed: gameInfo!.seed!,
          time: gameInfo!.time!,
          your_key: gameInfo?.public_key!,
          opponent_key: gameInfo?.partner_key!,
          starting: gameInfo?.initiator!,
          signature: gameInfo?.signature!,
          moves: gameState.history,
        };
        console.log(body);
        await fetch(`${backendUrl}/submit_game`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify(body),
        });
      })();
    }
  });

  let leaderboardData: LeaderBoard = $state(null!);

  onMount(async () => {
    const supported = (() => {
      try {
        if (typeof WebAssembly === "object" && typeof WebAssembly.instantiate === "function") {
          const module = new WebAssembly.Module(Uint8Array.of(0x0, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00));
          if (module instanceof WebAssembly.Module)
            return new WebAssembly.Instance(module) instanceof WebAssembly.Instance;
        }
      } catch (e) {}
      return false;
    })();
    wasm = supported;

    leaderboardData = await fetch(`${backendUrl}/leaderboard`).then((r) => r.json());
    localStorage.setItem("version", "0");
  });

  function formatIntoColumnsCountPerColumn(arr: number[], width: number) {
    if (width <= 0 || !arr || arr.length === 0) return [];

    const result: Record<string | number, number>[] = new Array(width).fill(null).map(() => ({}));
    const height = Math.ceil(arr.length / width);

    for (let i = 0; i < arr.length; i++) {
      const col = i % width;
      const value = arr[i];
      if (result[col][value]) {
        result[col][value]++;
      } else {
        result[col][value] = 1;
      }
    }

    return result;
  }

  let highLightsMine = $derived(formatIntoColumnsCountPerColumn(gameState?.decks.me ?? [], 3));
  let highLightsOther = $derived(formatIntoColumnsCountPerColumn(gameState?.decks.other ?? [], 3));

  let name = $state("");
</script>

{#snippet diceLayout(
  deck: number[],
  points: number[],
  highLights: ReturnType<typeof formatIntoColumnsCountPerColumn>,
  onclick: any,
)}
  {#each deck ?? [] as row, index}
    {@const occurrences = highLights[index % 3]?.[row] ?? 0}
    <button
      class="md:size-28 size-20 relative flex justify-center text-center text-3xl p-4 {row == 0
        ? 'hover:brightness-110'
        : ''}"
      onclick={() => {
        console.log("Dropped");
        onclick(index);
      }}
      ondrop={() => onclick(index)}
      ondragover={(event) => {
        event.preventDefault();
        event.dataTransfer!.dropEffect = "copy";
      }}
    >
      <enhanced:img src="$assets/dice-bg.png" alt="" class="absolute left-0 top-0 h-full w-full" />
      <Dice number={row} {occurrences}></Dice>
    </button>
  {/each}
  {#each points ?? [] as row}
    <div class="flex justify-center">
      <div class="md:size-20 size-12 flex justify-center text-white text-center text-3xl relative">
        <enhanced:img src="$assets/number-base.png" alt="" class="absolute left-0 top-0 h-full w-full"> </enhanced:img>
        <div class="absolute top-[50%] left-[50%] translate-x-[-50%] translate-y-[-50%]">
          {row}
        </div>
      </div>
    </div>
  {/each}
{/snippet}

<dialog bind:this={dialog} class="bg-transparent text-white">
  <div class="flex flex-col h-[50rem] w-[25rem] max-w-full">
    <enhanced:img src="$assets/ending-base.png" alt="" class="absolute left-0 top-0 h-full w-full" />
    {#if gameState?.winner.winner}
      <enhanced:img src="$assets/ending-win.png" alt="" class="absolute left-0 top-0 h-full w-full" />
    {:else if gameState?.winner?.win_by_tie}
      <enhanced:img src="$assets/ending-draw.png" alt="" class="absolute left-0 top-0 h-full w-full" />
    {:else}
      <enhanced:img src="$assets/ending-lose.png" alt="" class="absolute left-0 top-0 h-full w-full" />
    {/if}
    <div class="w-full h-full z-10 absolute left-0 top-0 flex flex-col justify-center items-center">
      <div class="ml-4 absolute left-0 top-48 text-5xl flex text-nowrap">
        <enhanced:img src="$assets/end-texts-your.png" class="h-10" alt="" />: {gameState?.points?.me?.reduce(
          (a, b) => a + b,
          0,
        )}
      </div>
      <div class="ml-4 absolute left-0 top-60 text-5xl flex text-nowrap">
        <enhanced:img src="$assets/end-texts-opponent.png" alt="" class="h-10" />: {gameState?.points?.other?.reduce(
          (a, b) => a + b,
          0,
        )}
      </div>
      <div class="ml-4 absolute left-0 top-72 text-5xl flex text-nowrap">
        <enhanced:img src="$assets/end-texts-total.png" class="h-10" alt="" />: {gameState?.seq}
      </div>
      <button
        class="mt-auto mx-auto mb-10"
        onclick={() => {
          resetChat();
          dialog.close();
        }}
      >
        <enhanced:img src="$assets/start-again.png" alt="" class="hover:brightness-110" />
      </button>
    </div>
  </div>
</dialog>

<dialog bind:this={disconnectedDialog} class=" bg-transparent text-white">
  Your opponent disconnected, Please start a new game.
  <button
    onclick={() => {
      resetChat();
      disconnectedDialog.close();
    }}>Return to start</button
  >
</dialog>

<dialog bind:this={waitingDialog} class="bg-transparent text-white outline-none">
  <div class="flex flex-col z-50 min-w-60 min-h-20 bg-blue-300 text-center p-4 text-3xl">
    Waiting for a opponent to join.
    <enhanced:img src="$assets/waiting.png" alt="" class="mx-auto h-24 w-auto" />
    {#if queueId}
      Queue <a href="{location.origin}/#{queueId}" onclick={() => window.navigator.clipboard.writeText(queueId ?? "")}
        >{queueId}</a
      >
    {/if}
  </div>
</dialog>

<dialog bind:this={kickedDialog} class=" bg-transparent text-white outline-none">
  You were kicked from the game. Reason: {status}
</dialog>

<dialog bind:this={userDialog} class=" bg-transparent text-white outline-none w-[40rem] h-[70rem]">
  <enhanced:img src="$assets/options.png" alt="" class=" absolute left-0 top-0 h-full w-full aspect-[2/5]" />

  <div class=" absolute px-16 flex flex-col h-full w-full pt-60 pb-10">
    <label class="text-5xl">
      Name:
      <input
        bind:value={name}
        maxlength="7"
        class="p-2 bg-cover bg-center bg-no-repeat h-12 w-64 text-5xl bg-transparent text-white outline-none"
        style:background-image="url({nameImg})"
      />
    </label>

    <button
      onclick={() => {
        navigator.clipboard.writeText(localStorage.getItem("userInfo")!);
        alert("Copied to clipboard make sure to save this somewhere safe");
      }}>Copy user info</button
    >
    <button
      onclick={() => {
        let info = prompt("Paste user info here");
        if (info) {
          localStorage.setItem("userInfo", info);
          userDialog.close();
        }
      }}>Login from user info</button
    >

    <button
      class="mt-auto mx-auto"
      onclick={async () => {
        let minLength = 3;
        let maxLength = 7;
        if (name.length < minLength || name.length > maxLength) {
          alert("Name must be between 3 and 7 characters");
          return;
        }
        console.log("Name is", name);
        let keys = localStorage.getItem("userInfo");
        if (!keys) {
          alert("You need to sign in first, play a game or insert private and public keys");
          return;
        }
        let json = JSON.parse(keys);
        let res = await fetch(`${backendUrl}/set_name`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            name: name,
            pub_key: json.pub_key,
            signature: sign_message(json.priv_key, name),
          }),
        });
        if (res.ok) {
          userDialog.close();
        }
      }}
    >
      <enhanced:img src="$assets/save.png" alt="" class="brightness-125 hover:brightness-110 w-72" />
    </button>
  </div>
  <button
    class=" absolute right-0 top-0"
    onclick={() => {
      userDialog.close();
    }}
  >
    <enhanced:img src="$assets/close.png" alt="" class=" size-12 hover:brightness-110 rounded-xl" />
  </button>
</dialog>

{#if queueId && !ws && !gameInfo}
  <dialog class="z-50">
    <img
      class="hidden"
      onerror={function () {
        this.parentNode.showModal();
      }}
      src="iamlazy zzzzzz"
    />
    <div class="flex flex-col min-w-60 min-h-20 bg-blue-300 text-center p-4 text-3xl">
      <span class="text-2xl">Join Match?</span>
      <span class="text-xl">
        Queue <a href="{location.origin}/#{queueId}" onclick={() => window.navigator.clipboard.writeText(queueId ?? "")}
          >{queueId}</a
        >
      </span>
      <button
        onclick={() => {
          startChat();
        }}
      >
        Join
      </button>
    </div>
  </dialog>
{/if}

{#if gameInfo}
  {#if status}
    <div class="absolute text-center text-2xl w-full h-full top-20 z-20">
      <span class="bg-blue text-red-900 p-3 rounded-xl">{status}</span>
    </div>
  {/if}

  <div class="flex gap-4">
    <div class="ml-auto flex mb-auto">
      <div class="flex flex-col mb-auto">
        <div class="">
          <enhanced:img src="$assets/turns-your.png" alt="" style:display={gameState?.your_turn ? "block" : "none"} />
          <enhanced:img src="$assets/turns-other.png" alt="" style:display={!gameState?.your_turn ? "block" : "none"} />

          <button
            class="mx-auto mb-10 my-4"
            onclick={() => {
              const sending = game.w_forfeit();
              peerConnection.send(sending);

              gameState = game.w_get_board_data();
            }}
          >
            <enhanced:img src="$assets/turns-forfeit.png" alt="" class="hover:brightness-110" />
          </button>
        </div>
        {#each [1, 2, 3, 4, 5, 6] as i}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="size-28 mx-auto"
            draggable="true"
            style:display={gameState?.your_turn && gameState?.next_dice == i ? "block" : "none"}
            ondragstart={() => {
              console.log("Dragging");
            }}
            ondragend={() => {
              console.log("Dragging ended");
            }}
          >
            <Dice number={i} occurrences={0}></Dice>
          </div>
        {/each}
      </div>
    </div>
    <div class=" flex gap-8 flex-col mr-auto">
      <div class="grid grid-cols-3 gap-3 relative">
        {@render diceLayout(gameState?.decks.me, gameState?.points?.me, highLightsMine, (index: number) => {
          if (!gameState?.your_turn) {
            alert("Not your turn");
            return;
          }
          if (gameState?.is_completed) {
            alert("Game is over!");
            return;
          }
          let pos = index % boardSize.width;
          let error = game.w_test_place(pos);
          if (error) {
            alert(error);
            return;
          }
          const sending = game.w_place(index % boardSize.width);
          console.log("Sending Bytes", sending);
          peerConnection.send(sending);
          gameState = game.w_get_board_data();
        })}
      </div>
      <span class="text-2xl">Opponents layout: </span>
      <div class=" grid grid-cols-3 gap-3 mx-auto relative">
        {@render diceLayout(gameState?.decks.other, gameState?.points?.other, highLightsOther, (index: number) => {
          console.log("Tried clicking on other dice ", index);
        })}
      </div>
    </div>
  </div>
{:else}
  <div class="w-full h-full flex justify-center items-center relative rounded-xl text-white">
    <div class=" h-[40rem] w-[20rem] relative">
      <enhanced:img src="$assets/start-bg.png" alt="" class="absolute left-0 top-0 h-full w-full rounded-xl" />
      <div class=" absolute left-0 top-0 h-full w-full flex flex-col justify-center items-center p-4">
        <span class=" text-4xl text-center font-semibold">KnuckleBones</span>
        <a class=" text-center" href="https://tricked.dev">By Tricked</a>
        <span
          >Rules: <a
            class=" underline hover:text-red-700 duration-150"
            href="https://cult-of-the-lamb.fandom.com/wiki/Knucklebones">As seen in the Cult Of Lamb Wiki</a
          ></span
        >
        <details class=" w-full text-lg">
          <summary class="w-full">TL;DR</summary>
          <ul class="font-serif">
            <li>You get a dice and the more of the same dice you have in a row the more points you get</li>
            <li>All dices of the same number get removed from the other row if you place a dice</li>
          </ul>
        </details>
        <details class=" w-full text-lg">
          <summary class="w-full">Leaderboard</summary>
          <ul class=" font-serif max-h-80 overflow-y-scroll">
            {#each leaderboardData?.entries ?? [] as entry}
              <li>
                {entry.name}: {entry.total_points} points, {entry.total_games} games,
                {entry.total_wins} wins
              </li>
            {/each}
          </ul>
        </details>

        {@render nojs()}

        {#if !wasm}
          {@render nowasm()}
        {/if}

        <button onclick={startChat} class=" mt-auto hover:brightness-110">
          <enhanced:img src="$assets/start-btn.png" class="h-24 w-50" alt="" />
        </button>
        <div class="flex justify-center gap-2">
          <button
            class="h-16 w-30 relative hover:brightness-110"
            onclick={() => {
              queueId = prompt("Enter a queue id", queueId) ?? undefined;
              if (queueId == undefined) alert("Cancelled");
              else startChat();
            }}
          >
            <enhanced:img src="$assets/private-btns-bg.png" class="w-full h-full absolute left-0 top-0" />
            <enhanced:img src="$assets/private-btns-join.png" class="w-full h-full absolute left-0 top-0" />
          </button>
          <button
            class="h-16 w-30 relative hover:brightness-110"
            onclick={() => {
              queueId = genUUID();
              startChat();
            }}
          >
            <enhanced:img src="$assets/private-btns-bg.png" class="w-full h-full absolute left-0 top-0" />
            <enhanced:img src="$assets/private-btns-start.png" class="w-full h-full absolute left-0 top-0" />
          </button>
        </div>
      </div>
    </div>
  </div>
  <button
    class=" absolute top-2 right-2 hover:brightness-110"
    onclick={() => {
      userDialog.showModal();
    }}
  >
    <enhanced:img class="size-16" src="$assets/user.png" />
  </button>
{/if}

{#snippet nowasm()}
  <span>Your browser does not support WebAssembly, please use a modern browser</span>
  <a href="https://webassembly.org/" class="text-blue-700 underline hvoer:text-blue-900">What is wasm?</a>
{/snippet}

{#snippet nojs()}
  <noscript class="w-full">
    <span class="text-4xl text-teal-300">
      You need to enable JavaScript to run this app. otherwise enjoy the artwork
    </span>
    <div class="flex justify-center gap-4">
      <div class="size-12">
        <Dice number={1} occurrences={0}></Dice>
      </div>
      <div class="size-12">
        <Dice number={2} occurrences={2}></Dice>
      </div>
      <div class="size-12">
        <Dice number={3} occurrences={3}></Dice>
      </div>
    </div>
    <span class="flex justify-center text-8xl text-center w-full gap-2">
      <enhanced:img src="$assets/end-texts-your.png" />
      <span class="text-8xl text-center w-full">0</span>
    </span>
  </noscript>
{/snippet}
