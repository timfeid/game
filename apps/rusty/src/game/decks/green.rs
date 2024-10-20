use crate::{
    game::{
        action::{
            generate_mana::GenerateManaAction, Action, ActionTriggerType, ApplyDynamicEffectToCard,
            ApplyEffectToPlayerCardType, ApplyEffectToTargetAction,
            ApplyEffectsToPlayerCreatureType, AsyncClosureAction,
            AsyncClosureActionWithTargetAndAbility, AsyncClosureWithCardAction, BlankAction,
            CardAction, CardActionTarget, CardActionTrigger, CardActionWrapper, CardRequiredTarget,
            CardTargetTeam, CastMandatoryAdditionalAbility, CastOptionalAdditionalAbility,
            ChooseFromSelectionAction, DamageTarget, DeclareAttackerAction, DeclareBlockerAction,
            DrawCardAction, DrawCardCardAction, PlayCardAction, PlayerActionTarget,
            ReturnToHandAction, TapCardAction, TriggerTarget,
        },
        card::{
            card::{create_creature_card, create_multiple_cards},
            Card, CardPhase, CardType, Counter, CreatureType,
        },
        decks::duplicate_card,
        effects::{
            DynamicStatModifierEffect, Effect, EffectID, EffectTarget, ExpireContract,
            ModifyStatTarget, StatModifierEffect,
        },
        mana::ManaType,
        player::Player,
        stat::{Stat, StatType, StaticStatId, Stats},
        turn::TurnPhase,
        ActionType, CardWithDetails, Game,
    },
    lobby::manager::{CardSelectionDetails, LobbyCommand},
};
use std::{f32::consts::E, future::Future, mem::zeroed, pin::Pin, sync::Arc};

use tokio::sync::Mutex;
use ulid::Ulid;

fn create_test_forest() -> Card {
    Card::new(
        "Forest",
        "",
        vec![
            CardActionTrigger::new(
                ActionTriggerType::CardPlayedFromHand(Some((
                    vec![
                        TurnPhase::Untap,
                        TurnPhase::Upkeep,
                        TurnPhase::Draw,
                        TurnPhase::Main,
                        TurnPhase::BeginningOfCombat,
                        TurnPhase::DeclareAttackers,
                        TurnPhase::DeclareBlockers,
                        TurnPhase::CombatDamage,
                        TurnPhase::EndOfCombat,
                        TurnPhase::Main2,
                        TurnPhase::End,
                        TurnPhase::Cleanup,
                    ],
                    TriggerTarget::Owner,
                ))),
                CardRequiredTarget::None,
                Arc::new(BlankAction {}),
            ),
            CardActionTrigger::new(
                ActionTriggerType::AbilityWithinPhases(
                    "Add 1 {G} to your pool".to_string(),
                    vec![],
                    None,
                    true,
                ),
                CardRequiredTarget::None,
                Arc::new(GenerateManaAction {
                    mana_to_add: vec![
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                        ManaType::Green,
                    ],
                    target: PlayerActionTarget::Owner,
                }),
            ),
        ],
        CardPhase::Ready,
        CardType::BasicLand(ManaType::Green),
        vec![],
        vec![],
    )
}

fn create_forest() -> Card {
    Card::new(
        "Forest",
        "",
        vec![
            CardActionTrigger::new(
                ActionTriggerType::CardPlayedFromHand(Some((
                    vec![
                        TurnPhase::Untap,
                        TurnPhase::Upkeep,
                        TurnPhase::Draw,
                        TurnPhase::Main,
                        TurnPhase::BeginningOfCombat,
                        TurnPhase::DeclareAttackers,
                        TurnPhase::DeclareBlockers,
                        TurnPhase::CombatDamage,
                        TurnPhase::EndOfCombat,
                        TurnPhase::Main2,
                        TurnPhase::End,
                        TurnPhase::Cleanup,
                    ],
                    TriggerTarget::Owner,
                ))),
                CardRequiredTarget::None,
                Arc::new(BlankAction {}),
            ),
            CardActionTrigger::new(
                ActionTriggerType::AbilityWithinPhases(
                    "Add 1 {G} to your pool".to_string(),
                    vec![],
                    None,
                    true,
                ),
                CardRequiredTarget::None,
                Arc::new(GenerateManaAction {
                    mana_to_add: vec![ManaType::Green],
                    target: PlayerActionTarget::Owner,
                }),
            ),
        ],
        CardPhase::Ready,
        CardType::BasicLand(ManaType::Green),
        vec![],
        vec![],
    )
}

pub fn create_devoted_druid() -> Card {
    create_creature_card!(
        "Devoted Druid",
        CreatureType::Elf,
        "",
        0,
        2,
        [ManaType::Colorless, ManaType::Green],
        [],
        CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Add {G} to your mana pool.".to_string(),
                vec![],
                None,
                true
            ),
            CardRequiredTarget::None,
            Arc::new(GenerateManaAction {
                mana_to_add: vec![ManaType::Green],
                target: PlayerActionTarget::Owner
            })
        ),
        CardActionTrigger::new_with_requirements(
            ActionTriggerType::AbilityWithinPhases(
                "Put a -1/-1 counter on Devoted Druid: Untap Devoted Druid".to_string(),
                vec![],
                None,
                false
            ),
            CardRequiredTarget::None,
            Arc::new(AsyncClosureAction::new(Arc::new(
                |game: Arc<Mutex<Game>>,
                 card: Arc<Mutex<Card>>|
                 -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                    Box::pin(async move {
                        Card::add_counter(
                            card.clone(),
                            &game,
                            Counter::PowerToughnessModifier(-1, -1),
                        )
                        .await;
                        card.lock().await.untap();
                        Ok(())
                    })
                }
            ))),
            Arc::new(
                |game: Arc<Mutex<Game>>,
                 card: Arc<Mutex<Card>>,
                 ability_id: String|
                 -> Pin<Box<dyn Future<Output = bool> + Send>> {
                    Box::pin(async move {
                        if let Ok(card) = card.try_lock() {
                            return card.tapped;
                        }

                        false
                    })
                }
            )
        )
    )
}

