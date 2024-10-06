use crate::game::{
    action::{
        AsyncClosureWithCardAction, DeclareAttackerAction, DeclareBlockerAction, TriggerTarget,
    },
    card::CreatureType,
    effects::{ApplyDynamicEffectToCard, Effect, EffectTarget},
    player::Player,
    stat::{Stat, Stats},
    turn::TurnPhase,
    Game,
};
use std::{f32::consts::E, future::Future, pin::Pin, sync::Arc};

use tokio::sync::Mutex;
use ulid::Ulid;

use super::{
    action::{
        generate_mana::GenerateManaAction, ActionTriggerType, CardActionTrigger,
        CardRequiredTarget, PlayerActionTarget,
    },
    card::{card::create_creature_card, Card, CardPhase, CardType},
    effects::{ApplyEffectToCardBasedOnTotalCardType, DynamicStatModifierEffect, ExpireContract},
    mana::ManaType,
    stat::StatType,
};

fn create_mana() -> Card {
    Card::new(
        "Forest",
        "TAP: Adds 1 green mana to your pool.",
        vec![CardActionTrigger::new(
            ActionTriggerType::CardTapped,
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
    )
}

pub fn create_angels_deck() -> Vec<Card> {
    vec![
        create_creature_card!(
            "another angel",
            CreatureType::Angel,
            "an angel",
            2,
            2,
            [],
            [StatType::Flying]
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
                        effect_generator: Arc::new(|target, source_card, amount_calculator| {
                            Arc::new(Mutex::new(DynamicStatModifierEffect::new(
                                target,
                                StatType::Power,
                                amount_calculator.clone(),
                                ExpireContract::Never,
                                source_card.clone(),
                                false,
                            )))
                        }),
                    }),
                ),
                CardActionTrigger::new(
                    ActionTriggerType::Attached,
                    CardRequiredTarget::CardOfType(CardType::Creature),
                    Arc::new(ApplyEffectToCardBasedOnTotalCardType {
                        card_type: CardType::BasicLand(ManaType::Green),

                        effect_generator: Arc::new(|target, source_card, amount_calculator| {
                            Arc::new(Mutex::new(DynamicStatModifierEffect::new(
                                target,
                                StatType::Toughness,
                                amount_calculator.clone(),
                                ExpireContract::Never,
                                source_card.clone(),
                                false,
                            )))
                        }),
                    }),
                ),
            ],
            CardPhase::Ready,
            CardType::Enchantment,
            vec![],
            vec![],
        ),
        Card::new(
            "Forest",
            "TAP: Adds 1 green mana to your pool.",
            vec![CardActionTrigger::new(
                ActionTriggerType::CardTapped,
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
    ]
}

fn create_creature_card() -> Card {
    create_creature_card!(
        "Sapphire Adept",
        CreatureType::None,
        "Adept in manipulating water magic.",
        0,
        0,
        [],
        []
    )
}

fn create_enchantment() -> Card {
    Card::new(
        "Blanchwood Armor",
        "Enchanted creature gets +1/+1 for each Forest you control",
        vec![
            CardActionTrigger::new(
                ActionTriggerType::Attached,
                CardRequiredTarget::CardOfType(CardType::Creature),
                Arc::new(ApplyEffectToCardBasedOnTotalCardType {
                    card_type: CardType::BasicLand(ManaType::Green),
                    effect_generator: Arc::new(|target, source_card, amount_calculator| {
                        Arc::new(Mutex::new(DynamicStatModifierEffect::new(
                            target,
                            StatType::Power,
                            amount_calculator.clone(),
                            ExpireContract::Never,
                            source_card.clone(),
                            false,
                        )))
                    }),
                }),
            ),
            CardActionTrigger::new(
                ActionTriggerType::Attached,
                CardRequiredTarget::CardOfType(CardType::Creature),
                Arc::new(ApplyEffectToCardBasedOnTotalCardType {
                    card_type: CardType::BasicLand(ManaType::Green),

                    effect_generator: Arc::new(|target, source_card, amount_calculator| {
                        Arc::new(Mutex::new(DynamicStatModifierEffect::new(
                            target,
                            StatType::Toughness,
                            amount_calculator.clone(),
                            ExpireContract::Never,
                            source_card.clone(),
                            false,
                        )))
                    }),
                }),
            ),
        ],
        CardPhase::Ready,
        CardType::Enchantment,
        vec![],
        vec![],
    )
}

mod test {
    use std::sync::Arc;

    use tokio::sync::{Mutex, RwLock};

    use crate::game::{
        decks::Deck,
        effects::EffectTarget,
        mana,
        player::Player,
        test::{create_angels_deck, create_creature_card, create_enchantment, create_mana},
        Game,
    };
}
