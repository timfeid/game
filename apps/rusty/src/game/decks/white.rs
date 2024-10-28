use crate::game::{
    action::{
        generate_mana::GenerateManaAction, Action, ActionBuilder, ActionTriggerType,
        ApplyDynamicEffectToCard, AsyncClosureAction, CardAction, CardActionTarget,
        CardActionTrigger, CardActionWrapper, CardRequiredTarget, CardTargetTeam,
        CastMandatoryAdditionalAbility, CounterSpellAction, DeclareAttackerAction,
        DeclareBlockerAction, PhaseTarget, PlayCardAction, PlayerActionTarget,
    },
    card::{
        card::{create_creature_card, create_multiple_cards},
        Card, CardBuilder, CardPhase, CardType, Counter, CreatureType,
    },
    effects::{
        DynamicStatModifierEffect, Effect, EffectID, EffectTarget, ExpireContract,
        ModifyStatTarget, StatModifierEffect,
    },
    mana::ManaType,
    player::Player,
    stat::{Stat, StatType, Stats},
    turn::TurnPhase,
    ActionType, FrontendTarget, Game,
};
use std::{f32::consts::E, future::Future, mem::zeroed, pin::Pin, sync::Arc};

use tokio::sync::Mutex;
use ulid::Ulid;

use super::duplicate_card;

fn create_test_plains() -> Card {
    CardBuilder::new()
        .name("Plains")
        .description("")
        .card_type(CardType::BasicLand(ManaType::Purple))
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Adds {W} white mana to your pool.".to_string(),
                vec![],
                None,
                true,
                CardRequiredTarget::None,
            ))
            .action(GenerateManaAction {
                mana_to_add: vec![
                    ManaType::Purple,
                    ManaType::Purple,
                    ManaType::Purple,
                    ManaType::Purple,
                    ManaType::Purple,
                    ManaType::Purple,
                    ManaType::Purple,
                    ManaType::Purple,
                    ManaType::Purple,
                    ManaType::Purple,
                    ManaType::Purple,
                ],
                target: PlayerActionTarget::Owner,
            }),
        )
        .build()
}

pub fn create_angels_blue_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    // deck.push();
    // deck.append(&mut duplicate_card(create_lunarch_veteran(), 1));
    // deck.append(&mut duplicate_card(create_bishop_of_wings(), 4));
    // deck.append(&mut duplicate_card(create_giada_font_of_hope(), 4));
    // deck.append(&mut duplicate_card(create_skyclave_cleric(), 1));

    // deck.append(&mut duplicate_card(create_counterspell(), 4));
    // deck.append(&mut duplicate_card(create_frost_breath(), 4));
    // deck.append(&mut duplicate_card(create_unsummon(), 4));
    // deck.append(&mut duplicate_card(create_youthful_valkyrie(), 4));
    // deck.append(&mut duplicate_card(create_metropolis_reformer(), 2));
    // deck.append(&mut duplicate_card(create_resplendent_angel(), 4));
    // deck.append(&mut duplicate_card(create_righteous_valkyrie(), 4));
    deck.append(&mut duplicate_card(create_ossification(), 2));
    // deck.append(&mut duplicate_card(create_ajani_strength_of_the_pride(), 1));
    // deck.append(&mut duplicate_card(create_serra_ascendant(), 4));
    // deck.append(&mut duplicate_card(create_angel_of_vitality(), 4));
    // deck.append(&mut duplicate_card(create_island(), 10));
    // deck.append(&mut duplicate_card(create_plains(), 12));

    deck.append(&mut duplicate_card(create_test_plains(), 1));
    // deck.append(&mut duplicate_card(create_angel_of_vitality(), 4));
    // deck.append(&mut duplicate_card(create_skyclave_cleric(), 4));
    // deck.append(&mut duplicate_card(create_lunarch_veteran(), 4));
    // deck.append(&mut duplicate_card(create_serra_ascendant(), 4));

    deck
}

pub fn create_angels_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    // deck.push();
    // deck.append(&mut duplicate_card(create_lunarch_veteran(), 4));
    // deck.append(&mut duplicate_card(create_bishop_of_wings(), 4));
    // deck.append(&mut duplicate_card(create_giada_font_of_hope(), 4));
    // deck.append(&mut duplicate_card(create_skyclave_cleric(), 1));
    // deck.append(&mut duplicate_card(create_youthful_valkyrie(), 4));
    // deck.append(&mut duplicate_card(create_metropolis_reformer(), 2));
    // deck.append(&mut duplicate_card(create_resplendent_angel(), 4));
    // // deck.append(&mut duplicate_card(create_righteous_valkyrie(), 4));
    // deck.append(&mut duplicate_card(create_ossification(), 2));
    // deck.append(&mut duplicate_card(create_ajani_strength_of_the_pride(), 1));
    // // deck.append(&mut duplicate_card(create_serra_ascendant(), 4));
    // deck.append(&mut duplicate_card(create_angel_of_vitality(), 4));
    // deck.append(&mut duplicate_card(create_plains(), 22));

    deck.append(&mut duplicate_card(create_test_plains(), 1));
    // deck.append(&mut duplicate_card(create_test_card(), 4));
    // deck.append(&mut duplicate_card(create_ossification(), 7));
    // deck.append(&mut duplicate_card(create_skyclave_apparition(), 4));
    deck.append(&mut duplicate_card(create_skyclave_cleric(), 2));
    deck.append(&mut duplicate_card(create_ossification(), 4));
    deck.append(&mut duplicate_card(create_counterspell(), 4));
    deck.append(&mut duplicate_card(create_lunarch_veteran(), 2));
    // deck.append(&mut duplicate_card(create_skyclave_cleric(), 4));
    // deck.append(&mut duplicate_card(create_lunarch_veteran(), 4));
    // deck.append(&mut duplicate_card(create_serra_ascendant(), 4));

    deck
}

