<script lang="ts">
	import type { GameState } from '@gangsta/rusty';
	import * as Dialog from '../../ui/dialog';
	import SlotMachine from '../mini-games/slot-machine.svelte';
	import { onMount } from 'svelte';
	import { showSlotMachine, showStockTicker } from '../../../stores/dialog';
	import { Button } from '../../ui/button';
	import StockTicker from '../mini-games/stock-ticker.svelte';

	export let game: GameState;
	export let code: string;

	let open = false;
	onMount(() => {
		return showStockTicker.subscribe((incoming) => {
			if (incoming) {
				open = true;
			}
		});
	});
	onMount(() => {
		return showSlotMachine.subscribe((incoming) => {
			if (incoming) {
				open = true;
			}
		});
	});
</script>

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay />
		<Dialog.Content noClose class="!w-[90vw] !max-w-[420px]">
			<div class="pt-4">
				{#if $showSlotMachine}
					<SlotMachine result={$showSlotMachine} />
				{/if}
				{#if $showStockTicker}
					<div class="w-[380px] overflow-hidden">
						<StockTicker />
					</div>
				{/if}
			</div>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>