pub fn create_elvish_warmaster() -> Card {
    create_creature_card!(
        "Elvish Warmaster",
        CreatureType::Elf,
        "Whenever one or more other Elves you control enter, create a 1/1 green Elf Warrior creature token. This ability triggers only once each turn.",
        2,
        2,
        [ManaType::Colorless, ManaType::Green],
        [],
        CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Elves you control get +2/+2 and gain deathtouch until end of turn.".to_string(),
                vec![
                    ManaType::Colorless,
                    ManaType::Colorless,
                    ManaType::Colorless,
                    ManaType::Colorless,
                    ManaType::Colorless,
                    ManaType::Green,
                    ManaType::Green,
                ],
                None,
                false,
            ),
            CardRequiredTarget::None,
            Arc::new(ApplyEffectsToPlayerCreatureType::new(
                CreatureType::Elf,
                Arc::new(
                    move |target,
                          source_card,
                          effect_id|
                          -> Pin<
                        Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
                    > {
                        Box::pin(async move {
                            let mut effects: Vec<Arc<Mutex<dyn Effect + Send + Sync>>> = vec![];
                            if let EffectTarget::Card(card) = &target {
                                if !Arc::ptr_eq(card, &source_card) {
                                    effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                        target: target.clone(),
                                        stat_type: StatType::Deathtouch,
                                        amount: 1,
                                        expires: ExpireContract::Turns(1),
                                        id: EffectID(format!(
                                            "{}-{}-deathtouch",
                                            source_card.clone().lock().await.id,
                                            effect_id
                                        )),
                                        applied: false,
                                        source_card: Some(source_card.clone()),
                                        previous_turn: None,
                                    })));
                                    effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                        target: target.clone(),
                                        stat_type: StatType::Power,
                                        amount: 2,
                                        expires: ExpireContract::Turns(1),
                                        id: EffectID(format!(
                                            "{}-{}-power",
                                            source_card.clone().lock().await.id,
                                            effect_id
                                        )),
                                        applied: false,
                                        source_card: Some(source_card.clone()),
                                        previous_turn: None,
                                    })));
                                    effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                        target: target.clone(),
                                        stat_type: StatType::Toughness,
                                        amount: 2,
                                        expires: ExpireContract::Turns(1),
                                        id: EffectID(format!(
                                            "{}-{}-toughness",
                                            source_card.clone().lock().await.id,
                                            effect_id
                                        )),
                                        applied: false,
                                        source_card: Some(source_card.clone()),
                                        previous_turn: None,
                                    })));
                                }
                            }
                            effects
                        })
                    },
                ),
            )),
        ),

        CardActionTrigger::new(
            ActionTriggerType::CreatureTypeCardPlayed(TriggerTarget::Owner, CreatureType::Elf),
            CardRequiredTarget::None,
            Arc::new(AsyncClosureActionWithTargetAndAbility::new(Arc::new(
                |game: Arc<Mutex<Game>>, card: Arc<Mutex<Card>>, _target, ability_id| -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                    Box::pin(async move {
                        let (owner, played) = {
                            let card = card.lock().await;
                            let owner_arc = card.owner.clone().unwrap();
                            let played = game.lock().await.triggers_played_this_turn.get(ability_id.as_str()).unwrap_or(&0).clone();

                            (owner_arc, played)
                        };

                        if played == 1 {
                            Game::play_token(&game, &owner, create_creature_card!("Token", CreatureType::Elf, "", 1,1, [], [])).await.ok();
                        }
                        Ok(())
                    })
                }
            )))
        )
    )
}

pub fn create_llanowar_elves() -> Card {
    create_creature_card!(
        "Llanowar Elves",
        CreatureType::Elf,
        "",
        1,
        1,
        [ManaType::Green],
        [],
        CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Add {G} to your mana pool.".to_string(),
                vec![],
                None,
                true
            ),
            CardRequiredTarget::None,
            Arc::new(GenerateManaAction {
                mana_to_add: vec![ManaType::Green],
                target: PlayerActionTarget::Owner
            })
        )
    )
}

pub fn create_elvish_mystic() -> Card {
    create_creature_card!(
        "Elvish Mystic",
        CreatureType::Elf,
        "",
        1,
        1,
        [ManaType::Green],
        [],
        CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Add {G} to your mana pool.".to_string(),
                vec![],
                None,
                true
            ),
            CardRequiredTarget::None,
            Arc::new(GenerateManaAction {
                mana_to_add: vec![ManaType::Green],
                target: PlayerActionTarget::Owner
            })
        )
    )
}

pub fn create_elvish_archdruid() -> Card {
    create_creature_card!(
        "Elvish Archdruid",
        CreatureType::Elf,
        "Other Elf creatures you control get +1/+1.",
        1,
        1,
        [ManaType::Colorless, ManaType::Green, ManaType::Green],
        [],
        CardActionTrigger::new(
            ActionTriggerType::Continuous,
            CardRequiredTarget::None,
            Arc::new(ApplyEffectsToPlayerCreatureType::new(
                CreatureType::Elf,
                Arc::new(
                    move |target,
                          source_card,
                          effect_id|
                          -> Pin<
                        Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
                    > {
                        Box::pin(async move {
                            let mut effects: Vec<Arc<Mutex<dyn Effect + Send + Sync>>> = vec![];
                            if let EffectTarget::Card(card) = &target {
                                if !Arc::ptr_eq(card, &source_card) {
                                    effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                        target: target.clone(),
                                        stat_type: StatType::Power,
                                        amount: 1,
                                        expires: ExpireContract::Never,
                                        id: EffectID(format!(
                                            "{}-{}-power",
                                            source_card.clone().lock().await.id,
                                            effect_id
                                        )),
                                        applied: false,
                                        source_card: Some(source_card.clone()),
                                        previous_turn: None,
                                    })));
                                    effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                        target: target.clone(),
                                        stat_type: StatType::Toughness,
                                        amount: 1,
                                        expires: ExpireContract::Never,
                                        id: EffectID(format!(
                                            "{}-{}-toughness",
                                            source_card.clone().lock().await.id,
                                            effect_id
                                        )),
                                        applied: false,
                                        source_card: Some(source_card.clone()),
                                        previous_turn: None,
                                    })));
                                }
                            }
                            effects
                        })
                    },
                ),
            )),
        ),
        CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Add {G} for each Elf on the battlefield.".to_string(),
                vec![],
                None,
                true
            ),
            CardRequiredTarget::None,
            Arc::new(AsyncClosureAction::new(Arc::new(
                |game: Arc<Mutex<Game>>,
                 card: Arc<Mutex<Card>>|
                 -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                    Box::pin(async move {
                        let owner = card.lock().await.owner.clone().unwrap();
                        let cards_in_play = &owner.lock().await.cards_in_play.clone();
                        for card in cards_in_play {
                            if card.lock().await.creature_type == Some(CreatureType::Elf) {
                                owner.lock().await.mana_pool.add_mana(ManaType::Green);
                            }
                        }
                        Ok(())
                    })
                }
            )))
        )
    )
}

pub fn create_priest_of_titania() -> Card {
    create_creature_card!(
        "Priest of Titania",
        CreatureType::Elf,
        "",
        1,
        1,
        [ManaType::Colorless, ManaType::Green],
        [],
        CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Add {G} for each Elf on the battlefield.".to_string(),
                vec![],
                None,
                true
            ),
            CardRequiredTarget::None,
            Arc::new(AsyncClosureAction::new(Arc::new(
                |game: Arc<Mutex<Game>>,
                 card: Arc<Mutex<Card>>|
                 -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                    Box::pin(async move {
                        let owner = card.lock().await.owner.clone().unwrap();
                        let cards_in_play = &owner.lock().await.cards_in_play.clone();
                        for card in cards_in_play {
                            if card.lock().await.creature_type == Some(CreatureType::Elf) {
                                owner.lock().await.mana_pool.add_mana(ManaType::Green);
                            }
                        }
                        Ok(())
                    })
                }
            )))
        )
    )
}

