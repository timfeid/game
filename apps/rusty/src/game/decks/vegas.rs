use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::vec;

use rand::rngs::SmallRng;
use rand::{seq::SliceRandom, Rng, SeedableRng};

use tokio::sync::Mutex;
use ulid::Ulid;

use crate::game::action::{
    Action, AsyncClosureAction, BlankAction, CardAction, CardActionWrapper,
    CastOptionalAdditionalAbility, ChooseFromSelectionAction,
};
use crate::game::card::Counter;
use crate::game::player::Player;
use crate::game::ActionType;
use crate::game::{
    action::{
        generate_mana::GenerateManaAction, ActionBuilder, ActionTriggerType, CardRequiredTarget,
        PhaseTarget, PlayerActionTarget,
    },
    card::{Card, CardBuilder, CardType},
    effects::{EffectID, EffectTarget, ExpireContract, StatModifierEffect},
    mana::ManaType,
    stat::{StatType, Stats},
    turn::TurnPhase,
    FrontendTarget, Game,
};

use super::duplicate_card;
fn create_blue() -> Card {
    CardBuilder::new()
        .name("Illusionary Lake")
        .card_type(CardType::BasicLand(ManaType::Blue))
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Adds {B} to your pool.".to_string(),
                vec![],
                None,
                true,
                CardRequiredTarget::None,
            ))
            .action(GenerateManaAction {
                mana_to_add: vec![ManaType::Blue],
                target: PlayerActionTarget::Owner,
            }),
        )
        .build()
}

fn create_gold() -> Card {
    CardBuilder::new()
        .name("Treasure Trove")
        .card_type(CardType::BasicLand(ManaType::Gold))
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Adds {D} to your pool.".to_string(),
                vec![],
                None,
                true,
                CardRequiredTarget::None,
            ))
            .action(GenerateManaAction {
                mana_to_add: vec![ManaType::Gold],
                target: PlayerActionTarget::Owner,
            }),
        )
        .build()
}

fn create_test_red() -> Card {
    CardBuilder::new()
        .name("Blue")
        .card_type(CardType::BasicLand(ManaType::Blue))
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
                    ManaType::Blue,
                    ManaType::Blue,
                    ManaType::Blue,
                    ManaType::Blue,
                    ManaType::Blue,
                    ManaType::Blue,
                    ManaType::Gold,
                    ManaType::Gold,
                    ManaType::Gold,
                    ManaType::Gold,
                    ManaType::Gold,
                ],
                target: PlayerActionTarget::Owner,
            }),
        )
        .build()
}

fn create_mind_reader() -> Card {
    CardBuilder::new()
        .name("Mind Reader")
        .description("When Mind Reader enters the battlefield, look at target player's hand.")
        .creature(2, 2)
        .mana_cost(vec![ManaType::Colorless, ManaType::Blue, ManaType::Gold])
        .play_target(CardRequiredTarget::AnyPlayer)
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Draw a card.".to_string(),
                vec![ManaType::Blue, ManaType::Blue],
                None,
                false,
                CardRequiredTarget::None,
            ))
            .closure_action(|game, source, player, target, ability_id| {
                Box::pin(async move {
                    println!("drawing card....");
                    Game::draw_card(game, player, Some(source)).await?;
                    Ok(())
                })
            }),
        )
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardEnteredBattlefield).closure_action(
                |game,
                 source,
                 owner,
                 target,
                 ability_id|
                 -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
                    Box::pin(async move {
                        let player_id = owner.lock().await.name.clone();
                        if let Ok((_, player)) =
                            Game::player_from_frontend_target(&game, &target.unwrap()).await
                        {
                            let cards = player.lock().await.cards_in_hand.clone();
                            Game::show_cards(
                                &game,
                                player_id,
                                cards,
                                "When Mind Reader enters the battlefield, look at target player's hand.",
                            )
                            .await;
                        }
                        Ok(())
                    })
                },
            ),
        )
        .build()
}

