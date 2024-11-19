<script lang="ts">
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card';
	import type { GameState, PlayerState } from '@gangsta/rusty';
	import { RectangleVertical } from 'lucide-svelte';
	import HeartPulse from 'lucide-svelte/icons/heart-pulse';
	import Clover from 'lucide-svelte/icons/clover';
	import { user } from '../../stores/access-token';
	import PlayingCardWithAttachments from './card/playing-card-with-attachments.svelte';
	import CCard from './card/playing-card.svelte';
	import { searchingForTarget, target } from './game';
	import ManaBubble from './mana-bubble/mana-bubble.svelte';
	import { Tooltip, TooltipContent, TooltipTrigger } from '../ui/tooltip';
	import { onMount } from 'svelte';

	type Props = {
		game: GameState;
		code: string;
		playerName: string;
		player: PlayerState;
	};

	let { game, code, playerName, player }: Props = $props();
	let showBattlefieldTargets = $state(false);

	async function setPlayerTarget(player: PlayerState) {
		target.set({ Player: player.sub });
	}

	function chooseBattlefield(battlefield: 'FrontlineBattlefield' | 'BacklineBattlefield') {
		target.set(battlefield);
	}

	onMount(() => {
		const unsubscribe = searchingForTarget.subscribe((target) => {
			if (player.sub !== $user?.sub) {
				return;
			}
			showBattlefieldTargets = false;
			console.log('hello', target);
			if (target && target === 'ChosenBattlefield') {
				console.log('hello');
				showBattlefieldTargets = true;
				return;
			}

			if (target === 'BacklineBattlefield' || target === 'FrontlineBattlefield') {
				return chooseBattlefield(target);
			}
		});
		return () => unsubscribe();
	});
</script>

<div class="flex-grow container h-full flex">
	<Card class="bg-transparent border-0 p-0 flex-grow flex flex-col">
		<CardHeader class="space-y-1">
			<CardTitle class="text-2xl font-bold text-center flex items-center">
				<button data-player={player.sub} onclick={() => setPlayerTarget(player)}>
					<div class="mr-4 flex items-center space-x-1">
						<div>
							{playerName}
						</div>
						<sub class="text-xs">
							{#if playerName === $user?.sub}(it's you){/if}
						</sub>
					</div>
				</button>
				<div class="flex space-x-2 items-center"></div>
				<div class="ml-auto flex flex-col">
					<div class="flex items-center">
						{#each { length: player.public_info.hand_size } as _}
							<RectangleVertical />
						{/each}
						<div class="flex space-x-2 items-center ml-3">
							<HeartPulse class="mr-2" />
							{player.public_info.health}
						</div>
					</div>
					{#if Object.values(player.public_info.mana_pool).find((x) => x !== true && x > 0)}
						<div class="ml-auto flex items-center space-x-2">
							<div class="text-xs uppercase">Mana pool</div>
							{#each Object.keys(player.public_info.mana_pool) as key}
								{@const val = player.public_info.mana_pool[key]}
								{#if Number.isInteger(val)}
									{#each { length: val } as _, i}
										<ManaBubble color={key} />
									{/each}
								{/if}
							{/each}
						</div>
					{/if}
				</div>
			</CardTitle>
		</CardHeader>
		<CardContent
			class="flex flex-col {player.player_index === 0 ? '' : 'flex-col-reverse'} flex-grow"
		>
			<div class="py-4 flex flex-wrap gap-2 flex-grow">
				{#each player.public_info.cards_in_play as card, i}
					{#if card.position === 'Frontline' && card.attached_to === null}
						<PlayingCardWithAttachments {game} cardWithDetails={card} />
					{/if}
				{/each}
				{#if showBattlefieldTargets}
					<button
						onclick={() => chooseBattlefield('FrontlineBattlefield')}
						class="flex flex-col text-xs card relative w-[215px] h-[215px] transition duration-300 font-serif justify-center items-center border rounded"
						>frontline</button
					>
				{/if}
			</div>
			<div
				class="py-4 flex flex-wrap gap-2 flex-grow relative {player.player_index === 0
					? ' border-t'
					: ' border-b'}"
			>
				<div
					class="absolute bg-background px-4 left-1/2 -translate-x-1/2 {player.player_index === 0
						? 'top-0 -translate-y-1/2'
						: 'bottom-0 translate-y-1/2'}"
				>
					backline
				</div>
				{#each player.public_info.cards_in_play as card, i}
					{#if card.position === 'Backline' && card.attached_to === null}
						<PlayingCardWithAttachments {game} cardWithDetails={card} />
					{/if}
				{/each}
				{#if showBattlefieldTargets}
					<button
						onclick={() => chooseBattlefield('BacklineBattlefield')}
						class="flex flex-col text-xs card relative w-[215px] h-[215px] transition duration-300 font-serif justify-center items-center border rounded"
						>backline</button
					>
				{/if}
			</div>
		</CardContent>
	</Card>
</div>
