<script lang="ts">
	import type { CardWithDetails, FrontendCardTarget, GameState } from '@gangsta/rusty';
	import PlayingCard from './playing-card.svelte';

	interface Props {
		cardWithDetails: CardWithDetails;
		game: GameState;
	}

	let props: Props = $props();

	function isSameTarget(target1: FrontendCardTarget, target2: FrontendCardTarget) {
		return (
			target1?.card_index === target2?.card_index &&
			target1?.pile === target2?.pile &&
			target1?.player_id === target2?.player_id
		);
	}

	const attachments = $derived.by(() => {
		const attachments: CardWithDetails[] = [];
		for (const player of Object.values(props.game.players)) {
			for (const card of player.public_info.cards_in_play) {
				if (
					card.attached_to &&
					isSameTarget(props.cardWithDetails.frontend_target, card.attached_to)
				) {
					attachments.push(card);
				}
			}
		}

		return attachments;
	});
</script>

{#if attachments.length > 0}
	<PlayingCard {...props}>
		{#each attachments as attachment}
			<PlayingCard cardWithDetails={attachment} game={props.game} />
		{/each}
	</PlayingCard>
{:else}
	<PlayingCard {...props} />
{/if}
