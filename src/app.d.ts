// See https://kit.svelte.dev/docs/types#app

import type { Game } from "./lib/game";

// for information about these interfaces
declare global {
  namespace App {
    // interface Error {}
    // interface Locals {}
    // interface PageData {}
    // interface PageState {}
    // interface Platform {}
  }
  var game: Game;
  var random_uuid: () => string;
}

export {};
