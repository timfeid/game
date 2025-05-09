<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import type { CardSelectionDetails, GameState } from '@gangsta/rusty';
	import { RSPCError } from '@rspc/client';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { client } from '../../../client';
	import { selectFromCards } from '../../../stores/dialog';
	import * as Dialog from '../../ui/dialog';
	import CCard from '../card/playing-card.svelte';

	let cards: CardSelectionDetails | undefined = $state(undefined);
	let open = $state(true);

	// export let game: GameState;
	// export let code: string;
	interface Props {
		game: GameState;
		code: string;
	}
	const { game, code }: Props = $props();

	let timeout: NodeJS.Timeout;
	function cancel() {
		open = false;
	}

	onMount(() => {
		return selectFromCards.subscribe((incoming) => {
			if (incoming) {
				console.log('clearing trimeout');
				clearTimeout(timeout);
				open = true;
			}
			cards = incoming;
		});
	});

	async function selectCard(e: MouseEvent, index: number) {
		e.preventDefault();
		if (cards) {
			const target = cards.cards[index].frontend_target;
			try {
				timeout = setTimeout(() => (open = false), 300);
				await client.mutation(['lobby.respond.card_selection', { code, target: { Card: target } }]);
			} catch (e) {
				clearTimeout(timeout);
				if (e instanceof RSPCError) {
					return toast.error(e.message);
				}
				toast.error('Unknown error!');
			}
		}
	}

	async function clickedAbility(id: string) {
		try {
			timeout = setTimeout(() => (open = false), 300);
			await client.mutation(['lobby.respond.card_selection_button', { code, button_id: id }]);
		} catch (e) {
			clearTimeout(timeout);
			if (e instanceof RSPCError) {
				return toast.error(e.message);
			}
			toast.error('Unknown error!');
		}
	}

	$effect(() => {
		console.log(open);
	});
</script>

{#if cards}
	<Dialog.Root bind:open>
		<Dialog.Portal>
			<Dialog.Overlay />
			<Dialog.Content noClose class="!w-[90vw] !max-w-[1400px]">
				<Dialog.Title>Choose a card</Dialog.Title>
				<Dialog.Description>{@html cards.message.replace('\n', '<br>')}</Dialog.Description>

				<ul class="border rounded w-full overflow-x-auto flex space-x-2">
					{#each cards.cards as card, i}
						<li class="border-b px-2 last:border-b-0 py-2">
							<button onclick={(e) => selectCard(e, i)}>
								<CCard noTooltips class="pointer-events-none" cardWithDetails={card} {game}></CCard>
							</button>
						</li>
					{/each}
				</ul>

				<div class="flex w-full justify-end">
					{#each cards.buttons as button}
						<Button onclick={() => clickedAbility(button.id)}>{button.text}</Button>
					{/each}

					{#if !cards.selection_required}
						<Dialog.Close>
							<Button>Close</Button>
						</Dialog.Close>
					{/if}
				</div>
			</Dialog.Content>
		</Dialog.Portal>
	</Dialog.Root>
{/if}
