<script lang="ts">
	import { goto } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import {
		Card,
		CardContent,
		CardDescription,
		CardFooter,
		CardHeader,
		CardTitle
	} from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { saveLoginDetails } from '../../auth';
	import { client } from '../../client';

	let loading = false;
	const args = {
		username: '',
		password: ''
	};
	async function login() {
		loading = true;
		try {
			const response = await client.mutation(['authentication.login', args]);
			if (response.success && response.access_token && response.refresh_token) {
				await saveLoginDetails(response);
			}
		} catch (e) {
			console.log(e);
		}
		loading = false;
		goto('/');
	}
</script>

<div class="pt-[25vh] mx-auto w-full max-w-md">
	<Card>
		<form on:submit|preventDefault={login}>
			<CardHeader class="space-y-1">
				<CardTitle class="text-2xl font-bold text-center">Login</CardTitle>
				<CardDescription class="text-center">
					Enter your username and password to access your account
				</CardDescription>
			</CardHeader>
			<CardContent class="space-y-4">
				<div class="space-y-2">
					<label
						for="username"
						class="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
					>
						Username
					</label>
					<Input
						focus
						bind:value={args.username}
						id="username"
						placeholder="Enter your username"
						class="w-full px-3 py-2 text-sm"
					/>
				</div>
				<div class="space-y-2">
					<label
						for="password"
						class="text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70"
					>
						Password
					</label>
					<Input
						bind:value={args.password}
						id="password"
						type="password"
						placeholder="Enter your password"
						class="w-full px-3 py-2 text-sm"
					/>
				</div>
			</CardContent>
			<CardFooter>
				<Button type="submit" {loading} class="w-full">Sign In</Button>
			</CardFooter>
		</form>
	</Card>
</div>