pub fn create_counterspell() -> Card {
    // Card::new(
    //     "Counter Spell",
    //     "Counter target spell.",
    //     vec![CardActionTrigger::new(
    //         ActionTriggerType::CardPlayedFromHand(None),
    //         CardRequiredTarget::Spell,
    //         Arc::new(CounterSpellAction {}),
    //     )],
    //     CardPhase::Ready,
    //     CardType::Instant,
    //     vec![],
    //     vec![ManaType::Blue, ManaType::Blue],
    // )
    CardBuilder::new()
        .name("Counter Spell")
        .description("Counter target spell")
        .play_target(CardRequiredTarget::Spell)
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardEnteredBattlefield)
                .action(CounterSpellAction {}),
        )
        .card_type(CardType::Instant)
        .build()
}

pub fn create_ossification() -> Card {
    CardBuilder::new()
        .name("Ossification")
        .description("Enchant basic land you control\nWhen Ossification enters, exile target creature or planeswalker an opponent controls until Ossification leaves the battlefield.")
        .play_target(CardRequiredTarget::BasicLand(CardTargetTeam::Owner, None))
        .add_action(
            ActionBuilder::new(
                ActionTriggerType::CardEnteredBattlefield,
            )
            .closure_action(|game, source_card, owner, target, _| {
                Box::pin(async move {

                    let card = Game::card_from_frontend_target(&game, &target.unwrap()).await;
                    source_card.lock().await.attached = Some(card);

                    Game::execute_actions(game.clone(), vec![Arc::new(CardActionWrapper {
                        ability_id: None,
                        card: source_card.clone(),
                        action: Arc::new(CastMandatoryAdditionalAbility {
                            action_type: ActionType::None,
                            mana: vec![],
                            target: CardRequiredTarget::CardOfType(
                                CardType::Creature,
                                CardTargetTeam::Opponent,
                                None
                            ),
                            description: "Exile target creature or planeswalker an opponent controls.".to_string(),
                            ability: Arc::new(|_| -> Arc<dyn CardAction + Send + Sync> {
                                Arc::new(AsyncClosureAction::new(|game, source_card, owner, target, _| {
                                    Box::pin(async move {
                                        source_card.lock().await.target = target.clone();
                                        if let Some(target) = &target {
                                            let card = Game::card_from_frontend_target(&game, &target).await;
                                            Game::exile_card(&game, &card).await?;
                                        }
                                        Ok(())
                                    })
                                }))
                            }),
                        }),
                        target: None,
                    })]).await?;
                    println!("hello!");

                    Ok(())
                })
            })
        )
        .mana_cost(vec![ManaType::Colorless, ManaType::Purple])
        .card_type(CardType::Enchantment)
        .phase(CardPhase::Ready)
            .play_requirements(|game, source, _| {
                Box::pin(async move {
                    if let Some(owner) = { source.lock().await.owner.clone() } {
                        // Check if an opponent has a creature or planeswalker in play
                        let owner_cloned = owner.clone();
                        let opponent_has_creature_or_pw = game.lock().await
                            .filter_cards_in_play(move |card_in_play| {
                                if let Some(card_owner) = &card_in_play.owner {
                                    !Arc::ptr_eq(&owner_cloned, card_owner) && card_in_play.card_type == CardType::Creature
                                } else {
                                    false
                                }
                            }).await.len() > 0;

                        // Check if the owner has a basic land in play
                        let has_basic_land = owner.lock().await.filter_cards_in_play(|card| {
                            matches!(card.card_type, CardType::BasicLand(_))
                        }).await.len() > 0;

                        // Both conditions must be true
                        has_basic_land && opponent_has_creature_or_pw
                    } else {
                        false
                    }
                })
            })
        .build()
}

// pub fn create_skyclave_apparition() -> Card {
//     CardBuilder::new()
//         .name("Skyclave Apparition")
//         .description("When Skyclave Apparition enters, exile up to one target nonland, nontoken permanent you don't control with mana value 4 or less.")
//         .creature_of_type(1,3, CreatureType::Angel)
//         .mana_cost(vec![ManaType::Colorless, ManaType::Purple])
//         .add_action(
//             ActionBuilder::new(
//                 ActionTriggerType::CardEnteredBattlefield,
//             )
//             .closure_action(
//                 |game, source, owner, target, ability_id| {
//                     Box::pin(async move {
//                         Ok(())
//                     })
//                 }
//             ))
//         .build()
// }

pub fn create_skyclave_cleric() -> Card {
    CardBuilder::new()
        .name("Skyclave Cleric")
        .description("When Skyclave Cleric enters, you gain 2 life.")
        .creature_of_type(1, 3, CreatureType::Angel)
        .mana_cost(vec![ManaType::Colorless, ManaType::Purple])
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardEnteredBattlefield).closure_action(
                |game, source, owner, _, _| {
                    Box::pin(async move {
                        Game::add_stat(&game, &source, &owner, StatType::Health, 2).await;

                        Ok(())
                    })
                },
            ),
        )
        .build()
}