fn regenerate_target_card() -> Arc<AsyncClosureWithCardAction> {
    Arc::new(AsyncClosureWithCardAction::new(Arc::new(
        |game: Arc<Mutex<Game>>,
         source: Arc<Mutex<Card>>,
         target_card: Arc<Mutex<Card>>|
         -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
            Box::pin(async move {
                let mut card = target_card.lock().await;
                card.add_stat(
                    StaticStatId::Regenerate.to_string(),
                    Stat::new(StatType::Regenerate, 1),
                );
                Ok(())
            })
        },
    )))
}

pub fn create_eladamri_korvecdal() -> Card {
    create_creature_card!(
        "Eladamri, Korvecdal",
        CreatureType::Elf,
        "You may look at the top card of your library any time.\nYou may cast creature spells from the top of your library.",
        2,
        2,
        [ManaType::Colorless, ManaType::Green, ManaType::Green],
        [],
        CardActionTrigger::new_with_requirements(
            ActionTriggerType::AbilityWithinPhases(
                "Tap two untapped creatures you control: Reveal a card from your hand or the top card of your library. If you reveal a creature card this way, put it onto the battlefield. Activate only during your turn.".to_string(),
                vec![ManaType::Green],
                None,
                true
            ),
            CardRequiredTarget::None,
            Arc::new(CastMandatoryAdditionalAbility {
                action_type: ActionType::None,
                mana: vec![],
                target: CardRequiredTarget::CardOfType(
                    CardType::Creature,
                    CardTargetTeam::Owner,
                    Some(true)
                ),
                description: "Tap untapped creature you control".to_string(),
                ability: Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                    Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                        |game: Arc<Mutex<Game>>,
                         source_card: Arc<Mutex<Card>>,
                         target: Arc<Mutex<Card>>|
                         -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                            Box::pin(async move {
                                {
                                    target.lock().await.tapped = true;
                                }

                                game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                        ability_id: None,
                                        card: source_card,
                                        action: Arc::new(CastMandatoryAdditionalAbility {
                                            action_type: ActionType::None,
                                            mana: vec![],
                                            target: CardRequiredTarget::CardOfType(
                                                CardType::Creature,
                                                CardTargetTeam::Owner,
                                                Some(true)
                                            ),
                                            description: "Tap untapped creature you control".to_string(),
                                            ability: Arc::new(|_| -> Arc<dyn CardAction + Send + Sync> {
                                                Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                                    |game: Arc<Mutex<Game>>,
                                                    source_card: Arc<Mutex<Card>>,
                                                    target: Arc<Mutex<Card>>|
                                                    -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                                        Box::pin(async move {
                                                            { target.lock().await.tapped = true; }

                                                            let owner = source_card.lock().await.owner.clone().unwrap();
                                                            let mut cards = vec![];
                                                            let mut valid_card_indexes = vec![];

                                                            let player_id = owner.lock().await.name.clone();
                                                            let deck = owner.lock().await.deck.draw_pile.clone();
                                                            let hand = owner.lock().await.cards_in_hand.clone();
                                                            for (index, card) in hand.iter().enumerate() {
                                                                cards.push(CardWithDetails::from_card_arc(Arc::clone(card), &game).await);
                                                                valid_card_indexes.push(index as i32)
                                                            }
                                                            if deck.len() > 0 {

                                                                cards.push(CardWithDetails::from_card_arc(Arc::clone(&deck[0]), &game).await);
                                                                valid_card_indexes.push((cards.len() -1) as i32)
                                                            }

                                                            game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                                                    ability_id: None,
                                                                    card: source_card,
                                                                    action: Arc::new(ChooseFromSelectionAction::new(player_id, cards, valid_card_indexes, Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                                                Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                                |game: Arc<Mutex<Game>>,
                                                source_card: Arc<Mutex<Card>>,
                                                target: Arc<Mutex<Card>>|
                                                -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                                    Box::pin(async move {
                                                        println!("source????? {:?}", source_card);
                                                        let card_type = { target.lock().await.card_type };
                                                        if card_type == CardType::Creature {

                                                            let position = game.lock().await.frontend_target_from_card(&target).await;
                                                            Game::play_card_without_mana(&game, position).await.expect("oh wow we are here?");
                                                        } else {
                                                            println!("we need to reveal this bitch");
                                                        }
                                                        Ok(())
                                                })
                                            })))
                                            }))) ,
                                                                    target: None
                                                                })]).await?;
                                                        Ok(())
                                                        })
                                                    },
                                                )))
                                            })
                                        }) ,
                                        target: None
                                    })]).await?;
                                Ok(())
                            })
                        },
                    )))
                })
            }),
            Arc::new(
                |game: Arc<Mutex<Game>>,
                 card: Arc<Mutex<Card>>,
                 ability_id|
                 -> Pin<Box<dyn Future<Output = bool> + Send>> {
                    Box::pin(async move {
                        let owner = {
                            if let Ok(card_l) = card.try_lock() {
                                card_l.owner.clone()
                            } else {
                                None
                            }
                        };

                        if let Some(owner) = owner {
                            let untapped_count = {
                                owner
                                            .lock()
                                            .await
                                            .filter_cards_in_play(Arc::new(
                                                move |card_arc: Arc<Mutex<Card>>| -> Pin<
                                                    Box<dyn Future<Output = bool> + Send>,
                                                > {
                                                    Box::pin(async move {
                                                        if let Ok(card) = card_arc.try_lock() {
                                                            return card.is_tappable() && card.card_type == CardType::Creature;
                                                        }

                                                        false
                                                    })
                                                },
                                            ))
                                            .await
                            };
                            return untapped_count.len() > 2;
                        }
                        println!("not ready yet {:?}", card);

                        false
                    })
                }
            )
        )
    )
}

