<script lang="ts">
	import type {
		AbilityDetails,
		Card,
		CardPhase,
		CardType,
		CardWithDetails,
		FrontendPileName,
		FrontendTarget,
		GameState,
		ManaType
	} from '@gangsta/rusty';
	import { fly } from 'svelte/transition';
	import ManaBubble from '../mana-bubble/mana-bubble.svelte';
	import Ability from './ability.svelte';
	import { searchingForTarget, target, waitForTarget } from '../game';
	import { selectedAbility, selectFromAbilities } from '../../../stores/dialog';
	import { user } from '../../../stores/access-token';
	import { toast } from 'svelte-sonner';
	import { client } from '../../../client';
	import { RSPCError } from '@rspc/client';
	import ManaBubbleList from '../mana-bubble/mana-bubble-list.svelte';

	export let cardWithDetails: CardWithDetails;
	export let game: GameState | undefined = undefined;
	// export let pile: FrontendPileName;
	// export let cardWithDetails.frontend_target.card_index: number;
	// export let playerIndex: number;
	export let className: string = '';
	export { className as class };
	export let noTooltips = false;

	let showAttachments = true;

	$: card = cardWithDetails.card;

	async function selectAbilityDialog(abilities: AbilityDetails[]): Promise<AbilityDetails> {
		return await new Promise((resolve, reject) => {
			let t: NodeJS.Timeout;
			toast.info('Please select an ability');
			selectedAbility.set(null);
			selectFromAbilities.set(abilities);
			selectedAbility.subscribe((ability) => {
				if (ability) {
					clearTimeout(t);
					resolve(ability);
				}
			});
			t = setTimeout(() => reject('ran out of time'), 10000);
		});
	}

	async function selectAbility(card: CardWithDetails) {
		let met = card.abilities.filter(
			(c) => c.meets_mana_requirements && c.meets_requirements_except_mana
		);
		if (met.length === 0) {
			return;
		}
		if (met.length === 1) {
			return met[0];
		}

		return selectAbilityDialog(met);
	}
	// Function to display the current phase in a readable format
	function displayCurrentPhase(phase: CardPhase) {
		if (typeof phase === 'string') {
			return phase;
		}
		const [key, value] = Object.entries(phase)[0];
		return `${key}: ${value}`;
	}

	// Function to display the card's stats
	function displayStats(stats: (typeof card)['stats']) {
		return Object.values(stats.stats).map((stat) => {
			return `${stat.stat_type}: ${stat.intensity}`;
		});
	}

	function displayOtherStats(stats: (typeof card)['stats']) {
		return Object.values(stats.stats).filter(
			(stat) =>
				stat.stat_type !== 'Power' && stat.stat_type !== 'Toughness' && stat.stat_type !== 'Counter'
		);
	}

	let damage = 0;
	let defense = 0;
	function extractDamageAndDefense(stats: (typeof card)['stats']) {
		damage = 0;
		defense = 0;
		for (let stat of Object.values(stats.stats)) {
			if (stat.stat_type === 'Power') {
				damage += stat.intensity;
			} else if (stat.stat_type === 'Toughness') {
				defense += stat.intensity;
			}
		}
	}

	$: {
		if (card) {
			extractDamageAndDefense(card.stats);
		}
	}

	function isAdvancedLand(cardType: CardType): cardType is { AdvancedLand: ManaType } {
		return typeof cardType !== 'string' && 'AdvancedLand' in cardType;
	}

	function isBasicLand(cardType: CardType): cardType is { BasicLand: ManaType } {
		return typeof cardType !== 'string' && 'BasicLand' in cardType;
	}

	$: manaType = isBasicLand(card.card_type)
		? card.card_type.BasicLand
		: isAdvancedLand(card.card_type)
			? card.card_type.AdvancedLand
			: null;

	async function actionCard() {
		console.log(cardWithDetails.abilities);
		if ($searchingForTarget) {
			console.log('set target.');
			target.set({ Card: cardWithDetails.frontend_target });
			return;
		}
		if (noTooltips) {
			return;
		}
		if (game) {
			if ($user?.sub === cardWithDetails.frontend_target.player_id) {
				try {
					const ability = await selectAbility(cardWithDetails);
					if (!ability) {
						const maybeAbilities = cardWithDetails.abilities.filter(
							(a) => a.meets_requirements_except_mana
						);
						if (maybeAbilities.length) {
							throw new Error(
								`Not enough mana to cast ${maybeAbilities[0].action_type} for ${card.name}`
							);
						}
						throw new Error('This card has no ability right now.');
					}

					const target = await waitForTarget(ability, game);
					await executeAction(target, ability);
				} catch (e) {
					toast.error((e as Error).toString());
				}
			}
		}
	}

	async function executeAction(target: FrontendTarget | null, ability: AbilityDetails) {
		try {
			await client.mutation([
				'lobby.action_card',
				{
					code: game!.code,
					card: cardWithDetails.frontend_target,
					target,
					trigger_id: ability.id
				}
			]);
		} catch (e) {
			if (e instanceof RSPCError) {
				return toast.error(e.message);
			}
			toast.error('Unknown error!');
		}
	}

	$: shownAbilities = cardWithDetails.abilities.filter((a) => a.show);
