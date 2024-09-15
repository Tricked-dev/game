<script lang="ts">
  import { onMount } from "svelte";
import { Game, sign_message, type BoardData ,type GameBody, type LeaderBoard} from "./lib/wasmdev/lib_knuckle";

const boardSize = {
	width: 3,
	height: 3,
};
let backendUrl = import.meta.env.DEV ? "http://localhost:8083" : '';

let game: Game;
let gameInfo: Partial<GameBody> & {initiator: boolean} & {[key: string]: string} = $state(null!);
let gameState: BoardData = $state(null!);

let ws: WebSocket;
let peerConnection: RTCPeerConnection;
let dataChannel: RTCDataChannel;
let dialog: HTMLDialogElement = $state(null!);
let disconnectedDialog: HTMLDialogElement = $state(null!);
let waitingDialog: HTMLDialogElement = $state(null!);
let kickedDialog: HTMLDialogElement = $state(null!);


let status: string | undefined = $state();

let pub_key: string;
let priv_key: string;

function startChat() {
	waitingDialog.showModal();
	status = "Starting Connection";

	if (import.meta.env.DEV) {
		ws = new WebSocket("ws://localhost:8083/ws");
	} else {
		ws = new WebSocket(`${window.origin.replace("http", "ws")}/ws`);
	}

	ws.onopen = () => {};

	ws.onmessage = async (event) => {
		let message:Record<string, any>;

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
				let json = userInfo ? JSON.parse(userInfo) : await fetch(`${backendUrl}/signup`).then(r => r.json());
                localStorage.setItem("userInfo", JSON.stringify(json));
				const private_key = json.priv_key;
				const response = await sign_message(private_key, message.verify_time);
				ws.send(
					JSON.stringify({
						type: "join",
						signature: response,
						pub_key: json.pub_key,
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
				initializePeerConnection(message.initiator);

				gameInfo = message;

				window.game = game;

				// ws.close()
				break;
			case "offer":
				await handleOffer(message as {offer:RTCSessionDescriptionInit});
				break;
			case "answer":
				await peerConnection.setRemoteDescription(
					new RTCSessionDescription(message.answer),
				);
				break;
			case "ice-candidate":
				await peerConnection.addIceCandidate(
					new RTCIceCandidate(message.candidate),
				);
				break;
			case "disconnected":
				// displayMessage("Your chat partner disconnected.");
				// resetChat();
				break;
			case "disconnect":
				waitingDialog.close();
				kickedDialog.showModal();
				break;
		}
	};

	ws.onclose = () => {
		if (gameState !== undefined) return;
		waitingDialog.close();
		kickedDialog.showModal();
	};
}
async function initializePeerConnection(isInitiator:boolean) {
	console.log("INit peer connection");
	peerConnection = new RTCPeerConnection({
		iceServers: [
			{
				urls: [
					"stun:stun.l.google.com:19302",
					"stun:stun1.l.google.com:19302",
					"stun:stun2.l.google.com:19302",
				],
			},
		],
	});

	peerConnection.onicecandidate = (event) => {
		const iceStatus = "Getting further (ICE Candidate)";
		status = iceStatus;
		console.log("Sending icecandidate!!!");
		console.log(event);
		if (event.candidate) {
			ws.send(
				JSON.stringify({
					type: "ice-candidate",
					candidate: event.candidate,
				}),
			);
		}

		setTimeout(async () => {
			if (status === iceStatus) {
				status = "Seems like this is taking a bit too long, requeing";
				await new Promise((resolve) => setTimeout(resolve, 1500));
				resetChat();
				startChat();
			}
		}, 5000);
	};
	window.pc = peerConnection;
	peerConnection.addEventListener("connectionstatechange", (event) => {
		console.log("connectionstatechange", event);
	});
	if (isInitiator) {
		console.log("Creating datachannel");
		dataChannel = peerConnection.createDataChannel("chat");
		setupDataChannel();

		const offer = await peerConnection.createOffer();
		console.log("Offer", offer);
		const answer = await peerConnection.setLocalDescription(offer);

		ws.send(
			JSON.stringify({
				type: "offer",
				offer: peerConnection.localDescription,
			}),
		);
	} else {
		peerConnection.ondatachannel = (event) => {
			dataChannel = event.channel;
			setupDataChannel();
		};
	}
}

async function handleOffer(message: {offer:RTCSessionDescriptionInit}) {
	await peerConnection.setRemoteDescription(
		new RTCSessionDescription(message.offer),
	);
	const answer = await peerConnection.createAnswer();
	await peerConnection.setLocalDescription(answer);
	ws.send(
		JSON.stringify({
			type: "answer",
			answer: peerConnection.localDescription,
		}),
	);
}

function setupDataChannel() {
	dataChannel.onopen = () => {
		ws.close();
		status = undefined;
	};

	dataChannel.onclose = () => {
		if (gameState === undefined || gameState?.is_completed) return;
		status = "Connection closed";
		disconnectedDialog.showModal();
		console.log("Datachannel closed");
	};

	dataChannel.onmessage = (event) => {
		let data = new Uint8Array(event.data);
		console.log(event);
		console.log("Received Data", data);
		console.log(game.w_add_opponent_move(data));
		gameState = game.w_get_board_data();
	};
}

function resetChat() {
	if (gameState.decks) {
		gameState.decks.me = undefined!;
		gameState.decks.other = undefined!;
	}
	if (gameState) {
		gameState.decks = undefined!;
		gameState.points = undefined!;
	}
	try {
		game.free();
	} catch (e) {
		// oh well
	}

	gameState = undefined!;
	gameInfo = undefined!;
	try {
		dataChannel.close();
	} catch (e) {}
	try {
		peerConnection.close();
	} catch (e) {}
	peerConnection = null!;
	dataChannel = null!;
}
$inspect(gameInfo);

$effect(() => {
	if (gameState?.is_completed) {
		dialog.showModal();
		(async () => {
			const body: GameBody = {
				seed: gameInfo!.seed! ,
				time: gameInfo!.time! ,
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


let leaderboardData:LeaderBoard = $state(null!);

onMount(async () => {
	leaderboardData = await fetch(`${backendUrl}/leaderboard`).then(r => r.json());
});

</script>

<svelte:options runes={true} ></svelte:options>

{#snippet dice(number: number)}
    <div class="relative w-full h-full">
        {#if number != 0}
            <img src="/assets/dices-base.png" alt="dice" class="absolute left-0 top-0 h-full w-full" draggable="false">
            <img src="/assets/dices-{number}.png" alt="dice" class="absolute left-0 top-0 h-full w-full"
            draggable="false"
            >
        {/if}
    </div>
{/snippet}

{#snippet diceLayout(deck: number[], points: number[], onclick: any)}
  {#each deck ?? [] as row, index}
    <button
        class="size-28 flex justify-center text-center text-3xl p-4"
        onclick={() => {
            console.log("Dropped")
            onclick(index)}}
        ondrop={() => onclick(index)}
        ondragover={(event) => {

          event.preventDefault();
    event.dataTransfer!.dropEffect = 'copy';}}

        style:background-image="url(/assets/dice-bg.png)"
        style:background-size="cover"
        >

        {@render dice(row)}
        <!-- <svelte:component this={icons[row]} /> -->
    </button>
  {/each}
  {#each points ?? [] as row}
  <div class="flex justify-center">
    <div
        class="size-20 flex justify-center text-white text-center text-3xl"
        style:background-image="url(/assets/number-base.png)"
        style:background-size="cover"

        >
        <div class="m-auto">
        {row}

        </div>
    </div>
  </div>

  {/each}
{/snippet}

<!-- <button onclick={() => dialog.showModal()}>End</button> -->

<dialog bind:this={dialog} class="bg-transparent text-white">
    <div class="flex flex-col h-[50rem] w-[20rem]">
    <img src="/assets/ending-base.png" alt="" class="absolute left-0 top-0 h-full w-full aspect-[2/5}">
    {#if gameState?.winner.winner}
        <img src="/assets/ending-win.png" alt="" class="absolute left-0 top-0 h-full w-full aspect-[2/5}">
    {:else if gameState?.winner?.win_by_tie}
        <img src="/assets/ending-draw.png" alt="" class="absolute left-0 top-0 h-full w-full aspect-[2/5}">
    {:else}
        <img src="/assets/ending-lose.png" alt="" class="absolute left-0 top-0 h-full w-full aspect-[2/5}">
    {/if}
    <div class="w-full h-full z-10 absolute left-0 top-0 flex flex-col justify-center items-center">
        <div class="ml-4 absolute left-0 top-48 text-5xl font-bold flex text-nowrap">
            <img src="/assets/end-texts-your.png" class="h-10" alt="">: {gameState?.points?.me?.reduce((a, b) => a + b, 0)}
        </div>
        <div class="ml-4 absolute left-0 top-60 text-5xl font-bold flex text-nowrap">
          <img src="/assets/end-texts-opponent.png" alt="" class="h-10">:  {gameState?.points?.other?.reduce((a, b) => a + b, 0)}
        </div>
        <div class="ml-4 absolute left-0 top-72 text-5xl font-bold flex terxt-nowrap">
           <img src="/assets/end-texts-total.png" class="h-10" alt="">: {gameState?.seq}
        </div>
    <button class="mt-auto mx-auto mb-10" onclick={() => {
       resetChat();
       dialog.close();
    }}>
    <img src="/assets/start-again.png" alt="" >
    </button>
    </div>

    </div>

</dialog>

<dialog bind:this={disconnectedDialog}>
    Your opponent disconnected, Please start a new game.
    <button onclick={() => {
       resetChat();
       disconnectedDialog.close();
    }}>Return to start</button>
</dialog>

<dialog bind:this={waitingDialog} class="bg-transparent text-white outline-none" >
	<div class="flex flex-col z-50">
    Waiting for a opponent to join.
	<img src="/assets/waiting.png" alt="" class="" >
	</div>
</dialog>

<dialog bind:this={kickedDialog} class="bg-transparent text-white outline-none" >
    You were kicked from the game. This is probably because you joined the queue on another device/tab
</dialog>

{#if gameInfo}



{#if status}
    <span>{status}</span>
{:else}

<div class="flex gap-4 mx-auto">
    <div class="ml-auto">
        <!-- Next dice {gameState?.next_dice}<br /> -->
        <!-- Seq {gameState?.seq}<br /> -->
        <!-- Starting: {gameInfo?.initiator}<br /> -->
        <!-- Your id {gameInfo?.public_key?.slice(0,5)}<br/> -->
        <!-- Partner id {gameInfo?.partner_key?.slice(0,5)}<br/> -->

        <div class="flex flex-col gap-4 justify-center">
 {#if gameState?.your_turn}
        <img src="/assets/turns-your.png" alt="">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="size-28 mx-auto " draggable="true" ondragstart={() => {
            console.log("Dragging")
        }}
    ondragend={() => {
        console.log("Dragging ended")
    }}
         >
        {@render dice(gameState?.next_dice)}

        </div>
        {:else}

        <img src="/assets/turns-other.png" alt="">
        {/if}
        </div>

        <button class="mt-auto mx-auto mb-10 mt-8" onclick={() => {
            const sending = game.w_forfeit();
            dataChannel.send(sending);

            gameState = game.w_get_board_data();
        }}>
        <img src="/assets/turns-forfeit.png" alt="" >
        </button>


    </div>
    <div class=" flex gap-8 flex-col mr-auto">
        <div class="grid grid-cols-3 gap-3 relative">
            {@render diceLayout(gameState?.decks.me, gameState?.points?.me, (index:number) => {
            if(!gameState?.your_turn) {alert("Not your turn");return}
            if(gameState?.is_completed) {alert("Game is over!");return}
            let pos = index % boardSize.width;
            let error = game.w_test_place(pos);
            if(error) {
                alert(error);
                return;
            }
            const sending = game.w_place(index % boardSize.width);
            console.log("Sending Bytes", sending)
            dataChannel.send(sending);
            gameState = game.w_get_board_data();
            })}

        </div>
        <span class="text-1xl font-semibold">Opponents layout: </span>
        <div class="grid grid-cols-3 gap-3 mx-auto relative">
            {@render diceLayout(gameState?.decks.other, gameState?.points?.other, (index:number) => {
            console.log("Tried clicking on other dice ", index)
            })}
        </div>
    </div>
</div>
{/if}

{:else}
<div class="w-full h-full flex justify-center items-center relative rounded-xl text-white">
    <div class="h-[40rem] w-[20rem] relative">
        <img src="/assets/start-bg.png" alt="" class="absolute left-0 top-0 h-full w-full rounded-xl">
        <div class="absolute left-0 top-0 h-full w-full flex flex-col justify-center items-center p-4">
            <span class="text-4xl text-center font-semibold">KnuckleBones</span>
            <span class="text-center">By Tricked</span>
            <span>Rules: <a class="underline hover:text-red-700 duration-150" href="https://cult-of-the-lamb.fandom.com/wiki/Knucklebones">As seen in the Cult Of Lamb Wiki</a></span>
            <details class="w-full text-lg">
                <summary class="w-full">TL;DR</summary>
                <ul class="font-serif">
                    <li>You get a dice and the more of the same dice you have in a row the more points you get</li>
                    <li>All dices of the same number get removed from the other row if you place a dice</li>
                </ul>
            </details>
			<details class="w-full text-lg">
				<summary class="w-full">Leaderboard</summary>
				<ul class="font-serif max-h-80 overflow-y-scroll">
					{#each (leaderboardData?.entries ?? []) as entry}
						<li>{entry.name}: {entry.total_points} points, {entry.total_games} games, {entry.total_wins} wins</li>
					{/each}
				</ul>
			</details>

            <button onclick={startChat} class="mt-auto">
                <img src="/assets/start-btn.png" class="h-24" alt="">
            </button>
        </div>

    </div>
</div>
{/if}