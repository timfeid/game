<script lang="ts">
	import { page } from '$app/stores';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import type { DeckSelector, GameState } from '@gangsta/rusty';
	import { Circle } from 'lucide-svelte';
	import { client } from '../../client';
	import { user } from '../../stores/access-token';
	import Button from '../ui/button/button.svelte';
	import ManaBubble from './mana-bubble.svelte';

	export let game_state: GameState;
	export let join_code: string;

	$: self = game_state.players[$user?.sub || ''];

	let deck: DeckSelector | undefined;
	$: deck = self.deck;

	async function ready() {
		await client.mutation(['lobby.ready', join_code]);
	}

	async function setDeck(deck: DeckSelector) {
		await client.mutation(['lobby.select_deck', { deck, code: join_code }]);
	}
</script>

<div class="container !px-3">
	<div class="flex items-center h-24">
		<Button on:click={ready}>Ready up</Button>
	</div>
	{#if self.status === 'Spectator'}
		<DropdownMenu.Root>
			<DropdownMenu.Trigger asChild let:builder>
				<Button builders={[builder]} variant="outline" class="w-[220px] flex justify-between">
					<div>Select deck</div>
					<div class="flex space-x-2 items-center">
						<div class="uppercase text-xs text-gray-400">
							{deck}
						</div>

						<ManaBubble color={deck} />
					</div>
				</Button>
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end" class="w-56">
				<DropdownMenu.Item on:click={() => setDeck('Green')}>
					<div
						class="w-3 h-3 rounded-full border-green-400 border mr-2"
						class:bg-green-400={deck === 'Green'}
					></div>
					<span> Green </span>
				</DropdownMenu.Item>
				<DropdownMenu.Item on:click={() => setDeck('Blue')}>
					<div
						class="w-3 h-3 rounded-full border-blue-400 border mr-2"
						class:bg-blue-400={deck === 'Blue'}
					></div>
					<span>Blue</span>
				</DropdownMenu.Item>
				<DropdownMenu.Item on:click={() => setDeck('Black')}>
					<div
						class="w-3 h-3 rounded-full border-black border mr-2"
						class:bg-black={deck === 'Black'}
					></div>
					<span>Black</span>
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	{:else}
		<div class="mt-4">
			looks like you're {self.status}
		</div>
	{/if}
	{#if $page.url.searchParams.has('debug')}
		<pre>{JSON.stringify(self)}</pre>
	{/if}
</div>