pub fn create_ezuri() -> Card {
    create_creature_card!(
        "Ezuri, Renegade Leader",
        CreatureType::Elf,
        "",
        2,
        2,
        [ManaType::Colorless, ManaType::Green, ManaType::Green],
        [],
        CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Regenerate another target Elf.".to_string(),
                vec![ManaType::Green],
                None,
                false
            ),
            CardRequiredTarget::CreatureOfType(CreatureType::Elf, CardTargetTeam::Any, None),
            regenerate_target_card(),
        ),
        CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Elf creatures you control get +3/+3 and gain trample until end of turn."
                    .to_string(),
                vec![
                    ManaType::Colorless,
                    ManaType::Colorless,
                    ManaType::Green,
                    ManaType::Green,
                    ManaType::Green
                ],
                None,
                false
            ),
            CardRequiredTarget::None,
            Arc::new(ApplyEffectsToPlayerCreatureType::new(
                CreatureType::Elf,
                Arc::new(
                    move |target,
                          source_card,
                          effect_id|
                          -> Pin<
                        Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
                    > {
                        Box::pin(async move {
                            let mut effects: Vec<Arc<Mutex<dyn Effect + Send + Sync>>> = vec![];
                            effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                target: target.clone(),
                                stat_type: StatType::Power,
                                amount: 3,
                                expires: ExpireContract::Turns(1),
                                id: EffectID(format!(
                                    "{}-{}-power",
                                    source_card.clone().lock().await.id,
                                    effect_id
                                )),
                                applied: false,
                                source_card: Some(source_card.clone()),
                                previous_turn: None,
                            })));
                            effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                target: target.clone(),
                                stat_type: StatType::Toughness,
                                amount: 3,
                                expires: ExpireContract::Turns(1),
                                id: EffectID(format!(
                                    "{}-{}-toughness",
                                    source_card.clone().lock().await.id,
                                    effect_id
                                )),
                                applied: false,
                                source_card: Some(source_card.clone()),
                                previous_turn: None,
                            })));
                            effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                target: target.clone(),
                                stat_type: StatType::Trample,
                                amount: 1,
                                expires: ExpireContract::Turns(1),
                                id: EffectID(format!(
                                    "{}-{}-trample",
                                    source_card.clone().lock().await.id,
                                    effect_id
                                )),
                                applied: false,
                                source_card: Some(source_card.clone()),
                                previous_turn: None,
                            })));
                            effects
                        })
                    },
                ),
            )),
        )
    )
}

pub fn create_heritage_druid() -> Card {
    create_creature_card!(
        "Heritage Druid",
        CreatureType::Elf,
        "",
        1,
        1,
        [ManaType::Green],
        [],
        CardActionTrigger::new_with_requirements(
            ActionTriggerType::AbilityWithinPhases(
                "Tap three untapped Elves you control: Add {G}{G}{G}.".to_string(),
                vec![],
                None,
                false
            ),
            CardRequiredTarget::None,
            Arc::new(CastMandatoryAdditionalAbility {
                action_type: ActionType::None,
                mana: vec![],
                target: CardRequiredTarget::CreatureOfType(
                    CreatureType::Elf,
                    CardTargetTeam::Owner,
                    Some(true)
                ),
                description: "Tap an untapped Elf you control".to_string(),
                ability: Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                    Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                        |game: Arc<Mutex<Game>>,
                         source_card: Arc<Mutex<Card>>,
                         target: Arc<Mutex<Card>>|
                         -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                            Box::pin(async move {
                                {
                                    target.lock().await.tapped = true;
                                }

                                game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                        ability_id: None,
                                        card: source_card,
                                        action: Arc::new(CastMandatoryAdditionalAbility {
                                            action_type: ActionType::None,
                                            mana: vec![],
                                            target: CardRequiredTarget::CreatureOfType(
                                                CreatureType::Elf,
                                                CardTargetTeam::Owner,
                                                Some(true)
                                            ),
                                            description: "Tap an untapped Elf you control".to_string(),
                                            ability: Arc::new(|_| -> Arc<dyn CardAction + Send + Sync> {
                                                Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                                    |game: Arc<Mutex<Game>>,
                                                    source_card: Arc<Mutex<Card>>,
                                                    target: Arc<Mutex<Card>>|
                                                    -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                                        Box::pin(async move {
                                                            { target.lock().await.tapped = true; }

                                                            game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                                                    ability_id: None,
                                                                    card: source_card,
                                                                    action: Arc::new(CastMandatoryAdditionalAbility {
                                                                        action_type: ActionType::None,
                                                                        mana: vec![],
                                                                        target: CardRequiredTarget::CreatureOfType(
                                                                            CreatureType::Elf,
                                                                            CardTargetTeam::Owner,
                                                                            Some(true)
                                                                        ),
                                                                        description: "Tap an untapped Elf you control".to_string(),
                                                                        ability: Arc::new(|_| -> Arc<dyn CardAction + Send + Sync> {
                                                                            Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                                                                |game: Arc<Mutex<Game>>,
                                                                                source_card: Arc<Mutex<Card>>,
                                                                                target: Arc<Mutex<Card>>|
                                                                                -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                                                                    Box::pin(async move {
                                                                                        { target.lock().await.tapped = true; }

                                                                                        game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                                                                                ability_id: None,
                                                                                                card: source_card,
                                                                                                action: Arc::new(GenerateManaAction {mana_to_add: vec![ManaType::Green, ManaType::Green, ManaType::Green], target: PlayerActionTarget::Owner}) ,
                                                                                                target: None
                                                                                            })]).await?;
                                                                                            Ok(())
                                                                                    })
                                                                                },
                                                                            )))
                                                                        })
                                                                    }) ,
                                                                    target: None
                                                                })]).await?;
                                                        Ok(())
                                                        })
                                                    },
                                                )))
                                            })
                                        }) ,
                                        target: None
                                    })]).await?;
                                                        Ok(())
                            })
                        },
                    )))
                })
            }),
            Arc::new(
                |game: Arc<Mutex<Game>>,
                 card: Arc<Mutex<Card>>,
                 ability_id|
                 -> Pin<Box<dyn Future<Output = bool> + Send>> {
                    Box::pin(async move {
                        let owner = {
                            if let Ok(card_l) = card.try_lock() {
                                card_l.owner.clone()
                            } else {
                                None
                            }
                        };

                        if let Some(owner) = owner {
                            let untapped_count = {
                                owner
                                            .lock()
                                            .await
                                            .filter_cards_in_play(Arc::new(
                                                move |card_arc: Arc<Mutex<Card>>| -> Pin<
                                                    Box<dyn Future<Output = bool> + Send>,
                                                > {
                                                    Box::pin(async move {
                                                        if let Ok(card) = card_arc.try_lock() {
                                                            return card.is_tappable() && card.creature_type == Some(CreatureType::Elf)
                                                        }

                                                        false
                                                    })
                                                },
                                            ))
                                            .await
                            };
                            return untapped_count.len() > 2;
                        }
                        println!("not ready yet {:?}", card);

                        false
                    })
                }
            )
        )
    )
}