// pub fn create_skyclave_cleric() -> Card {
//     create_creature_card!(
//         "Skyclave Cleric",
//         CreatureType::Angel,
//         "When Skyclave Cleric enters, you gain 2 life.",
//         1,
//         3,
//         [ManaType::Colorless, ManaType::Purple],
//         [],
//         CardActionTrigger::new(
//             ActionTriggerType::CardPlayedFromHand(Some((
//                 vec![TurnPhase::Main, TurnPhase::Main2],
//                 PhaseTarget::Owner
//             ))),
//             CardRequiredTarget::None,
//             Arc::new(AsyncClosureAction::new(Arc::new(
//                 |game, source, target, ability_id| {
//                     Box::pin(async move {
//                         let owner = source.lock().await.owner.clone();
//                         if let Some(owner) = owner {
//                             println!("adding health...");
//                             todo!()
//                             // game.lock().await.add_health(&owner, 2).await;
//                         }
//                         Ok(())
//                     })
//                 }
//             )))
//         )
//     )
// }

// // pub fn create_serra_ascendant() -> Card {
// //     create_creature_card!(
// //         "Serra Ascendant",
// //         CreatureType::None,
// //         "As long as you have 30 or more life, Serra Ascendant gets +5/+5 and has flying.",
// //         1,
// //         1,
// //         [ManaType::Purple],
// //         [],
// //         CardActionTrigger::new(
// //             ActionTriggerType::Continuous,
// //             CardRequiredTarget::None,
// //             Arc::new(ApplyDynamicEffectToCard::new(
// //                 Arc::new(
// //                     move |card_arc: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = i16> + Send>> {
// //                         Box::pin(async move {
// //                             let owner = { card_arc.lock().await.owner.clone() };

// //                             if let Some(owner_arc) = owner {
// //                                 let owner = owner_arc.lock().await;
// //                                 if owner.get_stat_value(StatType::Health) >= 30 {
// //                                     5
// //                                 } else {
// //                                     0
// //                                 }
// //                             } else {
// //                                 0
// //                             }
// //                         })
// //                     },
// //                 ),
// //                 Arc::new(
// //                     move |target,
// //                           card,
// //                           amount,
// //                           id|
// //                           -> Pin<
// //                         Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>,
// //                     > {
// //                         Box::pin(async move {
// //                             let mut effects: Vec<Arc<Mutex<dyn Effect + Send + Sync>>> = vec![];

// //                             let (name, id) = {
// //                                 let card = card.lock().await;
// //                                 (card.name.clone(), card.id.clone())
// //                             };

// //                             let mut effect = DynamicStatModifierEffect::new(
// //                                 EffectTarget::Card(card.clone()),
// //                                 StatType::Power,
// //                                 amount.clone(),
// //                                 ExpireContract::Never,
// //                                 Some(card.clone()),
// //                                 false,
// //                             );
// //                             let id = format!("{}-{}-{}-damage", card.lock().await.id, id, name);
// //                             effect.id = EffectID(id.clone());

// //                             let total = (amount)(card.clone()).await;
// //                             if total > 0 {
// //                                 effects.push(Arc::new(Mutex::new(effect)));
// //                                 let mut effect = StatModifierEffect::new(
// //                                     EffectTarget::Card(card.clone()),
// //                                     StatType::Flying,
// //                                     1,
// //                                     ExpireContract::Never,
// //                                     Some(card.clone()),
// //                                 );

// //                                 let id = format!("{}-{}-{}-flying", card.lock().await.id, id, name);
// //                                 effect.id = EffectID(id.clone());
// //                                 effects.push(Arc::new(Mutex::new(effect)));
// //                             }

// //                             let mut effect = DynamicStatModifierEffect::new(
// //                                 EffectTarget::Card(card.clone()),
// //                                 StatType::Toughness,
// //                                 amount.clone(),
// //                                 ExpireContract::Never,
// //                                 Some(card.clone()),
// //                                 false,
// //                             );

// //                             let id = format!("{}-{}-{}-defense", card.lock().await.id, id, name);
// //                             effect.id = EffectID(id.clone());
// //                             effects.push(Arc::new(Mutex::new(effect)));

// //                             println!("Applying effects! {:?}", effects);
// //                             effects
// //                         })
// //                     },
// //                 )
// //             ),)
// //         )
// //     )
// // }

// pub fn create_youthful_valkyrie() -> Card {
//     create_creature_card!(
//         "Youthful Valkyrie",
//         CreatureType::Angel,
//         "Whenever another Angel you control enters, put a +1/+1 counter on Youthful Valkyrie.",
//         1,
//         3,
//         [ManaType::Colorless, ManaType::Purple],
//         [StatType::Flying],
//         CardActionTrigger::new(
//             ActionTriggerType::OtherCardPlayed(PhaseTarget::Owner),
//             CardRequiredTarget::None,
//             Arc::new(AsyncClosureAction::new(Arc::new(
//                 |game, source, target, ability_id| {
//                     Box::pin(async move {
//                         let target = Game::card_from_frontend_target(&game, &target.expect("hm"));
//                         let creature_type = target.lock().await.creature_type.clone();
//                         if creature_type == Some(CreatureType::Angel) {
//                             Card::add_counter(
//                                 source.clone(),
//                                 &game,
//                                 Counter::PowerToughnessModifier(1, 1),
//                             )
//                             .await;
//                         }
//                         println!("done");
//                         Ok(())
//                     })
//                 }
//             )))
//         )
//     )
// }

