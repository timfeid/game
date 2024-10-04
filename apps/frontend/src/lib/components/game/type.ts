export type EffectTarget =
	| {
			name: 'player';
			index: number;
	  }
	| {
			name: 'card';
			player_index: number;
			card_index: number;
	  };