pub fn create_wirewood() -> Card {
    create_creature_card!(
        "Wirewood",
        CreatureType::Elf,
        "",
        1,
        1,
        [ManaType::Green],
        [],
        CardActionTrigger::new_with_requirements(
            ActionTriggerType::AbilityWithinPhases("Return an Elf you control to its owner's hand: Untap target creature. Activate only once each turn.".to_string(), vec![], None, false),
            CardRequiredTarget::None,
            Arc::new(CastMandatoryAdditionalAbility {
                action_type: ActionType::None,
                mana: vec![],
                target: CardRequiredTarget::CreatureOfType(CreatureType::Elf, CardTargetTeam::Owner, None),
                description: "Return an Elf you control to its owner's hand: Untap target creature. Activate only once each turn.".to_string(),
                ability: Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {

                    Arc::new(AsyncClosureAction::new(Arc::new(
                        |game: Arc<Mutex<Game>>, card: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                        Box::pin(async move {
                        let owner_arc = {
                            let card = card.lock().await;
                            card.owner.clone()
                        };

                        if let Some(owner_arc) = owner_arc {
                            {

                            let mut owner = owner_arc.lock().await;
                            if let Some(index) = owner.cards_in_play.iter().position(|c| Arc::ptr_eq(c, &card)) {
                                let card_arc = owner.cards_in_play.remove(index);
                                owner.cards_in_hand.push(card_arc.clone());
                            }
                        }


                                    game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                        card: card,
                                        ability_id: None,
                                        action: Arc::new(CastMandatoryAdditionalAbility {
                                            action_type: ActionType::None,
                                            mana: vec![],
                                            target: CardRequiredTarget::CardOfType(CardType::Creature, CardTargetTeam::Any, None),
                                            description:
                                                "Untap target creature"
                                                    .to_string(),
                                            ability: Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                                                Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                                    |game: Arc<Mutex<Game>>,
                                                    source: Arc<Mutex<Card>>,
                                                    card_played: Arc<Mutex<Card>>|
                                                    -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                                        Box::pin(async move {
                                                            let mut card = card_played.lock().await;
                                                            card.tapped = false;
                                                            Ok(())
                                                        })
                                                    },
                                                )))
                                                    as Arc<dyn CardAction + Send + Sync>
                                            })
                                        }) ,
                                        target: None
                                    })]).await?;
                        }

                    Ok(())
                    })
                },
            )))})}),
            Arc::new(
                |game: Arc<Mutex<Game>>, card: Arc<Mutex<Card>>, ability_id| -> Pin<Box<dyn Future<Output = bool> + Send>> {
                    Box::pin(async move {
                        if let Ok(card) = card.try_lock() {
                            if let Some(owner) = card.owner.as_ref() {
                                let creatures = { owner.lock().await.creatures_of_type(CreatureType::Elf).await };
                                let has_tappables = { game.lock().await.has_tapped_creature_excluding(&creatures).await};
                                return creatures.len() > 0 && has_tappables;
                            }
                        }
                        println!("not ready yet {:?}", card);

                        false
                    })
                }
            )

        )
    )
}

pub fn create_leaf_crowned_visionary() -> Card {
    create_creature_card!(
        "Leaf-Crowned Visionary",
        CreatureType::Elf,
        "Other Elves you control get +1/+1.\nWhenever you cast an Elf spell, you may pay {G}. If you do, draw a card.",
        1,
        1,
        [ManaType::Green, ManaType::Green],
        [],
        CardActionTrigger::new(
            ActionTriggerType::OtherCardPlayed(TriggerTarget::Owner),
            CardRequiredTarget::None,
            Arc::new(CastOptionalAdditionalAbility::new(
                 vec![ManaType::Green],
                 CardRequiredTarget::None,
                 Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                    Arc::new(DrawCardCardAction::one(CardActionTarget::SelfOwner))
                        as Arc<dyn CardAction + Send + Sync>
                }),
                "Whenever you cast an Elf spell, you may pay {G}. If you do, draw a card.".to_string(),
                 ActionType::None,
            ))
        ),
        CardActionTrigger::new(
            ActionTriggerType::Continuous,
            CardRequiredTarget::None,
            Arc::new(ApplyEffectsToPlayerCreatureType::new(
                CreatureType::Elf,
                Arc::new(
                    move |target,
                          source_card,
                          effect_id|
                          -> Pin<
                        Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
                    > {
                        Box::pin(async move {
                            let mut effects: Vec<Arc<Mutex<dyn Effect + Send + Sync>>> = vec![];
                            if let EffectTarget::Card(card) = &target {
                                if !Arc::ptr_eq(card, &source_card) {
                                    effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                        target: target.clone(),
                                        stat_type: StatType::Power,
                                        amount: 1,
                                        expires: ExpireContract::Never,
                                        id: EffectID(format!(
                                            "{}-{}-power",
                                            source_card.clone().lock().await.id,
                                            effect_id
                                        )),
                                        applied: false,
                                        source_card: Some(source_card.clone()),
                                        previous_turn: None,
                                    })));
                                    effects.push(Arc::new(Mutex::new(StatModifierEffect {
                                        target: target.clone(),
                                        stat_type: StatType::Toughness,
                                        amount: 1,
                                        expires: ExpireContract::Never,
                                        id: EffectID(format!(
                                            "{}-{}-toughness",
                                            source_card.clone().lock().await.id,
                                            effect_id
                                        )),
                                        applied: false,
                                        source_card: Some(source_card.clone()),
                                        previous_turn: None,
                                    })));
                                }
                            }
                            effects
                        })
                    },
                ),
            )),
        )
    )
}

pub fn create_cavern_of_souls() -> Card {
    Card::new(
        "Cavern of Souls",
        "",
        vec![
            CardActionTrigger::new(
                ActionTriggerType::CardPlayedFromHand(Some((
                    vec![
                        TurnPhase::Untap,
                        TurnPhase::Upkeep,
                        TurnPhase::Draw,
                        TurnPhase::Main,
                        TurnPhase::BeginningOfCombat,
                        TurnPhase::DeclareAttackers,
                        TurnPhase::DeclareBlockers,
                        TurnPhase::CombatDamage,
                        TurnPhase::EndOfCombat,
                        TurnPhase::Main2,
                        TurnPhase::End,
                        TurnPhase::Cleanup,
                    ],
                    TriggerTarget::Owner,
                ))),
                CardRequiredTarget::None,
                Arc::new(BlankAction {}),
            ),
            CardActionTrigger::new(
                ActionTriggerType::AbilityWithinPhases(
                    "Add {C} to your mana pool.".to_string(),
                    vec![],
                    None,
                    true,
                ),
                CardRequiredTarget::None,
                Arc::new(GenerateManaAction {
                    mana_to_add: vec![ManaType::Green],
                    target: PlayerActionTarget::Owner,
                }),
            ),
            CardActionTrigger::new(
                ActionTriggerType::AbilityWithinPhases("Add one mana of any color. Spend this mana only to cast a creature spell of the chosen type, and that spell can't be countered.".to_string(), vec![], None, true),
                CardRequiredTarget::None,
                Arc::new(GenerateManaAction {
                    mana_to_add: vec![ManaType::Green],
                    target: PlayerActionTarget::Owner,
                }),
            ),
        ],
        CardPhase::Ready,
        CardType::AdvancedLand(ManaType::Green),
        vec![],
        vec![],
    )
}

