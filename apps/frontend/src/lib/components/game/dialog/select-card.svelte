<script lang="ts">
	import * as AlertDialog from '$lib/components/ui/alert-dialog/index.js';
	import { Button } from '$lib/components/ui/button/index.js';
	import type {
		AbilityDetails,
		Card,
		CardSelectionDetails,
		CardWithDetails,
		GameState
	} from '@gangsta/rusty';
	import { onMount } from 'svelte';
	import {
		selectedAbility,
		selectedCard,
		selectFromAbilities,
		selectFromCards
	} from '../../../stores/dialog';
	import Ability from '../card/ability.svelte';
	import CCard from '../Card.svelte';
	import { toast } from 'svelte-sonner';
	import { client } from '../../../client';
	import * as Dialog from '../../ui/dialog';
	import { RSPCError } from '@rspc/client';

	let cards: CardSelectionDetails | undefined;
	export let game: GameState;
	export let code: string;

	function cancel() {
		open = false;
	}

	onMount(() => {
		return selectFromCards.subscribe((incoming) => {
			if (incoming) {
				open = true;
			}
			cards = incoming;
		});
	});

	async function selectCard(index: number) {
		if (cards) {
			const target = cards.cards[index].frontend_target;
			try {
				await client.mutation(['lobby.respond.card_selection', { code, target: { Card: target } }]);
				open = false;
			} catch (e) {
				if (e instanceof RSPCError) {
					return toast.error(e.message);
				}
				toast.error('Unknown error!');
			}
		}
	}

	let open = true;
</script>

{#if cards}
	<Dialog.Root bind:open>
		<Dialog.Portal>
			<Dialog.Overlay />
			<Dialog.Content noClose class="!w-[90vw] !max-w-[1400px]">
				<Dialog.Title>Choose a card</Dialog.Title>

				<ul class="border rounded w-full overflow-x-auto flex space-x-2">
					{#each cards.cards as card, i}
						<li class="border-b px-2 last:border-b-0 py-2">
							<button on:click|stopPropagation={() => selectCard(i)}>
								<CCard noTooltips class="pointer-events-none" cardWithDetails={card} {game}></CCard>
							</button>
						</li>
					{/each}
				</ul>

				<div class="flex w-full justify-end">
					<Dialog.Close>
						<Button>Cancel</Button>
					</Dialog.Close>
				</div>
			</Dialog.Content>
		</Dialog.Portal>
	</Dialog.Root>
{/if}