// pub fn create_metropolis_reformer() -> Card {
//     create_creature_card!(
//         "Metropolis Reformer",
//         CreatureType::Angel,
//         "You have hexproof.\nWhenever Metropolis Reformer is dealt damage, you gain that much life.",
//         2,
//         3,
//         [ManaType::Colorless, ManaType::Colorless, ManaType::Purple],
//         [StatType::Flying, StatType::Vigilance],
//         CardActionTrigger::new(
//             ActionTriggerType::DamageApplied,
//             CardRequiredTarget::None,
//             Arc::new(AsyncClosureAction::new(Arc::new(
//                 |game, source, target, ability_id| {
//                     Box::pin(async move {
//                         let damage_taken = {source.lock().await.damage_taken.clone()};

//                         if damage_taken > 0 {

//                             let owner = { source.lock().await.owner.clone().unwrap() };
//                             // game.lock().await.add_health(&owner, damage_taken).await;
//                             todo!()
//                         }

//                         Ok(())
//                     })
//                 }
//             )))
//         )
//     )
// }

// pub fn create_giada_font_of_hope() -> Card {
//     create_creature_card!(
//         "Giada, Font of Hope",
//         CreatureType::Angel,
//         "Each other Angel you control enters with an additional +1/+1 counter on it for each Angel you already control.",
//         2,
//         2,
//         [ManaType::Colorless, ManaType::Purple],
//         [StatType::Flying, StatType::Vigilance],
//         CardActionTrigger::new(
//             ActionTriggerType::AbilityWithinPhases(
//                 // TODO: mana restriction.
//                 "Add {W}. Spend this mana only to cast an Angel spell.".to_string(),
//                 vec![],
//                 None,
//                 true
//             ),
//             CardRequiredTarget::None,
//             Arc::new(GenerateManaAction {
//                 mana_to_add: vec![ManaType::Purple],
//                 target: PlayerActionTarget::Owner
//             })
//         ),
//         CardActionTrigger::new(
//             ActionTriggerType::OtherCardPlayed(
//                 PhaseTarget::Owner
//             ),
//             CardRequiredTarget::None,
//             Arc::new(AsyncClosureAction::new(Arc::new(
//                 |game, source, target, ability_id| {
//                     Box::pin(async move {
//                         let target = Game::card_from_frontend_target(&game, &target.expect("hm"));
//                         let owner = {
//                             if let Ok(card_l) = target.try_lock() {
//                                 card_l.owner.clone()
//                             } else {
//                                 None
//                             }
//                         };
//                         let creature_type = target.lock().await.creature_type.clone();

//                         if let Some(owner) = owner {
//                             if creature_type == Some(CreatureType::Angel) {

//                                 let angel_cards = {
//                                     owner
//                                         .lock()
//                                         .await
//                                         .filter_cards_in_play(Arc::new(
//                                             move |card_arc: Arc<Mutex<Card>>| -> Pin<
//                                                 Box<dyn Future<Output = bool> + Send>,
//                                             > {
//                                                 Box::pin(async move {
//                                                     if let Ok(card) = card_arc.try_lock() {
//                                                         return card.creature_type == Some(CreatureType::Angel);
//                                                     }

//                                                     false
//                                                 })
//                                             },
//                                         ))
//                                         .await
//                                 };

//                                 for _ in 0..angel_cards.len()-1 {
//                                     Card::add_counter(
//                                         target.clone(),
//                                         &game,
//                                         Counter::PowerToughnessModifier(1, 1),
//                                     )
//                                     .await;
//                                 }
//                             }
//                         }
//                         Ok(())
//                     })
//                 }
//             )))
//         )
//     )
// }

fn create_lunarch_veteran() -> Card {
    CardBuilder::new()
        .name("Lunarch Veteran")
        .creature_of_type(1, 1, CreatureType::Angel)
        .description("Whenever another creature you control enters, you gain 1 life.")
        .mana_cost(vec![ManaType::Purple])
        .add_action(
            ActionBuilder::new(ActionTriggerType::OtherCardPlayed(PhaseTarget::Owner))
                .closure_action(|game, source, owner, target, ability_id| {
                    Box::pin(async move {
                        let card_type = source.lock().await.card_type.clone();

                        if card_type == CardType::Creature {
                            Game::add_stat(&game, &source, &owner, StatType::Health, 1).await;
                        }
                        Ok(())
                    })
                }),
        )
        .build()
}

// fn create_resplendent_angel() -> Card {
//     create_creature_card!(
//         "Resplendent Angel",
//         CreatureType::Angel,
//         "At the beginning of each end step, if you gained 5 or more life this turn, create a 4/4 white Angel creature token with flying and vigilance.",
//         3,
//         3,
//         // [ManaType::Purple, ManaType::Colorless, ManaType::Colorless],
//         [ManaType::Colorless, ManaType::Purple, ManaType:: Purple],
//         [StatType::Flying],
//         CardActionTrigger::new(
//             ActionTriggerType::AbilityWithinPhases(
//                 "Until end of turn, Resplendent Angel gets +2/+2 and gains lifelink.".to_string(),
//                 vec![ManaType::Colorless, ManaType::Colorless, ManaType::Colorless, ManaType::Purple,ManaType::Purple,  ManaType::Purple, ],
//                 None,
//                 false
//             ),
//             CardRequiredTarget::None,
//                 Arc::new(ApplyDynamicEffectToCard::new(Arc::new(
//                     |game, target, source_card, effect_id| {
//                         let target = Game::frontend_card_to_effect_target(&game, &target);

