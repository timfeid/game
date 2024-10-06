<script lang="ts">
	import type {
		Card,
		CardPhase,
		CardWithDetails,
		FrontendPileName,
		GameState
	} from '@gangsta/rusty';
	import { fly } from 'svelte/transition';
	import ManaBubble from './mana-bubble.svelte';

	export let cardWithDetails: CardWithDetails;
	export let className: string = '';
	export let game: GameState | undefined = undefined;
	export let pile: FrontendPileName;
	export let cardIndex: number;
	export let playerIndex: number;
	export { className as class };
	$: card = cardWithDetails.card;

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
			(stat) => stat.stat_type !== 'Power' && stat.stat_type !== 'Toughness'
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
</script>

<button
	on:click
	class:rotate-90={card.tapped}
	class="flex flex-col card relative w-[184px] h-[184px] transition duration-300 font-serif {className}"
	data-card-index={cardIndex}
	data-pile={pile}
	data-player-index={playerIndex}
	in:fly={{ y: '-300%', duration: 500 }}
>
	<div
		class:defending={game?.public_info.blocks.find(
			(a) =>
				a.blocker.card_index === cardIndex &&
				a.blocker.pile === pile &&
				a.blocker.player_index === playerIndex
		)}
	></div>
	<div
		class:attacking={game?.public_info.attacks.find(
			(a) =>
				a.attacker.card_index === cardIndex &&
				a.attacker.pile === pile &&
				a.attacker.player_index === playerIndex
		)}
	></div>
	<div
		class="relative overflow-hidden rounded-xl border-[3px] dark:border-gray-700/40 border-gray-300/40 bg-gray-100 dark:bg-gray-950 w-full h-full"
	>
		<div
			class="card-header flex items-center justify-between w-full py-0.5 px-2 w-full"
			class:bg-green-200={card.card_type?.BasicLand === 'Green'}
			class:bg-blue-200={card.card_type?.BasicLand === 'Blue'}
			class:bg-black={card.card_type?.BasicLand === 'Black'}
			class:bg-white={card.card_type?.BasicLand === 'White'}
			class:text-white={card.card_type?.BasicLand === 'Black'}
			class:text-black={card.card_type?.BasicLand === 'White'}
			class:dark:bg-green-800={card.card_type?.BasicLand === 'Green'}
			class:dark:bg-blue-800={card.card_type?.BasicLand === 'Blue'}
			class:dark:bg-black={card.card_type?.BasicLand === 'Black'}
			class:dark:bg-white={card.card_type?.BasicLand === 'White'}
		>
			<h2 class="text-sm font-bold truncate">
				{card.name}
			</h2>
			<div class="absolute flex space-x-2 right-2">
				{#each card.cost as color}
					<ManaBubble {color} />
				{/each}
			</div>
		</div>

		<div class="card-type mb-2 flex w-full text-xs py-0.5 px-2 font-mono">
			<div class="text-gray-500 dark:text-stone-600 text-left uppercase">
				{#if typeof card.card_type === 'string'}
					{card.card_type}
				{:else if card.card_type?.BasicLand}
					{card.card_type?.BasicLand} Land
				{/if}
			</div>
			{#if !!damage || !!defense}
				<div class="ml-auto">
					{damage}/{defense}
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

		<div class="text-left mb-4 line-clamp-3 h-16 px-2 text-sm">
			<p class="text-gray-700 dark:text-gray-300">{card.description}</p>
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
</style>
