import type {
	AbilityDetails,
	CardSelectionDetails,
	ExecuteAbility,
	FrontendCardTarget
} from '@gangsta/rusty';
import { writable } from 'svelte/store';

export const askOptionalAbility = writable<ExecuteAbility | undefined>();
export const mandatoryAbility = writable<ExecuteAbility | undefined>();

export const selectFromAbilities = writable<AbilityDetails[] | undefined>();
export const selectedAbility = writable<AbilityDetails | null>(null);

export const selectFromCards = writable<CardSelectionDetails | undefined>();
export const selectedCard = writable<FrontendCardTarget | null>(null);