fn create_high_roller() -> Card {
    CardBuilder::new()
        .name("High Roller")
        .description("Whenever High Roller attacks, gain 1 Luck Token.")
        .creature(3, 2)
        .mana_cost(vec![ManaType::Colorless, ManaType::Blue, ManaType::Gold])
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardAttacked)
            .closure_action(|game, source, player, target, ability_id| {
                Box::pin(async move {
                    Game::add_stat(&game, &source, &player, StatType::LuckToken, 1).await;

                    Ok(())
                })
            })
            .requirements(|game, source, owner, ability_id| {
                Box::pin(async move { owner.lock().await.get_stat_value(StatType::LuckToken) > 0 })
            }),
        )
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Pay 1 Luck Token: Flip a coin. If heads, High Roller gets +2/+2 until end of turn. If tails, High Roller gets -1/-1 until end of turn."
                    .to_string(),
                vec![],
                None,
                false,
                CardRequiredTarget::None,
            ))
            .closure_action(|game, source, player, target, ability_id| {
                Box::pin(async move {
                    Game::add_stat(&game, &source, &player, StatType::LuckToken, -1).await;
                    let won = Game::slot_machine_minigame(&game, &player, 50, "High Roller gets +2/+2 until end of turn", "High Roller gets -1/-1 until end of turn.").await;
                        tokio::spawn(async move {
                            let amount = if won { 2 } else {-1};
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            Game::apply_effect(
                                &game,
                                StatModifierEffect::new(
                                    format!("counter-{}-{}-power", Ulid::new().to_string(), ability_id),
                                    EffectTarget::Card(source.clone()),
                                    StatType::Power,
                                    amount.clone(),
                                    ExpireContract::Turns(1),
                                    None,
                                ),
                            ).await;
                            Game::apply_effect(
                                &game,
                                StatModifierEffect::new(
                                    format!("counter-{}-{}-toughness", Ulid::new().to_string(), ability_id),
                                    EffectTarget::Card(source.clone()),
                                    StatType::Toughness,
                                    amount,
                                    ExpireContract::Turns(1),
                                    None,
                                ),
                            ).await;

                        });

                    Ok(())
                })
            })
            .requirements(|game, source, owner, ability_id| {
                Box::pin(async move { owner.lock().await.get_stat_value(StatType::LuckToken) > 0 })
            }),
        )
        .build()
}

fn create_lucky_gambler() -> Card {
    CardBuilder::new()
        .name("Lucky Gambler")
        .description("When Lucky Gambler enters the battlefield, roll a dice. Spin the slot machine for a 33% chance to win a Luck Token.")
        .creature(1, 1)
        .mana_cost(vec![ManaType::Blue])
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardEnteredBattlefield)
            .closure_action(|game, source, player, target, _| {
                Box::pin(async move {
                    let won = Game::slot_machine_minigame(&game, &player, 33, "Your received a Luck Token", "Sorry, no Luck Token this time.").await;
                    if won {
                        tokio::spawn(async move {

                            tokio::time::sleep(tokio::time::Duration::from_secs(1000)).await;
                            Game::add_stat(&game, &source, &player, StatType::LuckToken, 1).await;

                        });
                    }

                    Ok(())
                })
            })
        )
        .build()
}

fn create_pickpocket() -> Card {
    CardBuilder::new()
        .name("Pickpocket")
        .description(
            "When Pickpocket deals combat damage to a player, that player discards a card.",
        )
        .creature(1, 1)
        .mana_cost(vec![ManaType::Blue])
        .add_action(
            ActionBuilder::new(ActionTriggerType::DamageApplied).closure_action(
                |game, source, owner, target, ability_id| {
                    Box::pin(async move {
                        if let Some(FrontendTarget::Player(player_id)) = &target {
                            if let Ok((_, player)) =
                                Game::player_from_frontend_target(&game, &target.unwrap()).await
                            {
                                let cards_in_hand = player.lock().await.cards_in_hand.clone();

                                if !cards_in_hand.is_empty() {
                                    let game_cloned = game.clone();
                                    let player_id = player.lock().await.name.clone();
                                    Game::choose_card(
                                        &game,
                                        player_id,
                                        cards_in_hand,
                                        move |card| {
                                            let game = game_cloned.clone();
                                            Box::pin(async move {
                                                // card.target.lock().await
                                                println!("destroying card");
                                                Game::destroy_card(&game, &card.target).await?;
                                                Ok(())
                                            })
                                        },
                                        "When Pickpocket deals combat damage to a player, that player discards a card.",
                                    )
                                    .await;
                                } else {
                                    return Err("Target player has no cards in play".into());
                                }
                            }
                        }
                        Ok(())
                    })
                },
            ),
        )
        .build()
}

