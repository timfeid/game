<script lang="ts">
	import { onMount, onDestroy } from 'svelte';

	interface Stock {
		symbol: string;
		price: number;
		change: number;
	}

	let stocks: Stock[] = [];
	let tickerContent: HTMLDivElement;
	let priceUpdateInterval: NodeJS.Timeout;

	// Generate initial stock data
	function generateStocks(): Stock[] {
		const symbols = ['AAPL', 'GOOG', 'AMZN', 'MSFT', 'TSLA', 'META', 'NFLX', 'NVDA', 'JPM', 'BAC'];
		return symbols.map((symbol) => ({
			symbol,
			price: randomPrice(),
			change: 0
		}));
	}

	function randomPrice(): number {
		return +(Math.random() * 1000 + 100).toFixed(2); // Prices between $100 and $1100
	}

	// Update stock prices randomly to simulate market changes
	function updateStockPrices(): void {
		stocks = stocks.map((stock) => {
			const change = +(Math.random() * 10 - 5).toFixed(2); // Random change between -5 and +5
			const newPrice = +(stock.price + change).toFixed(2);
			return {
				...stock,
				price: newPrice > 0 ? newPrice : stock.price, // Ensure price doesn't go negative
				change
			};
		});
	}

	onMount(() => {
		stocks = generateStocks();

		// Update stock prices every 5 seconds
		priceUpdateInterval = setInterval(() => {
			updateStockPrices();
		}, 5000);
	});

	onDestroy(() => {
		clearInterval(priceUpdateInterval);
	});
</script>

<div class="ticker-container">
	<div bind:this={tickerContent} class="ticker-content">
		{#each stocks as stock}
			<div class="ticker-item">
				<strong>{stock.symbol}</strong> ${stock.price.toFixed(2)}
				<span class={stock.change > 0 ? 'positive' : stock.change < 0 ? 'negative' : 'neutral'}>
					({stock.change > 0 ? '+' : ''}{stock.change.toFixed(2)})
				</span>
			</div>
		{/each}
		<!-- Duplicate the stocks to create a seamless loop -->
		{#each stocks as stock}
			<div class="ticker-item">
				<strong>{stock.symbol}</strong> ${stock.price.toFixed(2)}
				<span class={stock.change > 0 ? 'positive' : stock.change < 0 ? 'negative' : 'neutral'}>
					({stock.change > 0 ? '+' : ''}{stock.change.toFixed(2)})
				</span>
			</div>
		{/each}
	</div>
</div>

<style>
	.ticker-container {
		overflow: hidden;
		background-color: #1a1a1a;
		color: #f0f0f0;
		font-family: Arial, sans-serif;
		padding: 10px 0;
		border-top: 2px solid #444;
		border-bottom: 2px solid #444;
		position: relative;
	}

	.ticker-content {
		display: flex;
		align-items: center;
		white-space: nowrap;
		animation: scroll 5s linear infinite;
	}

	@keyframes scroll {
		from {
			transform: translateX(0%);
		}
		to {
			transform: translateX(-50%);
		}
	}

	.ticker-item {
		display: inline-block;
		padding: 0 15px;
		box-sizing: border-box;
	}

	.positive {
		color: #4caf50; /* Green for positive change */
	}

	.negative {
		color: #f44336; /* Red for negative change */
	}

	.neutral {
		color: #ffffff; /* White for no change */
	}
</style>