pub fn create_pendelhaven() -> Card {
    Card::new(
        "Pendelhaven",
        "",
        vec![
            CardActionTrigger::new(
                ActionTriggerType::CardPlayedFromHand(Some((
                    vec![
                        TurnPhase::Untap,
                        TurnPhase::Upkeep,
                        TurnPhase::Draw,
                        TurnPhase::Main,
                        TurnPhase::BeginningOfCombat,
                        TurnPhase::DeclareAttackers,
                        TurnPhase::DeclareBlockers,
                        TurnPhase::CombatDamage,
                        TurnPhase::EndOfCombat,
                        TurnPhase::Main2,
                        TurnPhase::End,
                        TurnPhase::Cleanup,
                    ],
                    TriggerTarget::Owner,
                ))),
                CardRequiredTarget::None,
                Arc::new(BlankAction {}),
            ),
            CardActionTrigger::new(
                ActionTriggerType::AbilityWithinPhases("Add {G}".to_string(), vec![], None, true),
                CardRequiredTarget::None,
                Arc::new(GenerateManaAction {
                    mana_to_add: vec![ManaType::Green],
                    target: PlayerActionTarget::Owner,
                }),
            ),
            CardActionTrigger::new(
                ActionTriggerType::AbilityWithinPhases(
                    "Target 1/1 creature gains +1/+2 until end of turn.".to_string(),
                    vec![],
                    None,
                    true,
                ),
                CardRequiredTarget::CreatureWithPowerAndToughness(1, 1, CardTargetTeam::Any),
                Arc::new(ApplyEffectToTargetAction::new(Arc::new(
                    |target, source_card| {
                        println!("target {:?}\n\nsource {:?}", target, source_card);
                        Box::pin(async move {
                            vec![
                                Arc::new(Mutex::new(StatModifierEffect::new(
                                    target.clone(),
                                    StatType::Power,
                                    1,
                                    ExpireContract::Turns(1),
                                    Some(source_card.clone()),
                                )))
                                    as Arc<Mutex<dyn Effect + Send + Sync>>,
                                Arc::new(Mutex::new(StatModifierEffect::new(
                                    target,
                                    StatType::Toughness,
                                    2,
                                    ExpireContract::Turns(1),
                                    Some(source_card.clone()),
                                ))),
                            ]
                        })
                    },
                ))),
            ),
        ],
        CardPhase::Ready,
        CardType::AdvancedLand(ManaType::Green),
        vec![],
        vec![],
    )
}

// TODO: Convoke (Your creatures can help cast this spell. Each creature you tap while casting this spell pays for {1} or one mana of that creature's color.)

pub fn create_chord_of_calling() -> Card {
    Card::new(
        "Chord of Calling",
        "Search your library for a creature card with mana value X or less, put it onto the battlefield, then shuffle.",
        vec![CardActionTrigger::new(
            ActionTriggerType::CardPlayedFromHand(Some((vec![TurnPhase::Main, TurnPhase::Main2], TriggerTarget::Owner))),
            CardRequiredTarget::None,
            Arc::new(AsyncClosureAction::new(Arc::new(
                |game: Arc<Mutex<Game>>,
                 source: Arc<Mutex<Card>>|
                 -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                    Box::pin(async move {
                        let owner = source.lock().await.owner.clone().unwrap();
                        let mut cards = vec![];
                        let mut valid_card_indexes = vec![];

                        let player_id = owner.lock().await.name.clone();
                        let deck = owner.lock().await.deck.draw_pile.clone();
                        let mana_pool = owner.lock().await.mana_pool.green;
                        for (index, card) in deck.iter().enumerate() {
                            cards.push(CardWithDetails::from_card_arc(Arc::clone(card), &game).await);
                            let cost = card.lock().await.cost.len();
                            let is_creature = card.lock().await.card_type == CardType::Creature;
                            if cost < mana_pool as usize && is_creature {
                                valid_card_indexes.push(index as i32)
                            }
                        }

                        // game.lock().await.send_command(LobbyCommand::ChooseFromSelection(CardSelectionDetails { player_id, cards, valid_card_indexes }));

                                game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                        ability_id: None,
                                        card: source,
                                        action: Arc::new(ChooseFromSelectionAction::new(player_id, cards, valid_card_indexes, Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                                                Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                                |game: Arc<Mutex<Game>>,
                                                source_card: Arc<Mutex<Card>>,
                                                target: Arc<Mutex<Card>>|
                                                -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                                    Box::pin(async move {
                                                        println!("source????? {:?}", source_card);
                                                        let position = game.lock().await.frontend_target_from_card(&target).await;
                                                        Game::play_card_without_mana(&game, position).await.expect("oh wow we are here?");
                                                        // let source_card_cloned = source_card.clone();
                                                        // {
                                                        //     game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {action:Arc::new(ReturnToHandAction{}), card: source_card_cloned, target: Some(EffectTarget::Card(target)), ability_id: None })]).await;
                                                        // }

                                                        // game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                                        //         ability_id: None,
                                                        //         card: source_card,
                                                        //         action: Arc::new(CastMandatoryAdditionalAbility {
                                                        //             action_type: ActionType::None,
                                                        //             mana: vec![],
                                                        //             target: CardRequiredTarget::CreatureOfType(
                                                        //                 CreatureType::Elf,
                                                        //                 CardTargetTeam::Owner,
                                                        //                 Some(true)
                                                        //             ),
                                                        //             description: "Untap target creature".to_string(),
                                                        //             ability: Arc::new(|_| -> Arc<dyn CardAction + Send + Sync> {
                                                        //                 Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                                        //                     |game: Arc<Mutex<Game>>,
                                                        //                     source_card: Arc<Mutex<Card>>,
                                                        //                     target: Arc<Mutex<Card>>|
                                                        //                     -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                                        //                         Box::pin(async move {
                                                        //                             { target.lock().await.tapped = false; }

                                                        //                         })
                                                        //                     },
                                                        //                 )))
                                                        //             })
                                                        //         }) ,
                                                        //         target: None
                                                        //     })]).await;
                                                        Ok(())
                                                })
                                            })))
                                            }))),

                                        target: None
                                    })]).await;

                    // get cards in deck
                    // send
                    Ok(())
                    })
                },
            ))),
        )],
        CardPhase::Ready,
        CardType::Instant,
        vec![],
        vec![ManaType::Green, ManaType::Green, ManaType::Green],
    )
}

