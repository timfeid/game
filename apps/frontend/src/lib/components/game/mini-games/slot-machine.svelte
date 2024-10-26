<script lang="ts">
	import { tweened } from 'svelte/motion';
	import { cubicOut } from 'svelte/easing';
	import type { SlotMachineResult } from '@gangsta/rusty';
	import { onMount } from 'svelte';

	export let result: SlotMachineResult;

	const SYMBOLS: string[] = result.symbols_used;

	let reels: string[] = ['🍒', '🍋', '🍊'];
	let message: string = 'Good luck!';
	let spinning = false;
	const reelAnimations = [
		tweened(0, { duration: 1000, easing: cubicOut }),
		tweened(0, { duration: 1000, easing: cubicOut }),
		tweened(0, { duration: 1000, easing: cubicOut })
	];

	function spin(): void {
		if (spinning) {
			message = 'Already spinning!';
			return;
		}

		spinning = true;
		message = 'Spinning...';

		reelAnimations.forEach((reel, index) => {
			const steps = 10; // Number of fake spins before landing on result
			const targetIndex = SYMBOLS.indexOf(result.reels[index]);
			const randomSpinIndexes = Array.from({ length: steps }, () =>
				Math.floor(Math.random() * SYMBOLS.length)
			);
			const finalSpinIndexes = [...randomSpinIndexes, targetIndex];

			let delay = 0;
			finalSpinIndexes.forEach((symbolIndex) => {
				setTimeout(() => {
					reels[index] = SYMBOLS[symbolIndex];
					reel.set(symbolIndex * (100 / SYMBOLS.length)); // Update tweened value to create smooth animation
				}, delay);
				delay += 100; // Speed of spin animation for each step
			});
		});

		console.log(result);
		setTimeout(() => {
			spinning = false;
			message = result.result;
		}, 1000);
	}

	onMount(spin);
</script>

<div class="flex justify-center mb-4">
	{#each reels as symbol, index}
		<div class="text-6xl mx-2 bg-primary/10 rounded-lg p-4">
			<span style="display: block; transform: translateY({reelAnimations[index]}%);">{symbol}</span>
		</div>
	{/each}
</div>
<div class="text-center mb-4">
	<p class="text-lg">{message}</p>
</div>

<style>
	.text-6xl {
		font-size: 4rem;
	}
</style>
