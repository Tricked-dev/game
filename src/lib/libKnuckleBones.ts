import { arrayBufferToBase64, base64ToArrayBuffer } from "./util.ts";

import { xorshift } from "./rng.ts";

type HistoryItem = {
  seq: number;
  now: number;
  x: number;
  signature: string;
};

type ServerGameInfo = {
  seed: number;
  starter: number;
  signature: string;
};

const keyType = "Ed25519";

export class Game {
  private history: HistoryItem[] = [];

  private rng: ReturnType<typeof xorshift>;

  private deck: number[];
  private otherDeck: number[];
  private seq = 0;
  private nextDice = 0;

  constructor(
    private myKeys: CryptoKeyPair,
    private otherKeys: Omit<CryptoKeyPair, "privateKey">,
    private deckSize: {
      height: number;
      width: number;
    },
    private info: ServerGameInfo,
    private id: number
  ) {
    this.rng = xorshift(info.seed);
    this.deck = this.createDeck();
    this.otherDeck = this.createDeck();
    this.nextDice = this.rng();
  }

  private static encodeHistoryItem(item: Omit<HistoryItem, "signature">) {
    return new TextEncoder().encode(`${item.seq}:${item.now}:${item.x}`);
  }

  private createDeck() {
    return new Array(this.deckSize.height * this.deckSize.width).fill(0);
  }

  public async addOpponentMove(data: HistoryItem) {
    this.seq++;
    this.history.push(data);
    await this.playMove(data);
  }
  public async place(x: number) {
    this.seq++;
    let now = Date.now();
    const data = {
      seq: this.seq,
      now,
      x,
    };
    const toSign = Game.encodeHistoryItem(data);
    let signature = await crypto.subtle.sign(
      keyType,
      this.myKeys.privateKey,
      toSign
    );
    const item: HistoryItem = {
      ...data,
      signature: arrayBufferToBase64(signature),
    };

    this.history.push(item);
    await this.playMove(item);
    return item;
  }

  private async playMove(item: HistoryItem) {
    let player = this.seq % 2;

    let meFirst = this.info.starter === this.id;

    let publicKey: CryptoKey;
    let deck: number[];
    let otherDeck: number[];
    if ((meFirst && player == 1) || (!meFirst && player == 0)) {
      publicKey = this.myKeys.publicKey;
      deck = this.deck;
      otherDeck = this.otherDeck;
    } else {
      publicKey = this.otherKeys.publicKey;
      deck = this.otherDeck;
      otherDeck = this.deck;
    }

    const toVerify = Game.encodeHistoryItem(item);
    const result = await crypto.subtle.verify(
      keyType,
      publicKey,
      base64ToArrayBuffer(item.signature),
      toVerify
    );
    if (!result) {
      throw new Error("Invalid signature");
    }

    this.history.push(item);

    let itemY = 0;
    for (let i = 0; i < this.deckSize.height; i++) {
      if (deck[item.x + i * this.deckSize.width] === 0) {
        itemY = i;
        break;
      }
    }
    const pos = item.x + itemY * this.deckSize.width;
    if (deck[pos] !== 0) {
      throw new Error(
        `Collision deck at ${item.x},${itemY} already has a ${deck[pos]}, player ${player}`
      );
    }
    let num = this.nextDice;
    deck[item.x + itemY * this.deckSize.width] = num;
    for (let i = 0; i < this.deckSize.height; i++) {
      let pos = item.x + i * this.deckSize.width;
      if (otherDeck[pos] === num) {
        otherDeck[pos] = 0;
        break;
      }
    }
    this.nextDice = this.rng();
  }

  public getBoardData() {
    return {
      points: {
        me: calculateKnucklebonesPoints(this.deck, this.deckSize.height),
        other: calculateKnucklebonesPoints(
          this.otherDeck,
          this.deckSize.height
        ),
      },
      decks: {
        me: this.deck,
        other: this.otherDeck,
      },
      history: this.history,
      seq: this.seq,
      deckSize: this.deckSize,
      nextDice: this.nextDice,
    };
  }
}

function calculateKnucklebonesPoints(board: number[], height: number) {
  // The multiplication table for reference
  const multiplicationTable = {
    1: [1, 4, 9],
    2: [2, 8, 18],
    3: [3, 12, 27],
    4: [4, 16, 36],
    5: [5, 20, 45],
    6: [6, 24, 54],
  } as Record<string, number[]>;

  // get straight columns from the board
  const columns = [];
  for (let i = 0; i < height; i++) {
    const column = [];
    for (let j = 0; j < board.length; j += height) {
      column.push(board[j + i]);
    }
    columns.push(column);
  }
  let results = [];
  const countOccurrences = (arr: number[]) => {
    return arr.reduce((acc, item) => {
      acc[item] = (acc[item] || 0) + 1;
      return acc;
    }, {} as Record<string, number>);
  };
  for (const column of columns) {
    let total = 0;
    let occ = countOccurrences(column);

    for (const [key, value] of Object.entries(occ)) {
      total += multiplicationTable[key]?.[value - 1] ?? 0;
    }

    results.push(total);
  }

  return results;
}