pub fn create_quirion_ranger() -> Card {
    create_creature_card!(
            "Quirion Ranger",
            CreatureType::Elf,
            "",
            2,
            2,
            [ManaType::Colorless, ManaType::Green],
            [],
            CardActionTrigger::new_with_requirements(
                ActionTriggerType::AbilityWithinPhases(
                    "Return a Forest you control to its owner's hand: Untap target creature. Activate only once each turn.".to_string(),
                    vec![],
                    None,
                    false,
                ),
                CardRequiredTarget::CardOfType(CardType::BasicLand(ManaType::Green), CardTargetTeam::Owner, None),
                    Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                        |game: Arc<Mutex<Game>>,
                        source_card: Arc<Mutex<Card>>,
                        target: Arc<Mutex<Card>>|
                        -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                            Box::pin(async move {
                                let source_card_cloned = source_card.clone();
                                {
                                    game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {action:Arc::new(ReturnToHandAction{}), card: source_card_cloned, target: Some(EffectTarget::Card(target)), ability_id: None })]).await?;
                                }

                                game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                        ability_id: None,
                                        card: source_card,
                                        action: Arc::new(CastMandatoryAdditionalAbility {
                                            action_type: ActionType::None,
                                            mana: vec![],
                                            target: CardRequiredTarget::CreatureOfType(
                                                CreatureType::Elf,
                                                CardTargetTeam::Owner,
                                                Some(true)
                                            ),
                                            description: "Untap target creature".to_string(),
                                            ability: Arc::new(|_| -> Arc<dyn CardAction + Send + Sync> {
                                                Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                                    |game: Arc<Mutex<Game>>,
                                                    source_card: Arc<Mutex<Card>>,
                                                    target: Arc<Mutex<Card>>|
                                                    -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                                        Box::pin(async move {
                                                            { target.lock().await.tapped = false; Ok(()) }

                                                        })
                                                    },
                                                )))
                                            })
                                        }) ,
                                        target: None
                                    })]).await?;
                        Ok(())
                        })
                    }))),

                Arc::new(
                    |game: Arc<Mutex<Game>>,
                     card: Arc<Mutex<Card>>,
                     ability_id|
                     -> Pin<Box<dyn Future<Output = bool> + Send>> {
                        Box::pin(async move {
                            if let Ok(card) = card.try_lock() {
                                let has_tapped_creature = game.lock().await.filter_cards_in_play(Arc::new(
                                        move |card_arc: Arc<Mutex<Card>>| -> Pin<
                                            Box<dyn Future<Output = bool> + Send>,
                                        > {
                                            Box::pin(async move {
                                                if let Ok(card) = card_arc.try_lock() {
                                                    card.card_type == CardType::Creature && card.tapped
                                                } else {
                                                    false
                                                }
                                            })
                                        },
                                    )).await.len() > 0;
                                let has_forest = card.owner.as_ref().unwrap().lock().await
                                    .filter_cards_in_play(Arc::new(
                                        move |card_arc: Arc<Mutex<Card>>| -> Pin<
                                            Box<dyn Future<Output = bool> + Send>,
                                        > {
                                            Box::pin(async move {
                                                if let Ok(card) = card_arc.try_lock() {
                                                    card.card_type == CardType::BasicLand(ManaType::Green)
                                                } else {
                                                    false
                                                }
                                            })
                                        },
                                    )).await.len() > 0;

                                let total_triggers = game.lock().await.triggers_played_this_turn.get(ability_id.as_str()).unwrap_or(&0).clone();

                                return has_forest && has_tapped_creature && total_triggers ==0;
                            }

                            false
                        })
                    }
                )
            )
        )
}

pub fn create_temple_garden() -> Card {
    Card::new(
        "Temple Garden",
        "As Temple Garden enters, you may pay 2 life. If you don't, it enters tapped.",
        vec![
            CardActionTrigger::new(
                ActionTriggerType::CardPlayedFromHand(Some((
                    vec![
                        TurnPhase::Untap,
                        TurnPhase::Upkeep,
                        TurnPhase::Draw,
                        TurnPhase::Main,
                        TurnPhase::BeginningOfCombat,
                        TurnPhase::DeclareAttackers,
                        TurnPhase::DeclareBlockers,
                        TurnPhase::CombatDamage,
                        TurnPhase::EndOfCombat,
                        TurnPhase::Main2,
                        TurnPhase::End,
                        TurnPhase::Cleanup,
                    ],
                    TriggerTarget::Owner,
                ))),
                CardRequiredTarget::None,
                Arc::new(BlankAction {}),
            ),
            CardActionTrigger::new(
                ActionTriggerType::AbilityWithinPhases(
                    "Add {G} or {W} to your mana pool.".to_string(),
                    vec![],
                    None,
                    true,
                ),
                CardRequiredTarget::None,
                Arc::new(GenerateManaAction {
                    mana_to_add: vec![ManaType::Green],
                    target: PlayerActionTarget::Owner,
                }),
            ),
            CardActionTrigger::new(
                ActionTriggerType::CardPlayedFromHand(Some((vec![TurnPhase::Main, TurnPhase::Main2], TriggerTarget::Owner))),
                CardRequiredTarget::None,
                Arc::new(CastOptionalAdditionalAbility {
                    action_type: ActionType::None,
                    mana: vec![],
                    target: CardRequiredTarget::None,
                    description:
                        "As Temple Garden enters, you may pay 2 life. If you don't, it enters tapped."
                            .to_string(),
                    ability: Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                        Arc::new(AsyncClosureAction::new(Arc::new(
                            |game: Arc<Mutex<Game>>,
                            source: Arc<Mutex<Card>>|
                            -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                                Box::pin(async move {
                                    source.lock().await.owner.as_ref().unwrap().lock().await.modify_stat(StatType::Health, -2).await;
                                    Ok(())
                                })
                            },
                        )))
                            as Arc<dyn CardAction + Send + Sync>
                    }),

                    canceled: Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                        Arc::new(TapCardAction {})
                            as Arc<dyn CardAction + Send + Sync>
                    }),
                }),
            ),
        ],
        CardPhase::Ready,
        CardType::AdvancedLand(ManaType::Green),
        vec![],
        vec![],
    )
}

pub fn create_green_deck_v2() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    deck.append(&mut duplicate_card(create_leaf_crowned_visionary(), 1));
    deck.append(&mut duplicate_card(create_priest_of_titania(), 4));
    deck.append(&mut duplicate_card(create_eladamri_korvecdal(), 3));

    deck.append(&mut duplicate_card(create_wirewood(), 2));
    deck.append(&mut duplicate_card(create_devoted_druid(), 4));
    deck.append(&mut duplicate_card(create_elvish_archdruid(), 1));
    deck.append(&mut duplicate_card(create_ezuri(), 2));
    deck.append(&mut duplicate_card(create_elvish_mystic(), 2));
    deck.append(&mut duplicate_card(create_heritage_druid(), 4));
    deck.append(&mut duplicate_card(create_llanowar_elves(), 3));
    deck.append(&mut duplicate_card(create_elvish_warmaster(), 4));
    deck.append(&mut duplicate_card(create_quirion_ranger(), 3));

    deck.append(&mut duplicate_card(create_chord_of_calling(), 4));

    // deck.append(&mut duplicate_card(create_cavern_of_souls(), 3));
    deck.append(&mut duplicate_card(create_temple_garden(), 3));
    deck.append(&mut duplicate_card(create_pendelhaven(), 2));
    deck.append(&mut duplicate_card(create_forest(), 18));

    // deck.append(&mut duplicate_card(create_test_forest(), 1));
    // deck.append(&mut duplicate_card(create_pendelhaven(), 1));
    // deck.append(&mut duplicate_card(create_llanowar_elves(), 1));

    deck
}

