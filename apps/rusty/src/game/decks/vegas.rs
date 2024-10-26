use rand::rngs::SmallRng;
use rand::{seq::SliceRandom, Rng, SeedableRng};

use tokio::sync::Mutex;
use ulid::Ulid;

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
fn create_test_red() -> Card {
    CardBuilder::new()
        .name("Red")
        .description("")
        .card_type(CardType::BasicLand(ManaType::Red))
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
                    ManaType::Red,
                    ManaType::Red,
                    ManaType::Red,
                    ManaType::Red,
                    ManaType::Red,
                    ManaType::Red,
                    ManaType::Red,
                    ManaType::Red,
                    ManaType::Red,
                    ManaType::Red,
                    ManaType::Red,
                ],
                target: PlayerActionTarget::Owner,
            }),
        )
        .build()
}

fn create_high_roller() -> Card {
    CardBuilder::new()
        .name("High Roller")
        .description("Whenever High Roller attacks, gain 1 Luck Token.")
        .creature(3, 2)
        .mana_cost(vec![ManaType::Colorless, ManaType::Red, ManaType::Black])
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

fn create_showgirl_performer() -> Card {
    CardBuilder::new()
        .name("Showgirl Performer")
        .description("When Showgirl Performer enters the battlefield, gain 1 Luck Token.")
        .creature(1, 1)
        .mana_cost(vec![ManaType::Red])
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardEnteredBattlefield).closure_action(
                |game, source, player, target, _| {
                    Box::pin(async move {
                        Game::add_stat(&game, &source, &player, StatType::LuckToken, 1).await;
                        Ok(())
                    })
                },
            ),
        )
        .build()
}

fn create_lucky_gambler() -> Card {
    CardBuilder::new()
        .name("Lucky Gambler")
        .description("When Lucky Gambler enters the battlefield, roll a dice. Spin the slot machine for a 33% chance to win a Luck Token.")
        .creature(1, 1)
        .mana_cost(vec![ManaType::Red])
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

fn create_blackjack_dealer() -> Card {
    CardBuilder::new()
        .name("Blackjack Dealer")
        .description("When Blackjack Dealer enters the battlefield, each player draws a card.")
        .creature(2, 2)
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardEnteredBattlefield).closure_action(
                |game, source, player, target, _| {
                    Box::pin(async move {
                        let players = game.lock().await.players.clone();
                        for player in players {
                            player.lock().await.draw_card();
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
        .card_type(CardType::AdvancedMultiLand(ManaType::Black, ManaType::Red))
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
                mana_to_add: vec![ManaType::Black],
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
                mana_to_add: vec![ManaType::Red],
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

    // deck.append(&mut duplicate_card(create_casino_grounds(), 4));
    deck.append(&mut duplicate_card(create_showgirl_performer(), 4));
    deck.append(&mut duplicate_card(create_test_red(), 1));
    // deck.append(&mut duplicate_card(create_high_roller(), 4));
    deck.append(&mut duplicate_card(create_blackjack_dealer(), 4));
    // deck.append(&mut duplicate_card(create_casino_grounds(), 1));

    deck
}
