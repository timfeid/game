<script lang="ts">
	import { page } from '$app/stores';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import type { DeckDetails, DeckSelector, GameState } from '@gangsta/rusty';
	import { Circle } from 'lucide-svelte';
	import { client } from '../../client';
	import { user } from '../../stores/access-token';
	import Button from '../ui/button/button.svelte';
	import ManaBubble from './mana-bubble.svelte';
	import { onMount } from 'svelte';
	import { browser } from '$app/environment';
	import Card from './Card.svelte';

	export let game_state: GameState;
	export let join_code: string;

	let decks: DeckSelector[] = [];

	$: self = game_state.players[$user?.sub || ''];

	let deck: DeckSelector | undefined;
	$: deck = self.deck;

	let deckDetails: DeckDetails | undefined;

	$: if (game_state && browser && decks.length == 0) {
		client.query(['lobby.deck.list', join_code]).then((v) => (decks = v));
	}

	$: if (game_state && browser && deck) {
		client.query(['lobby.deck.cards', { deck, code: join_code }]).then((v) => (deckDetails = v));
	}

	$: totalCards =
		deckDetails?.cards.reduce((a, b) => {
			return a + b.count;
		}, 0) || 0;

	async function ready() {
		await client.mutation(['lobby.ready', join_code]);
	}

	async function setDeck(deck: DeckSelector) {
		await client.mutation(['lobby.deck.select', { deck, code: join_code }]);
	}
</script>

<div class="container !px-3">
	<div class="flex items-center h-24">
		<Button on:click={ready}>Ready up</Button>
	</div>
	{#if self.status === 'Spectator'}
		<DropdownMenu.Root>
			<DropdownMenu.Trigger asChild let:builder>
				<Button builders={[builder]} variant="outline" class="w-full flex justify-between">
					<div>Select deck</div>
					<div class="flex space-x-2 items-center">
						<div class="uppercase text-xs text-gray-400">
							{deck},
							{totalCards} cards
						</div>

						<ManaBubble color={deck} />
					</div>
				</Button>
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="start" class="w-56">
				{#each decks as deck}
					<DropdownMenu.Item on:click={() => setDeck(deck)}>
						<div
							class="w-3 h-3 rounded-full border-white border mr-2"
							class:bg-blue-400={deck === 'Angels'}
							class:bg-green-400={deck === 'Green'}
							class:bg-black={deck === 'Black'}
						></div>
						<span> {deck} </span>
					</DropdownMenu.Item>
				{/each}
			</DropdownMenu.Content>
		</DropdownMenu.Root>

		{#if deckDetails}
			<div class="flex flex-wrap gap-4 mt-4">
				{#each deckDetails.cards as card}
					<div class="relative">
						<!-- Render the card multiple times, up to a maximum of 4 -->
						{#each Array(Math.min(card.count, 4)) as _, index}
							{#if index === 0}
								<Card noTooltips cardWithDetails={card.card} class="z-10 absolute" />
							{:else}
								<div
									class="absolute"
									style="z-index: {4 - index}; top: {index * 4}px; left: -{index * 4}px;"
								>
									<Card noTooltips cardWithDetails={card.card} />
								</div>
							{/if}
						{/each}
						{#if card.count > 4}
							<div
								class="z-20 absolute bottom-0 right-0 w-full h-full flex items-center justify-center text-white font-bold"
							>
								+{card.count - 4}
							</div>
						{/if}
					</div>
				{/each}
			</div>
		{/if}
	{:else}
		<div class="mt-4">
			looks like you're {self.status}
		</div>
	{/if}
	{#if $page.url.searchParams.has('debug')}
		<pre>{JSON.stringify(self)}</pre>
	{/if}
</div>