pub fn create_green_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    deck.append(&mut duplicate_card(create_forest(), 12));
    deck.append(&mut duplicate_card(create_pendelhaven(), 2));
    deck.append(&mut duplicate_card(create_priest_of_titania(), 4));
    deck.append(&mut duplicate_card(create_llanowar_elves(), 4));
    deck.append(&mut duplicate_card(create_heritage_druid(), 4));
    deck.append(&mut duplicate_card(create_elvish_mystic(), 4));
    deck.append(&mut duplicate_card(create_quirion_ranger(), 4));
    deck.append(&mut duplicate_card(create_elvish_warmaster(), 4));
    deck.append(&mut duplicate_card(create_wirewood(), 4));
    deck.append(&mut duplicate_card(create_leaf_crowned_visionary(), 4));
    deck.append(&mut duplicate_card(create_devoted_druid(), 4));
    deck.append(&mut duplicate_card(create_ezuri(), 2));
    deck.append(&mut duplicate_card(create_chord_of_calling(), 4));
    deck.append(&mut duplicate_card(create_elvish_archdruid(), 4));

    deck
}

mod test {
    use std::sync::Arc;

    use tokio::sync::{Mutex, RwLock};

    use crate::{
        game::{
            action::ActionTriggerType,
            card::{Card, CardPhase},
            decks::{
                green::{
                    create_devoted_druid, create_forest, create_leaf_crowned_visionary,
                    create_wirewood,
                },
                Deck,
            },
            effects::EffectTarget,
            mana,
            player::Player,
            turn::TurnPhase,
            CardWithDetails, FrontendCardTarget, FrontendPileName, Game,
        },
        lobby::manager::LobbyManager,
    };

    #[tokio::test]
    async fn test_green_1() {
        let mut game = Game::new();
        let player = game
            .add_player(Player::new(
                "test",
                0,
                vec![
                    create_forest(),
                    create_leaf_crowned_visionary(),
                    create_forest(),
                    create_forest(),
                    create_forest(),
                ],
            ))
            .await;

        player.lock().await.draw_card();
        player.lock().await.draw_card();
        player.lock().await.draw_card();
        let leaf = player.lock().await.draw_card();
        game.start_turn(0).await;

        {
            let clone = Arc::clone(&player);
            let mut player = clone.lock().await;
            let mut cards: Vec<Arc<Mutex<Card>>> = player.cards_in_hand.drain(0..3).collect();

            // Append the drained cards to `cards_in_play`
            player.cards_in_play.append(&mut cards);
        }

        let ga = Arc::new(Mutex::new(game));
        // Game::process_action_queue(ga.clone(), leaf.clone().unwrap()).await;
        ga.lock().await.print().await;

        ga.lock()
            .await
            .activate_card_action_old(&player, 0, None)
            .await
            .expect("oh no?");

        ga.lock()
            .await
            .activate_card_action_old(&player, 1, None)
            .await
            .expect("oh no?");

        Game::play_card(&ga, &player, 0, None).await.expect("oh no");
        // Game::process_action_queue(ga.clone(), leaf.clone().unwrap()).await;

        let ability_id = ga
            .lock()
            .await
            .async_abilities
            .keys()
            .find(|x| true)
            .unwrap()
            .clone();
        println!("ability {}", ability_id);

        Game::respond_player_ability(ga.clone(), &player, ability_id, true, None)
            .await
            .expect("ok??");

        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_nanos(500)).await;
        })
        .await
        .expect("hm");

        ga.lock()
            .await
            .activate_card_action_old(&player, 2, None)
            .await
            .expect("oh no?");

        ga.lock().await.print().await;

        // ga.lock().await.advance_turn().await;
    }

    #[tokio::test]
    async fn test_green_2() {
        let mut game = Game::new();
        let player = game
            .add_player(Player::new(
                "test",
                0,
                vec![
                    create_wirewood(),
                    create_wirewood(),
                    create_wirewood(),
                    create_forest(),
                ],
            ))
            .await;

        player.lock().await.draw_card();
        let a = player.lock().await.draw_card();
        let b = player.lock().await.draw_card();
        let c = player.lock().await.draw_card();
        game.start_turn(0).await;
        game.advance_turn().await;
        // for _ in 0..18 {
        // }

        {
            let clone = Arc::clone(&player);
            let mut player = clone.lock().await;
            let mut cards: Vec<Arc<Mutex<Card>>> = player.cards_in_hand.drain(0..4).collect();

            // Append the drained cards to `cards_in_play`
            player.cards_in_play.append(&mut cards);
        }

        let ga = Arc::new(Mutex::new(game));

        {
            a.clone().unwrap().lock().await.current_phase = CardPhase::Ready;
            b.clone().unwrap().lock().await.current_phase = CardPhase::Ready;
            c.clone().unwrap().lock().await.current_phase = CardPhase::Ready;
        }

        ga.lock()
            .await
            .activate_card_action_old(&player, 1, None)
            .await
            .expect("oh no?");
        ga.lock().await.print().await;
    }
    #[tokio::test]
    async fn test_green_3() {
        let mut game = Game::new();
        let player = game
            .add_player(Player::new(
                "test",
                0,
                vec![
                    create_devoted_druid(),
                    create_devoted_druid(),
                    create_forest(),
                    create_forest(),
                ],
            ))
            .await;

        player.lock().await.draw_card();
        player.lock().await.draw_card();
        let a = player.lock().await.draw_card();
        let b = player.lock().await.draw_card();
        game.start_turn(0).await;
        game.advance_turn().await;
        // for _ in 0..18 {
        // }

        {
            let clone = Arc::clone(&player);
            let mut player = clone.lock().await;
            let mut cards: Vec<Arc<Mutex<Card>>> = player.cards_in_hand.drain(0..4).collect();

            // Append the drained cards to `cards_in_play`
            player.cards_in_play.append(&mut cards);
        }

        let ga = Arc::new(Mutex::new(game));
        ga.lock().await.print().await;

        {
            a.clone().unwrap().lock().await.current_phase = CardPhase::Ready;
            b.clone().unwrap().lock().await.current_phase = CardPhase::Ready;
        }

        let turn = ga.clone().lock().await.current_turn.clone().unwrap();

        let details = CardWithDetails::from_card_arc(a.unwrap(), &ga).await;
        println!("{:?}", details.abilities);

        Game::activate_card_action(
            &ga,
            &player,
            FrontendCardTarget {
                player_index: 0,
                pile: FrontendPileName::Play,
                card_index: 0,
            },
            None,
            details.abilities[0].id.clone(),
        )
        .await
        .expect("oh no?");

        Game::activate_card_action(
            &ga,
            &player,
            FrontendCardTarget {
                player_index: 0,
                pile: FrontendPileName::Play,
                card_index: 0,
            },
            None,
            details.abilities[1].id.clone(),
        )
        .await
        .expect("oh no?");

        Game::activate_card_action(
            &ga,
            &player,
            FrontendCardTarget {
                player_index: 0,
                pile: FrontendPileName::Play,
                card_index: 0,
            },
            None,
            details.abilities[0].id.clone(),
        )
        .await
        .expect("oh no?");

        Game::activate_card_action(
            &ga,
            &player,
            FrontendCardTarget {
                player_index: 0,
                pile: FrontendPileName::Play,
                card_index: 0,
            },
            None,
            details.abilities[1].id.clone(),
        )
        .await
        .expect("oh no?");
        ga.lock().await.print().await;
    }
}
