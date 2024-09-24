<script>
  export const prerender = true;
  import Game from "../Game.svelte";
  import "../global.css";
  import "@fontsource/pixelify-sans";
  import "virtual:uno.css";
  import "@unocss/reset/normalize.css";
  import "@unocss/reset/sanitize/sanitize.css";
  import "@unocss/reset/sanitize/assets.css";
  import "@unocss/reset/eric-meyer.css";
  import "@unocss/reset/tailwind.css";

  const site = `https://knucklebones.fyi/`;

  const description =
    "Knucklebones is an exciting 1v1 dice game where players your goal is to get the most points in your 3x3 grid while also trying to get the other players points by placing you dice in rows they have the same dice";
  const image = `${site}og/og.png`;
</script>

<svelte:head>
  <meta charset="utf-8" />
  <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
  <meta name="viewport" content="width=device-width" />
  <title>Knucklebones - 1v1 Dice Game</title>
  <meta name="description" content={description} />
  <meta name="theme-color" content="#834d35" />

  <!-- Open Graph Meta Tags -->
  <meta property="og:title" content="Knucklebones - 1v1 Dice Game" />
  <meta property="og:description" content={description} />
  <meta property="og:image" content={image} />
  <meta property="og:url" content={site} />
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="Knucklebones" />
  <meta property="og:image:width" content="1200" />
  <meta property="og:image:height" content="675" />
  <meta property="og:color" content="#834d35" />

  <!-- Twitter Card Meta Tags -->
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content="Knucklebones - 1v1 Dice Game" />
  <meta name="twitter:description" content={description} />
  <meta name="twitter:image" content={image} />
  <meta name="twitter:image:alt" content="Knucklebones game screenshot" />

  <!-- Discord Metadata (via Open Graph) -->
  <meta property="og:locale" content="en_US" />
</svelte:head>

<Game />
