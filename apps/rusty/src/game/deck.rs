use rand::seq::SliceRandom;
use rand::thread_rng;
use std::borrow::BorrowMut;
use std::sync::Arc;
use std::vec::Vec;
use tokio::sync::Mutex;

use crate::game::action::generate_mana::GenerateManaAction;
use crate::game::action::{
    ActionTriggerType, CardActionTarget, CardActionTrigger, CardRequiredTarget, CounterSpellAction,
    DrawCardAction, PlayerActionTarget, ReturnToHandAction,
};
use crate::game::card::card::create_creature_card;
use crate::game::card::{CardPhase, CardType};
use crate::game::effects::{
    ApplyEffectToCardBasedOnTotalCardType, ApplyEffectToPlayerCardType, ApplyEffectToTargetAction,
    DrawCardCardAction, ExpireContract, StatModifierEffect,
};
use crate::game::mana::ManaType;
use crate::game::stat::StatType;

use crate::game::action::{DeclareAttackerAction, DeclareBlockerAction};
use crate::game::card::Card;
use crate::game::stat::Stat;
use crate::game::turn::TurnPhase;

use super::player::Player;

#[derive(Debug, Default)]
pub struct Deck {
    pub draw_pile: Vec<Arc<Mutex<Card>>>,
    pub discard_pile: Vec<Arc<Mutex<Card>>>,
    pub destroyed_pile: Vec<Arc<Mutex<Card>>>,
    pub in_game: Vec<Arc<Mutex<Card>>>,
}

impl Deck {
    pub fn new(cards: Vec<Card>) -> Self {
        Self {
            draw_pile: cards.into_iter().map(|c| Arc::new(Mutex::new(c))).collect(),
            discard_pile: vec![],
            destroyed_pile: vec![],
            in_game: vec![],
        }
    }

    // Shuffle the draw pile
    pub fn shuffle(&mut self) {
        self.draw_pile.shuffle(&mut thread_rng());
    }

    // Draw a card from the draw pile, or shuffle the discard pile back in
    pub fn draw(&mut self) -> Option<Arc<Mutex<Card>>> {
        if let Some(card) = self.draw_pile.pop() {
            self.in_game.push(card.clone()); // Add to in-game pile
            Some(card)
        } else {
            None // No more cards to draw
        }
    }

    // Discard a card
    pub fn discard(&mut self, card: Arc<Mutex<Card>>) {
        self.discard_pile.push(card);
    }

    // Destroy a card
    pub fn destroy(&mut self, card: Arc<Mutex<Card>>) {
        self.destroyed_pile.push(card);
    }

    pub fn elsewhere(&mut self, card: Arc<Mutex<Card>>) {
        self.in_game.push(card);
    }

    // Reshuffle discard pile back into the draw pile
    fn reshuffle_discard_pile(&mut self) {
        self.draw_pile.append(&mut self.discard_pile);
        self.shuffle();
    }

    pub async fn set_owner(&self, player: &Arc<Mutex<Player>>) {
        for card in self.draw_pile.iter() {
            let mut d = card.lock().await;
            d.owner = Some(Arc::clone(player));
        }
    }
}