fn create_mind_games() -> Card {
    CardBuilder::new()
        .name("Mind Games")
        .description("Counter target spell unless its controller pays 2 mana.")
        .card_type(CardType::Instant)
        .mana_cost(vec![ManaType::Colorless, ManaType::Gold])
        .play_target(CardRequiredTarget::Spell)
        .add_action(
            ActionBuilder::new(ActionTriggerType::Instant).closure_action(
                |game, source, owner, target, ability_id| {
                    let cloned_game = game.clone();
                    Box::pin(async move {
                        // Clone variables to avoid moving them
                        let game = Arc::clone(&game);
                        let source = Arc::clone(&source);
                        let ability_id = ability_id.clone();
                        let target = target.clone();

                        // Ensure target is provided
                        if let Some(target) = target {
                            // Get the target spell (card)
                            let target_card =
                                Game::card_from_frontend_target(&cloned_game, &target).await;
                            // Get the owner of the target spell
                            let target_card_owner = {
                                let target_card_guard = target_card.lock().await;
                                &target_card_guard.owner.clone().unwrap()
                            };

                            // Create the optional additional ability for the target player
                            let action = Arc::new(CardActionWrapper {
                                ability_id: Some(ability_id.clone()),
                                card: Arc::clone(&target_card),
                                action: Arc::new(
                                    CastOptionalAdditionalAbility::new(
                                        vec![ManaType::Colorless, ManaType::Colorless], // 2 mana cost
                                        CardRequiredTarget::None,
                                        move |_card| Arc::new(BlankAction {}),
                                        "Pay 2 mana to prevent your spell from being countered."
                                            .to_string(),
                                        ActionType::None,
                                    )
                                    .canceled(move |_card| {
                                        let target_card = Arc::clone(&target_card);
                                        Arc::new(AsyncClosureAction::new(
                                            move |_game, _source, _owner, _target, _ability_id| {
                                                let target_card = target_card.clone();
                                                Box::pin(async move {
                                                    target_card.lock().await.is_countered = true;
                                                    Ok(())
                                                })
                                            },
                                        ))
                                    }),
                                )
                                    as Arc<(dyn CardAction + std::marker::Send + Sync + 'static)>,
                                target: None,
                            });

                            // Execute the action
                            Game::execute_actions(game, vec![action]).await?;
                        } else {
                            // Error: No target provided
                            return Err("No target spell selected.".into());
                        }

                        Ok(())
                    })
                },
            ),
        )
        .build()
}

fn create_card_counter() -> Card {
    CardBuilder::new()
        .name("Card Counter")
        .description(
            "Whenever you draw a card outside your draw step, put a +1/+1 counter on Card Counter.",
        )
        .creature(1, 1)
        .mana_cost(vec![ManaType::Blue])
        .add_action(
            ActionBuilder::new(ActionTriggerType::OtherCardDrawn(PhaseTarget::Owner))
                .closure_action(|game, source, owner, target, ability_id| {
                    Box::pin(async move {
                        println!("hello ");
                        if game.lock().await.current_phase() != TurnPhase::Draw {
                            Card::add_counter(source, &game, Counter::PowerToughnessModifier(1, 1))
                                .await;
                        }
                        Ok(())
                    })
                }),
        )
        .build()
}

fn create_poker_pro() -> Card {
    CardBuilder::new()
        .name("Poker Pro")
        .description("When Poker Pro attacks, you may look at target player's hand.")
        .creature(3, 2)
        .mana_cost(vec![ManaType::Colorless, ManaType::Blue, ManaType::Blue])
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardAttacked)
                .closure_action(|game, source, owner, target, ability_id| {
                    Box::pin(async move {
                        // Clone variables to avoid moving them into closures
                        let source = Arc::clone(&source);
                        let ability_id = ability_id.clone();
                        let target = target.clone();

                        let action = Arc::new(CardActionWrapper {
                            ability_id: Some(ability_id.clone()),
                            card: Arc::clone(&source),
                            action: Arc::new(CastOptionalAdditionalAbility::new(
                                vec![],
                                CardRequiredTarget::None,
                                move |_card| {
                                    let target = target.clone();

                                    Arc::new(AsyncClosureAction::new(
                                        move |game, source, owner, _target, _ability_id| {
                                            let target = target.clone();
                                            Box::pin(async move {
                                                let owner_name = {
                                                    // Lock the owner only to get the name
                                                    let owner_guard = owner.lock().await;
                                                    owner_guard.name.clone()
                                                };

                                                // Ensure target is Some
                                                if let Some(target) = &target {
                                                    if let Ok((_, target_player)) =
                                                        Game::player_from_frontend_target(
                                                            &game,
                                                            target,
                                                        )
                                                        .await
                                                    {
                                                        let cards_in_hand = {
                                                            // Lock the target player to get their hand
                                                            let target_player_guard =
                                                                target_player.lock().await;
                                                            target_player_guard
                                                                .cards_in_hand
                                                                .clone()
                                                        };

                                                        // Show the cards to the owner
                                                        Game::show_cards(
                                                            &game,
                                                            owner_name,
                                                            cards_in_hand,
                                                            "When Poker Pro attacks, you may look at target player's hand.",
                                                        )
                                                        .await;
                                                    } else {
                                                        // Handle error if player not found
                                                        return Err(
                                                            "Target player not found.".into()
                                                        );
                                                    }
                                                } else {
                                                    // Handle case where target is None
                                                    return Err("No target selected.".into());
                                                }
                                                Ok(())
                                            })
                                        },
                                    ))
                                },
                                "When Poker Pro attacks, you may look at target player's hand."
                                    .to_string(),
                                ActionType::None,
                            )),
                            target: None,
                        });

                        // Execute the action
                        Game::execute_actions(game.clone(), vec![action]).await?;

                        Ok(())
                    })
                }),
        )
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Draw a card, then discard a card".to_string(),
                vec![ManaType::Blue],
                None,
                false,
                CardRequiredTarget::None,
            ))
            .closure_action(|game, source, owner, _target, _ability_id| {
                Box::pin(async move {
                    // Clone variables to avoid moving them into closures
                    let game = Arc::clone(&game);
                    let source = Arc::clone(&source);
                    let owner = Arc::clone(&owner);

                    // Draw a card
                    Game::draw_card(game.clone(), owner.clone(), Some(source.clone())).await?;

                    let owner_name = {
                        let owner_guard = owner.lock().await;
                        owner_guard.name.clone()
                    };

                    let cards_in_hand = {
                        let owner_guard = owner.lock().await;
                        owner_guard.cards_in_hand.clone()
                    };

                    let game_cloned = game.clone();

                    // Choose a card to discard
                    Game::choose_card(
                        &game,
                        owner_name,
                        cards_in_hand,
                        move |card| {
                            let game = game_cloned.clone();
                            let card_to_destroy = Arc::clone(&card.target);
                            Box::pin(async move {
                                println!("Destroying card");
                                Game::destroy_card(&game, &card_to_destroy).await?;
                                Ok(())
                            })
                        },
                        "Discard a card",
                    )
                    .await;

                    Ok(())
                })
            }),
        )
        .build()
}

