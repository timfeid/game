import type {
	AbilityDetails,
	CardRequiredTarget,
	CardTargetTeam,
	CardType,
	CreatureType,
	FrontendCardTarget,
	FrontendTarget,
	GameState
} from '@gangsta/rusty';
import { tick } from 'svelte';
import { toast } from 'svelte-sonner';
import { writable } from 'svelte/store';

export const target = writable<FrontendTarget | null>();
export const searchingForTarget = writable(false);

function isPlayer(frontendTarget: FrontendTarget | null): frontendTarget is { Player: number } {
	if (!frontendTarget) {
		return false;
	}
	return 'Player' in frontendTarget;
}

function isCard(
	frontendTarget: FrontendTarget | null
): frontendTarget is { Card: FrontendCardTarget } {
	if (!frontendTarget) {
		return false;
	}
	return 'Card' in frontendTarget;
}

function isCardOfType(
	target: CardRequiredTarget
): target is { CardOfType: [CardType, CardTargetTeam] } {
	return typeof target === 'object' && 'CardOfType' in target;
}

function isCreatureTypeCardRequirement(
	target: CardRequiredTarget
): target is { CreatureOfType: [CreatureType, CardTargetTeam] } {
	return typeof target === 'object' && 'CreatureOfType' in target;
}

async function search(ability: AbilityDetails, game: GameState): Promise<null | FrontendTarget> {
	return await new Promise((resolve, reject) => {
		toast.info('Please select your target');
		target.set(null);
		target.subscribe((frontendTarget) => {
			if (isCard(frontendTarget)) {
				const targetPlayer = Object.values(game.players).find(
					(p) => p.player_index == frontendTarget.Card.player_index
				);
				let pile;
				if (frontendTarget.Card.pile === 'Spell') {
					pile = targetPlayer!.public_info.spells;
				} else if (frontendTarget.Card.pile === 'Hand') {
					// pile = targetPlayer!.public_info.
					console.log(
						'cannot read hand ?, still should return a e.target if were in a good required_target'
					);
				} else if (frontendTarget.Card.pile === 'Play') {
					pile = targetPlayer!.public_info.cards_in_play;
				}
				console.log(ability, frontendTarget.Card);
				if (ability.required_target === 'Spell' && frontendTarget.Card.pile === 'Spell') {
					return resolve(frontendTarget);
					// for (const )
					// if (
					// 	game.
					// ) {
					// 	return resolve(e);
					// }
				}
				if (ability.required_target === 'EnemyCardInCombat') {
					console.log(game.public_info);
					if (
						game.public_info.attacks.find((x) => {
							return (
								x.attacker.pile === frontendTarget.Card.pile &&
								x.attacker.card_index === frontendTarget.Card.card_index &&
								x.attacker.player_index === frontendTarget.Card.player_index
							);
						})
					) {
						return resolve(frontendTarget);
					}
				}

				if (pile) {
					const ccard = pile[frontendTarget.Card.card_index];
					console.log('pile', pile, ccard);
					if (isCreatureTypeCardRequirement(ability.required_target)) {
						const [type] = ability.required_target.CreatureOfType;
						// if (team)
						// TODO: check the team, too
						if (type === ccard.card.creature_type) {
							return resolve(frontendTarget);
						}
					}
					if (isCardOfType(ability.required_target)) {
						const [type] = ability.required_target.CardOfType;
						// if (team)
						// TODO: check the team, too
						if (type === ccard.card.card_type) {
							return resolve(frontendTarget);
						}
					}
					// console.log(ability.required_target);
					// if ((ability.required_target as any).CardOfType) {
					// 	// if ((frontendTarget.Card. as any).CardOfType);
					// 	if (ccard.card.card_type === (ability.required_target as any).CardOfType) {
					// 		return resolve(frontendTarget);
					// 	}
					// }
				}
			} else if (isPlayer(frontendTarget)) {
				console.log("it's a player,", frontendTarget);
				if (ability.required_target === 'EnemyCardOrPlayer') {
					return resolve(frontendTarget);
				}
			}

			if (frontendTarget && ability.required_target) {
				return reject('hi looking for' + JSON.stringify(ability.required_target));
			}
		});
	});
}

export async function waitForTarget(
	ability: AbilityDetails,
	game: GameState,
	forPlay = false
): Promise<null | FrontendTarget> {
	console.log(ability);
	if (!ability || ability.required_target === 'None') {
		return null;
	}
	if (ability.action_type !== 'Instant' && forPlay) {
		console.log('no?');
		return null;
	}

	target.set(null);
	searchingForTarget.set(true);
	await tick();
	const response = await search(ability, game);
	searchingForTarget.set(false);
	target.set(null);

	return response;
}
