<script lang="ts">
	import { browser } from '$app/environment';
	import { page } from '$app/stores';
	import { client, websocketClient } from '$lib/client';
	import { accessToken } from '$lib/stores/access-token';
	import type {
		AbilityDetails,
		CardSelectionDetails,
		ExecuteAbility,
		LobbyCommand,
		LobbyData,
		LobbyTurnMessage,
		SlotMachineResult
	} from '@gangsta/rusty';
	import { Loader } from 'lucide-svelte';
	import { type ComponentType } from 'svelte';
	import { toast } from 'svelte-sonner';
	import InGame from './InGame.svelte';
	import Lobby from './Lobby.svelte';
	import {
		askOptionalAbility,
		mandatoryAbility,
		selectFromCards,
		showSlotMachine
	} from '../../stores/dialog';

	export let code: string;

	let lobby: LobbyData | undefined;
	let unsubscribe: (() => void) | undefined;
	let turnMessage: LobbyTurnMessage | undefined;

	if (browser && $accessToken && code) {
		reset(code, $accessToken);
	}

	function isMandatoryExecuteAbility(
		data: LobbyCommand
	): data is { MandatoryExecuteAbility: ExecuteAbility } {
		return 'MandatoryExecuteAbility' in data;
	}

	function isAskExecuteAbility(data: LobbyCommand): data is { AskExecuteAbility: ExecuteAbility } {
		return 'AskExecuteAbility' in data;
	}

	function isCardSelection(
		data: LobbyCommand
	): data is { ChooseFromSelection: CardSelectionDetails } {
		return 'ChooseFromSelection' in data;
	}

	function isTurnMessages(data: LobbyCommand): data is { TurnMessages: LobbyTurnMessage } {
		return 'TurnMessages' in data;
	}

	function isUpdated(data: LobbyCommand): data is { Updated: LobbyData } {
		return 'Updated' in data;
	}

	function isSlotMachine(data: LobbyCommand): data is { ShowSlotMachine: SlotMachineResult } {
		return 'ShowSlotMachine' in data;
	}

	function askMandatoryAbility(updatedMessage: ExecuteAbility) {
		console.log('MAND');
		mandatoryAbility.set(updatedMessage);
	}

	function askExecuteAbility(updatedMessage: ExecuteAbility) {
		console.log('OPTION');
		askOptionalAbility.set(updatedMessage);
	}

	function turnMessageReceived(updatedMessage: LobbyTurnMessage) {
		turnMessage = updatedMessage;
	}

	function updated(data: LobbyData) {
		console.log(data);
		lobby = data;
	}

	function cardSelection(details: CardSelectionDetails) {
		selectFromCards.set(details);
	}

	function slotMachine(details: SlotMachineResult) {
		showSlotMachine.set(details);
	}

	async function reset(code: string, accessToken: string) {
		if (unsubscribe) {
			unsubscribe();
		}
		unsubscribe = websocketClient.addSubscription(['lobby.subscribe', [code, accessToken]], {
			onData(data) {
				if (isSlotMachine(data)) {
					return slotMachine(data.ShowSlotMachine);
				}
				if (isUpdated(data)) {
					return updated(data.Updated);
				}
				if (isAskExecuteAbility(data)) {
					return askExecuteAbility(data.AskExecuteAbility);
				}
				if (isMandatoryExecuteAbility(data)) {
					return askMandatoryAbility(data.MandatoryExecuteAbility);
				}

				if (isTurnMessages(data)) {
					return turnMessageReceived(data.TurnMessages);
				}

				if (isCardSelection(data)) {
					return cardSelection(data.ChooseFromSelection);
				}
			},
			onStarted() {
				console.log('started.');
			},
			onError(e) {
				console.log('error when streaming');
				console.error(e);
			}
		});

		await join(code);
	}

	async function join(code: string) {
		try {
			await client.mutation(['lobby.join', code]);
		} catch (e) {
			console.error(e);
			toast.error('Something went wrong');
		}
	}

	let component: ComponentType;
	$: {
		if (!lobby) {
			component = Loader;
		} else if (
			lobby?.game_state.status === 'NeedsPlayers' ||
			typeof lobby.game_state.status !== 'string'
		) {
			component = Lobby;
		} else {
			component = InGame;
		}
	}
</script>

{#if lobby}
	<svelte:component this={component} {...lobby} {turnMessage} />
	{#if $page.url.searchParams.has('debug')}
		<pre class="mt-16">{JSON.stringify(lobby, null, 2)}</pre>
	{/if}
{/if}