//                         Box::pin(async move {
//                             vec![
//                                 Arc::new(Mutex::new(StatModifierEffect::new(
//                                     target.clone(),
//                                     StatType::Power,
//                                     2,
//                                     ExpireContract::Turns(1),
//                                     Some(source_card.clone()),
//                                 )))
//                                     as Arc<Mutex<dyn Effect + Send + Sync>>,
//                                 Arc::new(Mutex::new(StatModifierEffect::new(
//                                     target.clone(),
//                                     StatType::Lifelink,
//                                     1,
//                                     ExpireContract::Turns(1),
//                                     Some(source_card.clone()),
//                                 ))),
//                                 Arc::new(Mutex::new(StatModifierEffect::new(
//                                     target,
//                                     StatType::Toughness,
//                                     2,
//                                     ExpireContract::Turns(1),
//                                     Some(source_card.clone()),
//                                 ))),
//                             ]
//                         })
//                     },
//                 ))),
//         ),
//         CardActionTrigger::new(
//             ActionTriggerType::PhaseStarted(vec![TurnPhase::End], PhaseTarget::Any),
//             CardRequiredTarget::None,
//             Arc::new(AsyncClosureAction::new(Arc::new(
//                 |game, card, target, ability| {
//                     Box::pin(async move {
//                         let (difference, owner) = {

//                         let card = card.lock().await;
//                         let owner_arc = card.owner.clone().unwrap();
//                         let owner = owner_arc.lock().await;
//                         let health = owner.stat_manager.get_stat_value(StatType::Health);
//                             (health - owner.health_at_start_of_round, owner_arc.clone())
//                         };
//                         if difference > 3 {
//                             Game::play_token(&game, &owner, create_creature_card!("Token", CreatureType::Angel, "", 4,4, [], [])).await.ok();
//                         }
//                         Ok(())
//                     })
//                 }
//             )))
//         )
//     )
// }

// fn create_angelic_accord() -> Card {
//     Card::new(
//         "Angelic Accord",
//         "At the beginning of each end step, if you gained 4 or more life this turn, create a 4/4 white Angel creature token with flying.",
//         vec![
//             CardActionTrigger::new(
//                 ActionTriggerType::PhaseStarted(vec![TurnPhase::End], PhaseTarget::Any),
//                 CardRequiredTarget::None,
//                 Arc::new(AsyncClosureAction::new(Arc::new(
//                     |game, card, target, ability| {
//                         Box::pin(async move {
//                             let (difference, owner) = {

//                             let card = card.lock().await;
//                             let owner_arc = card.owner.clone().unwrap();
//                             let owner = owner_arc.lock().await;
//                             let health = owner.stat_manager.get_stat_value(StatType::Health);
//                                 (health - owner.health_at_start_of_round, owner_arc.clone())
//                             };
//                             println!("difference: {}", difference);
//                             if difference > 3 {
//                                 Game::play_token(&game, &owner, create_creature_card!("Token", CreatureType::Angel, "", 4,4, [], [])).await.ok();
//                             }
//                             Ok(())
//                         })
//                     }
//                 )))
//             )
//         ],
//         CardPhase::Ready,
//         CardType::Enchantment,
//         vec![],
//         // vec![ManaType::Purple, ManaType::Colorless, ManaType::Colorless, ManaType::Colorless],
//         vec![],
//     )
// }

// fn create_bishop_of_wings() -> Card {
//     create_creature_card!(
//         "Bishop of Wings",
//         CreatureType::Angel,
//         "Whenever an Angel you control enters, you gain 4 life.\nWhenever an Angel you control dies, create a 1/1 white Spirit creature token with flying.",
//         1,
//         4,
//         [ManaType::Purple, ManaType::Purple],
//         // [],
//         [],
//         // CardActionTrigger::new(
//         //     ActionTriggerType::AbilityWithinPhases("Sacrifice bishop".to_string(), vec![], None, false),
//         //     CardRequiredTarget::None,
//         //     Arc::new(AsyncClosureAction::new(Arc::new(
//         //        |game, source, target, ability| {
//         //             Box::pin(async move {
//         //                 game.lock().await.destroy_card(&source).await;
//         //             })
//         //         }
//         //     )))
//         // ),
//         CardActionTrigger::new(
//             ActionTriggerType::OtherCardDestroyed(PhaseTarget::Owner),
//             CardRequiredTarget::None,
//             Arc::new(AsyncClosureAction::new(Arc::new(
//                 |game, source, target, ability_id| {
//                     Box::pin(async move {
//                         let card_destroyed = Game::card_from_frontend_target(&game, &target.expect("No target?"));
//                         let creature_type = {card_destroyed.lock().await.creature_type.clone()};
//                         let owner = {card_destroyed.lock().await.owner.clone().unwrap()};

//                         if creature_type == Some(CreatureType::Angel) {
//                             Game::play_token(&game, &owner, create_creature_card!("Token", CreatureType::Angel, "", 1,1, [], [StatType::Flying])).await?;
//                         }
//                         Ok(())
//                     })
//                 }
//             )))
//         )
//         ,
//         CardActionTrigger::new(
//             ActionTriggerType::OtherCardPlayed(PhaseTarget::Owner),
//             CardRequiredTarget::None,
//             Arc::new(AsyncClosureAction::new(Arc::new(
//                 |game, source, target, ability_id| {
//                     Box::pin(async move {
//                         let card_played = Game::card_from_frontend_target(&game, &target.expect("No target?"));
//                         let (owner, creature_type) = {
//                             let card = card_played.lock().await;
//                             let owner = card.owner.clone().unwrap();
//                             let creature_type = card.creature_type.clone();
//                             (owner, creature_type)
//                         };

