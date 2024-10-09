import type { ExecuteAbility } from '@gangsta/rusty';
import { writable } from 'svelte/store';

export const askOptionalAbility = writable<ExecuteAbility | undefined>();
export const mandatoryAbility = writable<ExecuteAbility | undefined>();
