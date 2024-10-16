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
		if (!cards?.valid_card_indexes.includes(index)) {
			toast.error('invalid card selected?');
			return;
		}
		const target = cards.cards[index].frontend_target;
		await client.mutation(['lobby.respond_card_selection', { code, target: { Card: target } }]);
		open = false;
	}

	let open = true;
</script>

{#if cards}
	<AlertDialog.Root bind:open>
		<AlertDialog.Trigger asChild let:builder>
			<Button builders={[builder]} variant="outline">Show Dialog</Button>
		</AlertDialog.Trigger>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>Choose a card</AlertDialog.Title>
				<ul class="border rounded">
					{#each cards.cards as card, i}
						<li class="border-b px-2 last:border-b-0 py-2">
							<button on:click={() => selectCard(i)}>
								<CCard cardWithDetails={card} {game}></CCard>
							</button>
						</li>
					{/each}
				</ul>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Action on:click={cancel}>Cancel</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>
{/if}
