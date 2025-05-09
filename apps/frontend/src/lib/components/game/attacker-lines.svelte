<script lang="ts">
	import { onMount } from 'svelte';
	import anime from 'animejs';
	import type { Attack, GameState } from '@gangsta/rusty';

	export let game: GameState;

	$: attackers = game.public_info.attacks;

	$: {
		if (container) {
			updated(attackers);
		}
	}

	let container: HTMLDivElement;
	onMount(() => {
		if (!container) return;
	});

	function updated(attackers: Attack[]) {
		const containerRect = container.getBoundingClientRect();

		for (const index in attackers) {
			const attacker = attackers[index];
			const startDiv = document.querySelector(
				`[data-player-id="${attacker.attacker.player_id}"][data-pile="${attacker.attacker.pile}"][data-card-index="${attacker.attacker.card_index}"]`
			);

			let selector = '';
			if (typeof (attacker.target as any).Player === 'string') {
				selector = `[data-player="${attacker.target.Player}"]`;
			} else {
				selector = `[data-player-id="${attacker.target.Card.player_id}"][data-pile="${attacker.target.Card.pile}"][data-card-index="${attacker.target.Card.card_index}"]`;
			}

			const endDiv = document.querySelector(selector);

			// if (startDiv && endDiv) {
			// 	const attacker = startDiv.getBoundingClientRect();
			// 	const receiver = endDiv.getBoundingClientRect();

			// 	// Convert absolute screen coordinates to container coordinates
			// 	const startX = attacker.left + attacker.width / 2 - containerRect.left;
			// 	const startY = attacker.top + attacker.height / 2 - containerRect.top;
			// 	const endX = receiver.left + receiver.width / 2 - containerRect.left;
			// 	const endY = receiver.top + receiver.height / 2 - containerRect.top;

			// 	// Define the path for the particles to follow using Bezier curve
			// 	const pathString = `M${startX},${startY} C${startX + 100},${startY} ${endX - 100},${endY} ${endX},${endY}`;

			// 	// Create an SVG element to define the path for motion
			// 	const svgNamespace = 'http://www.w3.org/2000/svg';
			// 	const svg = document.createElementNS(svgNamespace, 'svg');
			// 	const path = document.createElementNS(svgNamespace, 'path');
			// 	path.setAttribute('d', pathString);
			// 	path.setAttribute('fill', 'none');
			// 	svg.appendChild(path);
			// 	container.appendChild(svg);

			// 	// Create div particles instead of SVG
			// 	for (let i = 0; i < 30; i++) {
			// 		const particle = document.createElement('div');
			// 		particle.classList.add(`particle-${index}-${i}`, 'dot');

			// 		var size = anime.random(2, 8);

			// 		particle.style.width = size + 'px';
			// 		particle.style.height = size + 'px';
			// 		container.appendChild(particle);

			// 		// Get a random initial progress (0 to 1) along the path
			// 		const initialProgress = Math.random();
			// 		const motionPath = anime.path(path);

			// 		// Set the particle's initial position using the random progress
			// 		particle.style.transform = `translate(${motionPath('x', { progress: initialProgress })}px, ${motionPath('y', { progress: initialProgress })}px)`;

			// 		// Animate the particle to move along the path
			// 		anime({
			// 			targets: `.particle-${index}-${i}`,
			// 			opacity: [
			// 				{ value: 1, duration: 100, easing: 'easeInSine' }, // Fade in
			// 				{ value: 0.3, duration: anime.random(1000, 3000), easing: 'easeOutSine' } // Fade out
			// 			],
			// 			width: anime.random(0, 6),
			// 			height: anime.random(0, 6),
			// 			duration: anime.random(1500, 4000), // Control speed for each dot
			// 			loop: true,
			// 			easing: 'easeInOutSine',
			// 			delay: anime.stagger(300), // Stagger each particle's appearance for a smoother effect
			// 			translateX: motionPath('x'),
			// 			translateY: motionPath('y')
			// 		});
			// 	}
			// }
		}
	}
</script>

<div
	class="absolute attacker-lines pointer-events-none top-0 left-0 right-0 bottom-0 z-50"
	bind:this={container}
></div>

<style>
	:global(.dot) {
		position: absolute;
		background-color: #ffd700;
		border-radius: 50%;
		pointer-events: none;
	}

	:global(.attacker-lines svg) {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		z-index: -1; /* Ensure the SVG path is behind the dots */
	}
</style>
