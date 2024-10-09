<script lang="ts">
	import { browser } from '$app/environment';
	import { page } from '$app/stores';
	import { client, websocketClient } from '$lib/client';
	import { accessToken } from '$lib/stores/access-token';
	import type {
		AbilityDetails,
		ExecuteAbility,
		LobbyCommand,
		LobbyData,
		LobbyTurnMessage
	} from '@gangsta/rusty';
	import { Loader } from 'lucide-svelte';
	import { type ComponentType } from 'svelte';
	import { toast } from 'svelte-sonner';
	import InGame from './InGame.svelte';
	import Lobby from './Lobby.svelte';
	import { askOptionalAbility, mandatoryAbility } from '../../stores/dialog';

	let lobby: LobbyData | undefined;
	let unsubscribe: (() => void) | undefined;
	let turnMessage: LobbyTurnMessage | undefined;

	if (browser && $accessToken) {
		reset($accessToken);
	}

	function isMandatoryExecuteAbility(
		data: LobbyCommand
	): data is { MandatoryExecuteAbility: ExecuteAbility } {
		return 'MandatoryExecuteAbility' in data;
	}

	function isAskExecuteAbility(data: LobbyCommand): data is { AskExecuteAbility: ExecuteAbility } {
		return 'AskExecuteAbility' in data;
	}

	function isTurnMessages(data: LobbyCommand): data is { TurnMessages: LobbyTurnMessage } {
		return 'TurnMessages' in data;
	}

	function isUpdated(data: LobbyCommand): data is { Updated: LobbyData } {
		return 'Updated' in data;
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
		lobby = data;
	}

	async function reset(accessToken: string) {
		if (unsubscribe) {
			unsubscribe();
		}
		unsubscribe = websocketClient.addSubscription(
			['lobby.subscribe', [$page.params.slug, accessToken]],
			{
				onData(data) {
					// console.log(data);
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
				},
				onStarted() {
					console.log('started.');
				},
				onError(e) {
					console.log('error when streaming');
					console.error(e);
				}
			}
		);

		await join();
	}
	export let code: string;

	async function join() {
		try {
			await client.mutation(['lobby.join', $page.params.slug]);
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