//                         if creature_type == Some(CreatureType::Angel) {
//                             // game.lock().await.add_health(&owner, 4).await;
//                             todo!()
//                         }
//                         Ok(())

//                     })
//                 }
//             )))
//         )
//     )
// }

// fn create_ajani_strength_of_the_pride() -> Card {
//     Card::new(
//         "Ajani, Strength of the Pride",
//         "[0]: If you have at least 15 life more than your starting life total, exile Ajani, Strength of the Pride and each artifact and creature your opponents control.",
//         vec![
//             CardActionTrigger::new(
//                 ActionTriggerType::CardPlayedFromHand(Some((
//                     vec![TurnPhase::Main, TurnPhase::Main2],
//                     PhaseTarget::Owner,
//                 ))),
//                 CardRequiredTarget::None,
//                 Arc::new(PlayCardAction {}),
//             ),
//             CardActionTrigger::new_with_requirements(
//                 ActionTriggerType::AbilityWithinPhases(
//                     "[+1]: You gain life equal to the number of creatures you control plus the number of planeswalkers you control.".to_string(),
//                     vec![],
//                     Some((vec![TurnPhase::Main, TurnPhase::Main2], PhaseTarget::Owner)),
//                     false,
//                 ),
//                 CardRequiredTarget::None,
//                 Arc::new(AsyncClosureAction::new(Arc::new(
//                     |game, source, target, ability| {
//                         Box::pin(async move {
//                             Card::add_counter(source.clone(), &game, Counter::Incremental(1)).await;
//                             let owner = source.lock().await.owner.clone();
//                             if let Some(owner) = owner {
//                                 let total_creatures_and_plainwalkers = owner.lock().await.filter_cards_in_play(Arc::new(
//                                     move |card_arc: Arc<Mutex<Card>>| -> Pin<
//                                         Box<dyn Future<Output = bool> + Send>,
//                                     > {
//                                         Box::pin(async move {
//                                             if let Ok(card) = card_arc.try_lock() {
//                                                 vec![CardType::Creature, CardType::Planeswalker].contains(&card.card_type)
//                                             } else {
//                                                 false
//                                             }
//                                         })
//                                     },
//                                 )).await.len();

//                                 // game.lock().await.add_health(&owner, total_creatures_and_plainwalkers as i16).await;
//                                 todo!();

//                             }
//                             Ok(())
//                         })
//                     }
//                 ))),

//                 Arc::new(
//                     |game: Arc<Mutex<Game>>,
//                      card: Arc<Mutex<Card>>,
//                      ability_id|
//                      -> Pin<Box<dyn Future<Output = bool> + Send>> {
//                         Box::pin(async move {
//                             let game = game.lock().await;
//                             let total_triggers = game.cards_triggered_this_turn.get(&card.lock().await.id.clone());

//                             return total_triggers.is_none();
//                         })
//                     }
//                 )
//             ),
//             CardActionTrigger::new_with_requirements(
//                 ActionTriggerType::AbilityWithinPhases(
//                     "[−2]: Create a 2/2 white Cat Soldier creature token named Ajani's Pridemate with \"Whenever you gain life, put a +1/+1 counter on Ajani's Pridemate.\"".to_string(),
//                     vec![],
//                     Some((vec![TurnPhase::Main, TurnPhase::Main2], PhaseTarget::Owner)),
//                     false,
//                 ),
//                 CardRequiredTarget::None,
//                 Arc::new(AsyncClosureAction::new(Arc::new(
//                    |game, source, target, ability| {
//                         Box::pin(async move {
//                             if source.lock().await.get_stat_value(StatType::Counter) < 2 {
//                                 return Err("Cannot go below 0".to_string());
//                             }

//                             Card::add_counter(source.clone(), &game, Counter::Incremental(-2)).await;
//                             let owner = source.lock().await.owner.clone();
//                             if let Some(owner) = owner {

//                                 Game::play_token(&game, &owner, create_creature_card!("Token - Ajani's Pridemate", CreatureType::None, "Whenever you gain life, put a +1/+1 counter on Ajani's Pridemate.", 2,2, [], [],

//                                     CardActionTrigger::new(
//                                         ActionTriggerType::HealthGained,
//                                         CardRequiredTarget::None,
//                                         Arc::new(AsyncClosureAction::new(Arc::new(
//                                             |game, source, target, ability| {
//                                                 Box::pin(async move {
//                                                     Card::add_counter(source, &game, Counter::PowerToughnessModifier(1, 1)).await;
//                                                     Ok(())
//                                                 })
//                                             }
//                                         )))
//                                     ))).await?;
//                             }

//                             Ok(())
//                         })
//                     }
//                 ))),
//                 Arc::new(
//                     |game: Arc<Mutex<Game>>,
//                      card: Arc<Mutex<Card>>,
//                      ability_id|
//                      -> Pin<Box<dyn Future<Output = bool> + Send>> {
//                         Box::pin(async move {
//                             let game = game.lock().await;
//                             let total_triggers = game.cards_triggered_this_turn.get(&card.lock().await.id.clone());

