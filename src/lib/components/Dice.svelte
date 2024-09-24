<script lang="ts">
  let {
    number,
    occurrences,
  }: {
    number: number;
    occurrences: number;
  } = $props();

  const dices = import.meta.glob("../../assets/dices-*.png", {
    eager: true,
    query: {
      enhanced: true,
    },
  });
  let color = $derived(occurrences == 2 ? "glow-yellow" : occurrences == 3 ? "glow-red" : "");
</script>

<div class="relative w-full h-full">
  {#if number != 0}
    <enhanced:img
      src="$assets/dices-base.png"
      alt="dice"
      class="absolute left-0 top-0 h-full w-full {color}"
      draggable="false"
    />
    <img
      src={dices[`../../assets/dices-${number}.png`].default.img.src}
      alt="dice"
      class="absolute left-0 top-0 h-full w-full"
      draggable="false"
    />
  {/if}
</div>

<style>
  .glow-yellow {
    filter: drop-shadow(0 0 10px yellow);
  }

  .glow-red {
    filter: drop-shadow(0 0 10px red);
  }
</style>
