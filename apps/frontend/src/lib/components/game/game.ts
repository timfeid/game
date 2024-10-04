import type {
	CardWithDetails,
	FrontendCardTarget,
	FrontendTarget,
	GameState
} from '@gangsta/rusty';
import { toast } from 'svelte-sonner';
import { writable } from 'svelte/store';

export const target = writable<FrontendTarget | undefined>();
export const searchingForTarget = writable(false);

function isCard(card: unknown): card is FrontendCardTarget {
	return (card as any) !== undefined;
}

async function search(card: CardWithDetails, game: GameState): Promise<undefined | FrontendTarget> {
	return await new Promise((resolve, reject) => {
		toast.info('Please select your target');
		target.subscribe((e) => {
			if (e) {
				const c = (e as any).Card;
				const p = (e as any).Player;

				if (isCard(c)) {
					const targetPlayer = Object.values(game.players).find(
						(p) => p.player_index == c.player_index
					);
					let pile;
					if (c.pile === 'Spell') {
						pile = targetPlayer!.public_info.spells;
					} else if (c.pile === 'Hand') {
						// pile = targetPlayer!.public_info.
						console.log(
							'cannot read hand ?, still should return a e.target if were in a good required_target'
						);
					} else if (c.pile === 'Play') {
						pile = targetPlayer!.public_info.cards_in_play;
					}
					console.log(card, c);
					if (card.required_target === 'Spell' && c.pile === 'Spell') {
						return resolve(e);
						// for (const )
						// if (
						// 	game.
						// ) {
						// 	return resolve(e);
						// }
					}
					if (card.required_target === 'EnemyCardInCombat') {
						if (
							game.public_info.attacking_cards.find((x) => {
								return (
									x.pile === c.pile &&
									x.card_index === c.card_index &&
									x.player_index === c.player_index
								);
							})
						) {
							return resolve(e);
						}
					}

					if (pile) {
						const ccard = pile[c.card_index];
						if ((card.required_target as any).CardOfType) {
							// if ((c. as any).CardOfType);
							if (ccard.card.card_type === (card.required_target as any).CardOfType) {
								return resolve(e);
							}
						}
					}
				}
				if (Number.isInteger(p)) {
					console.log("it's a player,", p);
					if (card.required_target === 'EnemyCardOrPlayer') {
						return resolve(e);
					}
				}

				reject('hi looking for' + JSON.stringify(card.required_target));
			}
		});
	});
}

export async function waitForTarget(
	card: CardWithDetails,
	game: GameState,
	forPlay = false
): Promise<undefined | FrontendTarget> {
	console.log(card);
	if (card.required_target === 'None' || card.action_type === 'None') {
		return undefined;
	}
	if (card.action_type !== 'Instant' && forPlay) {
		return undefined;
	}

	target.set(undefined);
	searchingForTarget.set(true);
	const response = await search(card, game);
	searchingForTarget.set(false);
	target.set(undefined);

	return response;
}