//                             return total_triggers.is_none();
//                         })
//                     }
//                 )
//             ),
//             CardActionTrigger::new(
//                 ActionTriggerType::CardStatChanged,
//                 CardRequiredTarget::None,
//                 Arc::new(AsyncClosureAction::new(Arc::new(
//                     |game, source, target, ability| {
//                         Box::pin(async move {
//                             let owner = source.lock().await.owner.clone();
//                             if let Some(card_owner) = owner {
//                                 if source.lock().await.get_stat_value(StatType::Counter) == 0 {
//                                     let health_needed = game.lock().await.starting_health + 15;
//                                     if card_owner.lock().await.get_stat_value(StatType::Health) >= health_needed {
//                                         let cards_to_exile = game.lock().await
//                                             .filter_cards_in_play(Arc::new(
//                                                 move |card_arc: Arc<Mutex<Card>>| -> Pin<
//                                                     Box<dyn Future<Output = bool> + Send>,
//                                                 > {
//                                                     let card_owner = card_owner.clone();
//                                                     let source = source.clone();
//                                                     Box::pin(async move {
//                                                         if let Ok(card) = card_arc.try_lock() {
//                                                             if let Some(owner) = card.owner.clone() {
//                                                                 return Arc::ptr_eq(&card_arc, &source) || (!Arc::ptr_eq(&owner, &card_owner) && vec![CardType::Creature, CardType::Artifact].contains(&card.card_type));
//                                                             }
//                                                         }

//                                                         false
//                                                     })
//                                                 },
//                                             ))
//                                             .await;

//                                         for card in cards_to_exile {
//                                             Game::exile_card(&game, &card).await;
//                                         }
//                                     }
//                                 }
//                             }
//                             Ok(())
//                         })
//                     }
//                 )))
//             ),
//         ],
//         CardPhase::Ready,
//         CardType::Planeswalker,
//         vec![Stat::new(StatType::Counter, 5)],
//         vec![
//             ManaType::Colorless,
//             ManaType::Colorless,
//             ManaType::Purple,
//             ManaType::Purple,
//         ],
//     )
// }

// // fn create_righteous_valkyrie() -> Card {
// //     create_creature_card!(
// //         "Righteous Valkyrie",
// //         CreatureType::Angel,
// //         "Whenever another angel or cleric enters the battlefield under your control, you gain life equal to that creature’s toughness. If you have 27 or more life, creatures you control get +2/+2.",
// //         2,
// //         4,
// //         [ManaType::Purple, ManaType::Colorless, ManaType::Colorless],
// //         // [],
// //         [StatType::Flying],
// //         CardActionTrigger::new(
// //             ActionTriggerType::Continuous,
// //             CardRequiredTarget::None,
// //             Arc::new(ApplyDynamicEffectToCard::new(Arc::new(
// //                     move |card_arc: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = i16> + Send>> {
// //                         Box::pin(async move {
// //                             let owner = {
// //                                 card_arc.lock().await.owner.clone()
// //                             };

// //                             if let Some(owner_arc) = owner {
// //                                 let owner = owner_arc.lock().await;
// //                                 if owner.get_stat_value(StatType::Health) > 26 {
// //                                     2
// //                                 } else {
// //                                     0
// //                                 }
// //                             } else {
// //                                 0
// //                             }
// //                         })
// //                     },
// //                 ),
// //                  Arc::new(
// //                     move |target, source_card, amount, id| -> Pin<Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>> {
// //                         Box::pin(async move {
// //                             let mut effects: Vec<Arc<Mutex<dyn Effect + Send + Sync>>> = vec![];

// //                             let (owner,name,id) = {
// //                                 let card = source_card.clone();
// //                                 let card = card.lock().await;
// //                                 (card.owner.clone(), card.name.clone(), card.id.clone())
// //                             };

// //                             if let Some(owner_arc) = owner {
// //                                 for card in &owner_arc.lock().await.cards_in_play {
// //                                     if card.lock().await.card_type == CardType::Creature {
// //                                         // println!("{} is applying effect to card: {}", name, card.lock().await.name);

// //                                         let mut effect = DynamicStatModifierEffect::new(
// //                                             EffectTarget::Card(card.clone()),
// //                                             StatType::Power,
// //                                             amount.clone(),
// //                                             ExpireContract::Never,
// //                                             Some(source_card.clone()),
// //                                             false,
// //                                         );
// //                                         let id = format!("{}-{}-{}-damage",card.lock().await.id, id, name);
// //                                         effect.id = EffectID(id.clone());

// //                                         effects.push(Arc::new(Mutex::new(effect)));
// //                                         let mut effect = DynamicStatModifierEffect::new(
// //                                             EffectTarget::Card(card.clone()),
// //                                             StatType::Toughness,
// //                                             amount.clone(),
// //                                             ExpireContract::Never,
// //                                             Some(source_card.clone()),
// //                                             false,
// //                                         );

// //                                         let id = format!("{}-{}-{}-defense",card.lock().await.id, id, name);
// //                                         effect.id = EffectID(id.clone());
// //                                         effects.push(Arc::new(Mutex::new(effect)));
// //                                     }
// //                                 }
// //                             }

// //                             effects
// //                         })
// //                     },

// //                 )),
// //             )
// //         )
// //         ,
// //         CardActionTrigger::new(
// //             ActionTriggerType::OtherCardPlayed(PhaseTarget::Owner),
// //             CardRequiredTarget::None,
// //             Arc::new(AsyncClosureAction::new(Arc::new(
// //                 |game: Arc<Mutex<Game>>, source: Arc<Mutex<Card>>, card_played: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
// //                     Box::pin(async move {

