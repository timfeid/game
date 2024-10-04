import type { ActionCardTarget, CardWithDetails, GameState } from '@gangsta/rusty';
import { toast } from 'svelte-sonner';
import { writable } from 'svelte/store';
import type { EffectTarget } from './type';

export const target = writable<EffectTarget | undefined>();
export const searchingForTarget = writable(false);

async function search(card: CardWithDetails, game: GameState): Promise<undefined | EffectTarget> {
	if (card.required_target === 'EnemyCardOrPlayer') {
		toast.info('Please select your target');

		if (target) {
			try {
				return await new Promise((resolve, reject) => {
					target.subscribe((e) => {
						console.log(e);
						if (e) {
							if (e.name === 'player') {
								return resolve(e);
							}
							reject('Invalid target, looking for ' + card.required_target);
						}
					});
				});
			} catch (e) {
				toast.error((e as Error).message);
			}
		}

		return undefined;
	}

	if (typeof card.required_target !== 'string') {
		const lookingFor = card.required_target.CardOfType;
		toast.info('Please select your target: ' + lookingFor);

		if (target) {
			try {
				return new Promise((resolve, reject) => {
					target.subscribe((e) => {
						console.log(e);
						if (e) {
							if (e.name === 'card') {
								const targetPlayer = Object.values(game.players).find(
									(p) => p.player_index == e.player_index
								);
								if (targetPlayer) {
									const card = targetPlayer.public_info.cards_in_play[e.card_index];
									if (card.card.card_type === lookingFor) {
										return resolve(e);
									}
								}
							}
							reject('Invalid target, looking for ' + card.required_target);
						}
					});
				});
			} catch (e) {
				toast.error((e as Error).message);
			}
		}

		return;
	}
}

export async function waitForTarget(
	card: CardWithDetails,
	game: GameState,
	forPlay = false
): Promise<undefined | EffectTarget> {
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

export function convertEffectTarget(target: EffectTarget): ActionCardTarget {
	if (target.name === 'player') {
		return { Player: target.index };
	}
	// if (target.name === 'card') {
	return { CardInPlay: [target.player_index, target.card_index] };
	// }
}