fn create_blackjack_dealer() -> Card {
    CardBuilder::new()
        .name("Goldjack Dealer")
        .description("When Goldjack Dealer enters the battlefield, each player draws a card.")
        // .mana_cost(vec![ManaType::Blue, ManaType::Gold])
        .creature(2, 2)
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardEnteredBattlefield).closure_action(
                |game, source, player, target, _| {
                    Box::pin(async move {
                        let players = game.lock().await.players.clone();
                        for player in &players {
                            println!("draw card.");
                            Game::draw_card(game.clone(), Arc::clone(player), Some(source.clone()))
                                .await?;
                        }
                        Ok(())
                    })
                },
            ),
        )
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Pay 1 Luck Token, Tap: Target player discards a random card.".to_string(),
                vec![],
                None,
                true,
                CardRequiredTarget::AnyPlayer,
            ))
            .closure_action(|game, source, owner, target, ability_id| {
                Box::pin(async move {
                    if let Ok((_, player)) =
                        Game::player_from_frontend_target(&game, &target.unwrap()).await
                    {
                        let cards_in_hand = player.lock().await.cards_in_hand.clone();

                        if !cards_in_hand.is_empty() {
                            Game::add_stat(&game, &source, &owner, StatType::LuckToken, -1).await;
                            let mut rng = SmallRng::from_entropy();

                            if let Some(card) = cards_in_hand.choose(&mut rng) {
                                if let Ok(target) =
                                    Game::frontend_target_from_card(&game, card).await
                                {
                                    game.lock().await.remove_from_frontend_target(&target).await;
                                    return Ok(());
                                } else {
                                    return Err("Hmm".to_string());
                                }
                            } else {
                                return Err("Failed to select a card".into());
                            }
                        } else {
                            return Err("Target player has no cards in play".into());
                        }
                    } else {
                        return Err("Invalid target: Expected a player".into());
                    }
                })
            })
            .requirements(|game, source, owner, ability_id| {
                Box::pin(async move { owner.lock().await.get_stat_value(StatType::LuckToken) > 0 })
            }),
        )
        .build()
}