// //                         let (owner, creature_type, toughness) = {
// //                             let card = card_played.lock().await;
// //                             let owner = card.owner.clone().unwrap();
// //                             let creature_type = card.creature_type.clone();
// //                             let toughness = card.get_stat_value(StatType::Toughness);
// //                             (owner, creature_type, toughness)
// //                         };

// //                         if creature_type == Some(CreatureType::Angel) {

// //                             game.lock().await.add_health(&owner, toughness).await;
// //                         }
// //                         Ok(())
// //                     })
// //                 }
// //             )))
// //         )

// //     )
// // }

mod test {
    use std::sync::Arc;

    use tokio::sync::{Mutex, RwLock};

    // use crate::game::{
    //     decks::{duplicate_card, white::create_angelic_accord, Deck},
    //     effects::EffectTarget,
    //     mana,
    //     player::Player,
    //     Game,
    // };

    // #[tokio::test]
    // async fn test_angel_1() {
    //     // DeclareAttackerAction
    //     // TurnPhase
    //     // DeclareBlockerAction
    //     // Stat
    //     let mut game = Game::new();
    //     let player = game
    //         .add_player(Player::new(
    //             "test",
    //             0,
    //             duplicate_card(create_righteous_valkyrie(), 4),
    //         ))
    //         .await;

    //     {
    //         player.lock().await.draw_card();
    //         player.lock().await.draw_card();
    //         player.lock().await.draw_card();
    //         player.lock().await.draw_card();
    //     }
    //     game.start_turn(0).await;

    //     let ga = Arc::new(Mutex::new(game));

    //     println!("\n\n\n\nplaying creature");
    //     let a = Game::play_card_from_hand(&ga, &player, 0, None)
    //         .await
    //         .expect("oh");
    //     // Game::process_action_queue(ga.clone(), a.clone()).await;
    //     ga.lock().await.print().await;

    //     println!("\n\n\n\nplaying creature");
    //     let b = Game::play_card_from_hand(&ga, &player, 0, None)
    //         .await
    //         .expect("oh");
    //     // Game::process_action_queue(ga.clone(), b.clone()).await;
    //     ga.lock().await.print().await;

    //     println!("\n\n\n\nplaying creature");
    //     let c = Game::play_card_from_hand(&ga, &player, 0, None)
    //         .await
    //         .expect("oh");
    //     // Game::process_action_queue(ga.clone(), c.clone()).await;
    //     ga.lock().await.print().await;

    //     println!("\n\n\n\nplaying creature");
    //     let d = Game::play_card_from_hand(&ga, &player, 0, None)
    //         .await
    //         .expect("oh");
    //     // Game::process_action_queue(ga.clone(), b.clone()).await;
    //     ga.lock().await.print().await;

    //     println!("\n\n\n\ndestroying creature");
    //     let d = ga.lock().await.destroy_card(&d).await;
    //     // Game::process_action_queue(ga.clone(), b.clone()).await;
    //     ga.lock().await.advance_turn().await;
    //     ga.lock().await.print().await;
    // }
    // #[tokio::test]

    // async fn test_angel_2() {
    //     // DeclareAttackerAction
    //     // TurnPhase
    //     // DeclareBlockerAction
    //     // Stat
    //     let mut game = Game::new();
    //     let player = game
    //         .add_player(Player::new(
    //             "test",
    //             0,
    //             vec![
    //                 create_angelic_accord(),
    //                 create_righteous_valkyrie(),
    //                 create_righteous_valkyrie(),
    //             ],
    //         ))
    //         .await;

    //     {
    //         player.lock().await.draw_card();
    //         player.lock().await.draw_card();
    //         player.lock().await.draw_card();
    //     }
    //     game.start_turn(0).await;

    //     let ga = Arc::new(Mutex::new(game));

    //     println!("\n\n\n\nplaying creature");
    //     let a = Game::play_card_from_hand(&ga, &player, 0, None)
    //         .await
    //         .expect("oh");
    //     // Game::process_action_queue(ga.clone(), a.clone()).await;
    //     ga.lock().await.print().await;

    //     println!("\n\n\n\nplaying creature");
    //     let b = Game::play_card_from_hand(&ga, &player, 0, None)
    //         .await
    //         .expect("oh");
    //     // Game::process_action_queue(ga.clone(), b.clone()).await;
    //     ga.lock().await.print().await;

    //     println!("\n\n\n\nplaying enchantment");
    //     let b = Game::play_card_from_hand(&ga, &player, 0, None)
    //         .await
    //         .expect("oh");
    //     // Game::process_action_queue(ga.clone(), b.clone()).await;
    //     ga.lock().await.print().await;

    //     // println!("\n\n\n\nplaying creature");
    //     // let c = ga
    //     //     .lock()
    //     //     .await
    //     //     .play_card(&player, 0, None)
    //     //     .await
    //     //     .expect("oh");
    //     // Game::process_action_queue(ga.clone(), c.clone()).await;
    //     // ga.lock().await.print().await;

    //     // println!("\n\n\n\nplaying creature");
    //     // let d = ga
    //     //     .lock()
    //     //     .await
    //     //     .play_card(&player, 0, None)
    //     //     .await
    //     //     .expect("oh");
    //     // Game::process_action_queue(ga.clone(), b.clone()).await;
    //     // ga.lock().await.print().await;

    //     for _ in 0..11 {
    //         ga.lock().await.advance_turn().await;
    //     }
    //     ga.lock().await.print().await;
    // }
}