</script>

<button
	on:click={actionCard}
	class:rotate-90={card.tapped}
	class:scale-75={card.tapped}
	class:has-attachments={$$slots.default}
	class="flex flex-col text-xs card relative w-[215px] h-[300px] transition duration-300 font-serif {className}"
	data-card-index={cardWithDetails.frontend_target.card_index}
	data-pile={cardWithDetails.frontend_target.pile}
	data-player-id={cardWithDetails.frontend_target.player_id}
	in:fly={{ y: '-300%', duration: 500 }}
>
	{#if $$slots.default}
		<div class="attachment-wrapper" transition:fly={{ y: 100, duration: 300 }}>
			<slot />
		</div>
	{/if}
	<div
		class:defending={game?.public_info.blocks.find(
			(a) =>
				a.blocker.card_index === cardWithDetails.frontend_target.card_index &&
				a.blocker.pile === cardWithDetails.frontend_target.pile &&
				a.blocker.player_id === cardWithDetails.frontend_target.player_id
		)}
	></div>
	<div
		class:attacking={game?.public_info.attacks.find(
			(a) =>
				a.attacker.card_index === cardWithDetails.frontend_target.card_index &&
				a.attacker.pile === cardWithDetails.frontend_target.pile &&
				a.attacker.player_id === cardWithDetails.frontend_target.player_id
		)}
	></div>
	<div
		class="card-main relative overflow-hidden rounded-xl border-[3px] dark:border-gray-700/40 border-gray-300/40 bg-gray-100 dark:bg-gray-950 w-full h-full"
	>
		<div
			class="card-header flex items-center justify-between w-full py-0.5 px-2 w-full"
			class:bg-green-200={manaType === 'Green'}
			class:bg-blue-200={manaType === 'Blue'}
			class:bg-black={manaType === 'Black'}
			class:bg-white={manaType === 'White'}
			class:text-white={manaType === 'Black'}
			class:text-black={manaType === 'White'}
			class:dark:bg-green-800={manaType === 'Green'}
			class:dark:bg-blue-800={manaType === 'Blue'}
			class:dark:bg-black={manaType === 'Black'}
			class:dark:bg-white={manaType === 'White'}
		>
			<h2 class="text-xs leading-6 font-bold truncate">
				{card.name}
			</h2>
			<ManaBubbleList mana={card.cost} />
		</div>

		<div class="card-type mb-2 flex w-full py-0.5 px-2 font-mono">
			<div class="text-gray-500 dark:text-stone-600 text-left uppercase">
				{#if typeof card.card_type === 'string'}
					{#if card.creature_type && card.creature_type !== 'None'}
						{card.creature_type}
					{/if}
					{card.card_type}
				{:else if isBasicLand(card.card_type)}
					Basic Land
				{:else if isAdvancedLand(card.card_type)}
					Land
				{/if}
			</div>
			{#if !!damage || !!defense}
				<div class="ml-auto">
					{damage}/{defense}
				</div>
			{:else if card.card_type === 'Planeswalker'}
				<div class="ml-auto">
					{Object.values(card.stats.stats).find((x) => x.stat_type === 'Counter')?.intensity}
				</div>
			{/if}
		</div>

		<div class="text-xs text-left px-2">
			<ul class="flex space-x-2 text-gray-700 dark:text-gray-300 uppercase font-semibold">
				{#each displayOtherStats(card.stats) as stat}
					{#each Object.keys(stat) as key}
						{#if key != 'intensity'}
							<li>
								{stat[key]}
							</li>
						{/if}
					{/each}
				{/each}
			</ul>
		</div>

		<div class="text-left mb-6 px-2">
			{#if card.description}
				<p class="text-gray-700 dark:text-gray-300 line-clamp-6 mb-1.5">{card.description}</p>
			{/if}
			{#if shownAbilities.length > 0}
				<div class="space-y-1.5">
					{#each shownAbilities as ability, i}
						<Ability
							{noTooltips}
							inHand={cardWithDetails.frontend_target.pile === 'Hand'}
							{ability}
						/>
					{/each}
				</div>
			{/if}
			{#if card.card_type === 'Creature' && Object.values(card.counters).length > 0}
				<div class=" text-muted">Counters</div>
				<div class="flex space-x-2">
					{#each Object.entries(Object.values(card.counters).reduce((acc, counter) => {
							const key = `${counter.PowerToughnessModifier[0]}/${counter.PowerToughnessModifier[1]}`;
							if (!acc[key]) {
								acc[key] = 1;
							} else {
								acc[key]++;
							}
							return acc;
						}, {})) as [key, count]}
						<div>
							{count}x {key}
						</div>
					{/each}
				</div>
			{/if}
		</div>
		<div class="text-left px-2 absolute top-full pb-1 -translate-y-full">
			<span class="text-xs text-gray-400 uppercase font-sans">
				{displayCurrentPhase(card.current_phase)}
			</span>
		</div>
	</div>
</button>

<style>
	@keyframes flicker {
		0% {
			opacity: 0.8;
		}
		25% {
			opacity: 1;
		}
		50% {
			opacity: 0.9;
		}
		75% {
			opacity: 1;
		}
		100% {
			opacity: 0.8;
		}
	}

	button:hover > .card-main {
		transform: scale(1.05);
	}

	button:hover > .attachment-wrapper {
		transform: translateX(0);
		opacity: 1;
	}

	.card-main {
		position: relative;
		overflow: hidden;
		border-radius: 10px;
		border: 3px solid gray;
		transition: transform 0.2s ease;
	}

	.defending {
		position: absolute;
		top: -10px;
		left: -10px;
		right: -10px;
		bottom: -10px;
		background: radial-gradient(
			ellipse at center,
			rgba(173, 216, 230, 0.8) 0%,
			/* LightBlue */ rgba(135, 206, 250, 0.7) 35%,
			/* LightSkyBlue */ rgba(0, 191, 255, 0.5) 65%,
			/* DeepSkyBlue */ rgba(0, 0, 255, 0) 100% /* Blue (transparent at the edges) */
		);
		filter: blur(10px);
		opacity: 0.8;
		animation: flicker 3s infinite alternate;
		/* Remove z-index */
	}
	.defending::before,
	.defending::after {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: inherit;
		filter: blur(10px);
		opacity: 0.8;
		animation: flicker 5s infinite alternate-reverse;
	}

	.defending::after {
		filter: blur(20px);
		animation-duration: 7s;
	}
	.attacking {
		position: absolute;
		top: -10px;
		left: -10px;
		right: -10px;
		bottom: -10px;
		background: radial-gradient(
			ellipse at center,
			rgba(255, 174, 0, 0.8) 0%,
			rgba(255, 103, 0, 0.7) 35%,
			rgba(255, 0, 0, 0.5) 65%,
			rgba(0, 0, 0, 0) 100%
		);
		filter: blur(10px);
		opacity: 0.8;
		animation: flicker 3s infinite alternate;
		/* Remove z-index */
	}
	.attacking::before,
	.attacking::after {
		content: '';
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: inherit;
		filter: blur(10px);
		opacity: 0.8;
		animation: flicker 5s infinite alternate-reverse;
	}

	.attacking::after {
		filter: blur(20px);
		animation-duration: 7s;
	}
	.has-attachments {
		border-color: red !important;
		box-shadow: 0 0 10px rgba(255, 0, 0, 0.5);
		animation: pulse 2s infinite alternate;
	}
	.attachment-wrapper {
		position: absolute;
		top: 0;
		left: 100%;
		transform: translateX(-100%);
		display: flex;
		flex-direction: column;
		gap: 8px;
		opacity: 0;
		transition:
			transform 0.3s ease,
			opacity 0.3s ease;
	}

	@keyframes pulse {
		0% {
			box-shadow: 0 0 10px rgba(255, 0, 0, 0.5);
		}
		100% {
			box-shadow: 0 0 20px rgba(255, 0, 0, 0.8);
		}
	}
</style>
