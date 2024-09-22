<script lang="ts">
	import { Button } from '$lib/components/ui/button/index.js';
	import Plus from 'lucide-svelte/icons/badge-plus';
	import Hamburger from 'lucide-svelte/icons/ellipsis-vertical';
	import LogOut from 'lucide-svelte/icons/log-out';

	import * as DropdownMenu from '$lib/components/ui/dropdown-menu/index.js';
	import { Eclipse, Moon, Palette, Sun } from 'lucide-svelte';
	import { setTheme } from '../../../stores/theme';

	// let creatingAccount = false;
	// async function addAccount() {
	// createAccount();
	// creatingAccount = true;
	// if (isTauri) {
	// 	return await openQrScanner();
	// }
	// }
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger asChild let:builder>
		<Button builders={[builder]} variant="secondary" size="icon">
			<Hamburger size={12} />
		</Button>
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end" class="w-56">
		<DropdownMenu.Sub>
			<DropdownMenu.SubTrigger>
				<Palette class="mr-2 h-4 w-4" />
				<span>Theme</span>
			</DropdownMenu.SubTrigger>
			<DropdownMenu.SubContent>
				<DropdownMenu.Item on:click={() => setTheme('dark')}>
					<Moon class="mr-2 h-4 w-4" />
					<span>Dark mode</span>
				</DropdownMenu.Item>
				<DropdownMenu.Item on:click={() => setTheme('light')}>
					<Sun class="mr-2 h-4 w-4" />
					<span>Light mode</span>
				</DropdownMenu.Item>
				<DropdownMenu.Item on:click={() => setTheme(undefined)}>
					<Eclipse class="mr-2 h-4 w-4" />
					<span>Automatic</span>
				</DropdownMenu.Item>
			</DropdownMenu.SubContent>
		</DropdownMenu.Sub>
		<DropdownMenu.Separator />
		<DropdownMenu.Item href="/logout">
			<LogOut class="mr-2 h-4 w-4" />
			<span>Log out</span>
		</DropdownMenu.Item>
	</DropdownMenu.Content>
</DropdownMenu.Root>

<!-- <AddAccountDialog bind:creatingAccount /> -->