impl Deck {
    pub fn create_blue_deck() -> Vec<Card> {
        vec![
            // 12 Blue Creatures
            create_creature_card!(
                "Sapphire Adept",
                "Adept in manipulating water magic.",
                1,
                1,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Mystic Apprentice",
                "Learns spells as the game progresses.",
                1,
                1,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Illusionary Phantasm",
                "A creature that's hard to hit.",
                2,
                3,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Wind Weaver",
                "Can tap to draw a card.",
                1,
                2,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Spellcaster Adept",
                "Boosts your spells' effectiveness.",
                2,
                2,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Aether Sprite",
                "Has flying and evades ground creatures.",
                1,
                1,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Tidal Elemental",
                "Can return a creature to its owner's hand.",
                3,
                3,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Mind Manipulator",
                "Can control an opponent's creature temporarily.",
                2,
                2,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Sea Serpent",
                "A powerful creature from the depths.",
                5,
                5,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Arcane Scholar",
                "Draws an extra card each turn.",
                1,
                3,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Frost Titan",
                "Freezes enemy creatures when it attacks.",
                6,
                6,
                [ManaType::Blue]
            ),
            create_creature_card!(
                "Master of Waves",
                "Boosts other blue creatures.",
                2,
                1,
                [ManaType::Blue]
            ),
            // 12 Islands (Lands)
            Card::new(
                "Island",
                "TAP: Adds 1 blue mana to your pool.",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Blue],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Blue),
                vec![],
                vec![],
            ),
            // 12 Blue Instants and Sorceries (Control Spells)
            Card::new(
                "Counter Spell",
                "Counter target spell.",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Instant,
                    CardRequiredTarget::Spell,
                    Arc::new(CounterSpellAction {}),
                )],
                CardPhase::Ready,
                CardType::Instant,
                vec![],
                vec![ManaType::Blue, ManaType::Blue],
            ),
            Card::new(
                "Unsummon",
                "Return target creature to its owner's hand.",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Instant,
                    CardRequiredTarget::CardOfType(CardType::Creature),
                    Arc::new(ReturnToHandAction {}),
                )],
                CardPhase::Ready,
                CardType::Instant,
                vec![],
                vec![ManaType::Blue],
            ),
            Card::new(
                "Divination",
                "Draw two cards.",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(DrawCardCardAction {
                            target: CardActionTarget::SelfOwner,
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(DrawCardCardAction {
                            target: CardActionTarget::SelfOwner,
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Sorcery,
                vec![],
                vec![ManaType::Blue, ManaType::Colorless],
            ),
            // Card::new(
            //     "Frost Breath",
            //     "Tap up to two target creatures. They don't untap during their controller's next untap step.",
            //     vec![CardActionTrigger::new(
            //         ActionTriggerType::Instant,
            //         CardRequiredTarget::MultipleCardsOfType(CardType::Creature, 2),
            //         Arc::new(TapAndFreezeAction {
            //             duration: 1,
            //         }),
            //     )],
            //     CardPhase::Ready,
            //     CardType::Instant,
            //     vec![],
            //     vec![ManaType::Blue, ManaType::Colorless],
            // ),
            // Card::new(
            //     "Mind Control",
            //     "Gain control of target creature.",
            //     vec![CardActionTrigger::new(
            //         ActionTriggerType::Sorcery,
            //         CardRequiredTarget::CardOfType(CardType::Creature),
            //         Arc::new(GainControlAction {}),
            //     )],
            //     CardPhase::Ready,
            //     CardType::Enchantment,
            //     vec![],
            //     vec![ManaType::Blue, ManaType::Blue, ManaType::Colorless],
            // ),
        ]
    }

    pub fn create_green_deck() -> Vec<Card> {
        vec![
            // 12 Green Creatures
            create_creature_card!("Llanowar Elves", "", 1, 1, [ManaType::Green]),
            create_creature_card!("Elvish Mystic", "", 1, 1, [ManaType::Green]),
            create_creature_card!(
                "Kalonian Tusker",
                "Big creature with raw power",
                3,
                3,
                [ManaType::Green]
            ),
            create_creature_card!(
                "Voracious Hydra",
                "Hydra that grows with X mana",
                0,
                1,
                [ManaType::Green]
            ),
            create_creature_card!(
                "Steel Leaf Champion",
                "Cannot be blocked by creatures with power 2 or less",
                5,
                4,
                [ManaType::Green]
            ),
            create_creature_card!(
                "Wildgrowth Walker",
                "Gains life whenever you explore",
                1,
                3,
                [ManaType::Green]
            ),
            create_creature_card!(
                "Bristling Boar",
                "Can't be blocked by more than one creature",
                4,
                3,
                [ManaType::Green]
            ),
            create_creature_card!(
                "Pelt Collector",
                "Grows in power when other creatures enter",
                1,
                1,
                [ManaType::Green]
            ),
            create_creature_card!(
                "Prized Unicorn",
                "All creatures able to block it must do so",
                2,
                2,
                [ManaType::Green]
            ),
            create_creature_card!(
                "Thorn Lieutenant",
                "Gets +2/+2 when targeted",
                2,
                3,
                [ManaType::Green]
            ),
            create_creature_card!(
                "Nessian Boar",
                "Must be blocked if able",
                10,
                6,
                [ManaType::Green]
            ),
            create_creature_card!(
                "Gigantosaurus",
                "Massive creature with raw power",
                10,
                10,
                [ManaType::Green]
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            Card::new(
                "Forest",
                "TAP: Adds 1 forest mana to your pool",
                vec![CardActionTrigger::new(
                    ActionTriggerType::Tap,
                    CardRequiredTarget::None,
                    Arc::new(GenerateManaAction {
                        mana_to_add: vec![ManaType::Green],
                        target: PlayerActionTarget::SelfPlayer,
                    }),
                )],
                CardPhase::Ready,
                CardType::BasicLand(ManaType::Green),
                vec![],
                vec![],
            ),
            // 12 Green Sorceries and Instants (Buffs, Removal)
            Card::new(
                "Giant Growth",
                "Target creature gets +3/+3 until end of turn",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToTargetAction {
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Damage,
                                    3,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToTargetAction {
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Defense,
                                    3,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Instant,
                vec![],
                vec![ManaType::Green],
            ),
            Card::new(
                "Overrun",
                "Creatures you control get +3/+3 and trample",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToPlayerCardType {
                            card_type: CardType::Creature,
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Damage,
                                    3,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToPlayerCardType {
                            card_type: CardType::Creature,
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Defense,
                                    3,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(ApplyEffectToPlayerCardType {
                            card_type: CardType::Creature,
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Trample,
                                    1,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Sorcery,
                vec![],
                vec![ManaType::Green],
            ),
            // Card::new(
            //     "Nature's Spiral",
            //     "Return a permanent from your graveyard to your hand",
            //     vec![
            //         CardActionTrigger::new(ActionTriggerType::Instant, Arc::new(ReturnFromGraveyardAction {})),
            //     ],
            //     CardPhase::Ready,
            //     CardType::Sorcery,
            //     vec![],
            //     vec![ManaType::Green],
            // ),
            // Card::new(
            //     "Rabid Bite",
            //     "Target creature you control deals damage equal to its power to target creature you don't control",
            //     vec![
            //         CardActionTrigger::new(ActionTriggerType::Instant, Arc::new(FightAction {
            //             target: CardActionTarget::EffectTarget
            //         })),
            //     ],
            //     CardPhase::Ready,
            //     CardType::Instant,
            //     vec![],
            //     vec![ManaType::Green],
            // ),
            // Card::new(
            //     "Prey Upon",
            //     "Target creature you control fights target creature you don't control",
            //     vec![
            //         CardActionTrigger::new(ActionTriggerType::Instant, Arc::new(FightAction {
            //             target: CardActionTarget::EffectTarget
            //         })),
            //     ],
            //     CardPhase::Ready,
            //     CardType::Sorcery,
            //     vec![],
            //     vec![ManaType::Green],
            // ),
            Card::new(
                "Harmonize",
                "Draw three cards",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(DrawCardCardAction {
                            target: CardActionTarget::SelfOwner,
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(DrawCardCardAction {
                            target: CardActionTarget::SelfOwner,
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::None,
                        Arc::new(DrawCardCardAction {
                            target: CardActionTarget::SelfOwner,
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Sorcery,
                vec![],
                vec![ManaType::Green],
            ),
            Card::new(
                "Hunter's Prowess",
                "Target creature gets +3/+3 and trample until end of turn",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToTargetAction {
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Damage,
                                    3,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Instant,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToTargetAction {
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Trample,
                                    1,
                                    ExpireContract::Turns(1),
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Sorcery,
                vec![],
                vec![ManaType::Green],
            ),
            // 12 Enchantments or Other
            Card::new(
                "Rancor",
                "Enchanted creature gets +2/+0 and trample",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Attached,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToTargetAction {
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Trample,
                                    1,
                                    ExpireContract::Never,
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Attached,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToTargetAction {
                            effect_generator: Arc::new(|target, source_card| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Damage,
                                    2,
                                    ExpireContract::Never,
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Enchantment,
                vec![],
                vec![ManaType::Green],
            ),
            Card::new(
                "Blanchwood Armor",
                "Enchanted creature gets +1/+1 for each Forest you control",
                vec![
                    CardActionTrigger::new(
                        ActionTriggerType::Attached,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToCardBasedOnTotalCardType {
                            card_type: CardType::BasicLand(ManaType::Green),
                            effect_generator: Arc::new(|target, source_card, total| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Defense,
                                    total,
                                    ExpireContract::Never,
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                    CardActionTrigger::new(
                        ActionTriggerType::Attached,
                        CardRequiredTarget::CardOfType(CardType::Creature),
                        Arc::new(ApplyEffectToCardBasedOnTotalCardType {
                            card_type: CardType::BasicLand(ManaType::Green),
                            effect_generator: Arc::new(|target, source_card, total| {
                                let effect = StatModifierEffect::new(
                                    target,
                                    StatType::Damage,
                                    total,
                                    ExpireContract::Never,
                                    source_card,
                                );
                                Arc::new(Mutex::new(effect))
                            }),
                        }),
                    ),
                ],
                CardPhase::Ready,
                CardType::Enchantment,
                vec![],
                vec![ManaType::Green],
            ),
            // Card::new(
            //     "Gift of Paradise",
            //     "Enchanted land gains 'Tap: Add two mana of any one color'",
            //     vec![CardActionTrigger::new(
            //         ActionTriggerType::Attached,
            //         Arc::new(GenerateManaAction {
            //             mana_to_add: vec![ManaType::Green, ManaType::Green],
            //             target: PlayerActionTarget::SelfPlayer,
            //         }),
            //     )],
            //     CardPhase::Ready,
            //     CardType::Enchantment,
            //     vec![],
            //     vec![ManaType::Green],
            // ),
            // ...more enchantments
        ]
    }
}