fn create_casino_grounds() -> Card {
    CardBuilder::new()
        .name("Casino Grounds")
        .description("Generates one Luck Token each turn.")
        .card_type(CardType::AdvancedMultiLand(ManaType::Gold, ManaType::Blue))
        .add_action(
            ActionBuilder::new(ActionTriggerType::PhaseStarted(
                vec![TurnPhase::Untap],
                PhaseTarget::Owner,
            ))
            .closure_action(|game, source, player, target, _| {
                Box::pin(async move {
                    Game::add_stat(&game, &source, &player, StatType::LuckToken, 1).await;
                    Ok(())
                })
            }),
        )
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Adds {Bk} to your mana pool as long as you have at least 1 Luck Token."
                    .to_string(),
                vec![],
                None,
                true,
                CardRequiredTarget::None,
            ))
            .action(GenerateManaAction {
                mana_to_add: vec![ManaType::Gold],
                target: PlayerActionTarget::Owner,
            })
            .requirements(|game, source, owner, ability_id| {
                Box::pin(async move { owner.lock().await.get_stat_value(StatType::LuckToken) > 0 })
            }),
        )
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Adds {R} to your mana pool as long as you have at least 1 Luck Token.".to_string(),
                vec![],
                None,
                true,
                CardRequiredTarget::None,
            ))
            .action(GenerateManaAction {
                mana_to_add: vec![ManaType::Blue],
                target: PlayerActionTarget::Owner,
            })
            .requirements(|game, source, owner, ability_id| {
                Box::pin(async move { owner.lock().await.get_stat_value(StatType::LuckToken) > 0 })
            }),
        )
        .build()
}

pub fn create_vegas_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];

    deck.append(&mut duplicate_card(create_showgirl_performer(), 4));
    deck.append(&mut duplicate_card(create_pickpocket(), 4));
    deck.append(&mut duplicate_card(create_mind_reader(), 4));
    deck.append(&mut duplicate_card(create_high_roller(), 4));
    deck.append(&mut duplicate_card(create_blackjack_dealer(), 4));
    deck.append(&mut duplicate_card(create_poker_pro(), 4));
    deck.append(&mut duplicate_card(create_card_counter(), 4));
    deck.append(&mut duplicate_card(create_mind_games(), 4));

    deck.append(&mut duplicate_card(create_casino_grounds(), 4));
    deck.append(&mut duplicate_card(create_gold(), 14));
    deck.append(&mut duplicate_card(create_blue(), 6));

    deck
}
