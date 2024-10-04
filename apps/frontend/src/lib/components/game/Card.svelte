<script lang="ts">
	import type { Card, CardPhase, CardWithDetails } from '@gangsta/rusty';

	export let cardWithDetails: CardWithDetails;
	export let className: string = '';
	export { className as class };
	$: card = cardWithDetails.card;

	// Function to display the current phase in a readable format
	function displayCurrentPhase(phase: CardPhase) {
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
			(stat) => stat.stat_type !== 'Damage' && stat.stat_type !== 'Defense'
		);
	}

	let damage = 0;
	let defense = 0;
	function extractDamageAndDefense(stats: (typeof card)['stats']) {
		damage = 0;
		defense = 0;
		for (let stat of Object.values(stats.stats)) {
			if (stat.stat_type === 'Damage') {
				damage += stat.intensity;
			} else if (stat.stat_type === 'Defense') {
				defense += stat.intensity;
			}
		}
	}
	// const { damage, defense } = extractDamageAndDefense(card.stats);

	$: {
		if (card) {
			extractDamageAndDefense(card.stats);
		}
	}
</script>

<button
	on:click
	class:rotate-90={card.tapped}
	class="flex flex-col border-[3px] dark:border-gray-700/40 border-gray-300/40 card dark:bg-gray-900 rounded-xl overflow-hidden relative w-[184px] h-[184px] transition duration-300 {className}"
>
	<div
		class="card-header flex items-center justify-between mb-2 w-full py-1 px-2"
		class:bg-green-200={card.card_type?.BasicLand === 'Green'}
		class:bg-blue-200={card.card_type?.BasicLand === 'Blue'}
		class:dark:bg-green-800={card.card_type?.BasicLand === 'Green'}
		class:dark:bg-blue-800={card.card_type?.BasicLand === 'Blue'}
	>
		<h2 class="text-sm font-bold truncate">
			{card.name}
		</h2>
		<div class="absolute flex space-x-2 right-2">
			{#each card.cost as color}
				<div
					class="w-4 h-4 rounded-full"
					class:bg-gray-100={color === 'Colorless'}
					class:bg-green-400={color === 'Green'}
					class:bg-blue-400={color === 'Blue'}
				></div>
			{/each}
		</div>
	</div>

	<div class="card-type mb-2 flex w-full text-xs px-2">
		<div class="text-gray-600 text-left">
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

	<div class="text-left mb-4 line-clamp-3 h-16 px-2 text-sm">
		<p class="text-muted">{card.description}</p>
	</div>

	<div class="card-current-phase mb-4">
		<span class="text-xs md:text-sm text-gray-600 px-2">
			Phase: {displayCurrentPhase(card.current_phase)}
		</span>
	</div>

	<div class="card-stats">
		<ul class="list-disc list-inside text-gray-800 text-xs md:text-sm">
			{#each displayOtherStats(card.stats) as stat}
				{#each Object.keys(stat) as key}
					<li>{key}: {stat[key]}</li>
				{/each}
			{/each}
		</ul>
	</div>
</button>
