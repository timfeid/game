use crate::game::action::{CardActionWrapper, PlayCardAction};
use crate::game::card::{CardPhase, Counter};
use crate::game::player::Player;
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
use crate::game::{ActionType, CardWithDetails};
use futures::future::join_all;
use rand::rngs::SmallRng;
use rand::{seq::SliceRandom, Rng, SeedableRng};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::vec;
use tokio::sync::Mutex;
use ulid::Ulid;

use super::duplicate_card;

fn create_ace_of_spades() -> Card {
    CardBuilder::new()
        .name("Ace of Spades, Gambler's Guardian")
        .description(
            "Whenever Ace of Spades deals combat damage to a player, you may look at that player's hand. Choose a nonland card from it; that player discards that card.",
        )
        .creature(2, 3)
        .mana_cost(vec![ManaType::Colorless, ManaType::Influence, ManaType::Influence])
        .add_action(
            ActionBuilder::new(ActionTriggerType::DamageApplied).optional_closure_action_passing_target(
                "Whenever Ace of Spades deals combat damage to a player, you may look at that player's hand. Choose a nonland card from it; that player discards that card.",
                |game, source, owner, target, ability_id| {
                    Box::pin(async move {
                        if let Some(FrontendTarget::Player(player_id)) = &target {
                            if let Ok((_, player)) =
                                Game::player_from_id(&game, player_id).await
                            {
                                let cards_in_hand = player.lock().await.cards_in_hand.clone();

                                if !cards_in_hand.is_empty() {
                                    let game_cloned = game.clone();
                                    let player_id = owner.lock().await.name.clone();
                                    Game::choose_card(
                                        &game,
                                        player_id,
                                        cards_in_hand,
                                        move |card| {
                                            let game = game_cloned.clone();
                                            Box::pin(async move {
                                                if card.target.lock().await.card_type.is_spell() {
                                                    Game::destroy_card(&game, &card.target).await?;
                                                    Ok(())
                                                } else {
                                                    Err("Please select a non-land card.".to_string())
                                                }
                                            })
                                        },
                                        "Whenever Ace of Spades deals combat damage to a player, you may look at that player's hand. Choose a nonland card from it; that player discards that card.",
                                    false,
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
            None,
            ),
        )
        .build()
}

fn create_slot_machine() -> Card {
    CardBuilder::new()
        .name("Slot Machine")
        .description("Reveal the top three cards of your library. If all three share a card type, you may cast one of them without paying its mana cost. Put the rest into your graveyard.")
        .card_type(CardType::Artifact)
        .mana_cost(vec![ManaType::Influence, ManaType::Influence])
        .phase(CardPhase::Ready)
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Reveal the top three cards of your library. If all three share a card type, you may cast one of them without paying its mana cost. Put the rest into your graveyard."
                    .to_string(),
                vec![ManaType::Influence, ManaType::Influence],
                None,
                true,
                CardRequiredTarget::None,
            ))
            .closure_action(|game, source, player, target, ability_id| {
                Box::pin(async move {

                    let cards: Vec<Arc<Mutex<Card>>> = player.lock().await.deck.draw_pile.iter().rev().take(3).cloned().collect();
                    let card_types: Vec<_> = join_all(cards.iter().map(|card| async {
                        card.lock().await.card_type.clone()
                    }))
                    .await;
                    let won = card_types.windows(2).all(|w| w[0] == w[1]);

                    let val = format!("The cards types match! Enjoy your {:?}s", card_types[0]);

                    let message = if won {
                        val.as_str()
                    } else {
                        Game::destroy_card(&game, &cards[0]).await?;
                        Game::destroy_card(&game, &cards[1]).await?;
                        Game::destroy_card(&game, &cards[2]).await?;

                        "Unfortunately, the house won this time."
                    };

                    let player_id = player.lock().await.name.clone();
                    Game::show_cards_to_all(
                        &game,
                        cards.clone(),
                        message,
                    )
                    .await;

                    if won {
                        let game_cloned = game.clone();
                        Game::choose_card(
                            &game,
                            player_id,
                            cards,
                            move |card| {
                                let game = game_cloned.clone();
                                Box::pin(async move {
                                    Game::execute_actions(game, vec![
                                        Arc::new(CardActionWrapper::new(PlayCardAction {}, card.target, None, None))
                                    ]).await?;
                                    Ok(())
                                })
                            },
                            "You may cast one of them without paying its mana cost",
                            false,
                        )
                        .await;
                    }


                    Ok(())
                })
            })
        )
        .build()
}

fn create_sic_bo() -> Card {
    CardBuilder::new()
        .name("Sic Bo")
        .description("Look at the top three cards of your library. You may rearrange them in any order or put them on the bottom of your library.")
        .card_type(CardType::Instant)
        // .mana_cost(vec![ManaType::Influence, ManaType::Influence])
        .add_action(
            ActionBuilder::new(ActionTriggerType::CardEnteredBattlefield)
            .closure_action(|game, source, player, target, ability_id| {
                Box::pin(async move {
                    let cards_: Vec<Arc<Mutex<Card>>> = player.lock().await.deck.draw_pile.iter().rev().take(3).cloned().collect();
                    let mut cards = vec![];
                    for card in cards_ {
                        cards.push(CardWithDetails::from_card_arc(game.clone(), card.clone()).await);
                    }

                    Game::scry(
                        game.clone(),
                        player.clone(),
                        cards,
                    )
                    .await?;

                    Ok(())
                })
            })
        )
        .build()
}

fn create_craps_shooter() -> Card {
    CardBuilder::new()
        .name("Craps Shooter")
        .creature(1, 1)
        // .mana_cost(vec![ManaType::Colorless, ManaType::Fortune])
        .play_target(CardRequiredTarget::ChosenBattlefield)
        .add_action(
            ActionBuilder::new(ActionTriggerType::AbilityWithinPhases(
                "Reveal the top two cards of your library. If both cards share the same card type, put them into your hand. Otherwise, put them into your graveyard."
                    .to_string(),
                vec![ManaType::Fortune],
                None,
                true,
                CardRequiredTarget::None,
            ))
            .closure_action(|game, source, player, target, ability_id| {
                Box::pin(async move {
                    let mut cards = vec![];
                    cards.push(Game::draw_card(game.clone(), player.clone(), Some(source.clone())).await?);
                    cards.push(Game::draw_card(game.clone(), player.clone(), Some(source.clone())).await?);

                    let card_type = cards[0].lock().await.card_type.clone();
                    let card_type2 = cards[1].lock().await.card_type.clone();
                    let val = format!("The cards types match! Enjoy your {:?}s", card_type);
                    let message = if card_type == card_type2 {
                        val.as_str()
                    } else {
                        Game::destroy_card(&game, &cards[0]).await?;
                        Game::destroy_card(&game, &cards[1]).await?;

                        "Unfortunately, the house won this time."
                    };

                    Game::show_cards_to_all(
                        &game,
                        cards,
                        message,
                    )
                    .await;

                    Ok(())
                })
            })
        )
        .build()
}

pub fn create_vegas_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];

    // deck.append(&mut duplicate_card(create_showgirl_performer(), 4));
    // deck.append(&mut duplicate_card(create_pickpocket(), 4));
    // deck.append(&mut duplicate_card(create_mind_reader(), 4));
    // deck.append(&mut duplicate_card(create_high_roller(), 4));
    // deck.append(&mut duplicate_card(create_ace_of_spades(), 4));
    // deck.append(&mut duplicate_card(create_test_mana(), 4));
    deck.append(&mut duplicate_card(create_craps_shooter(), 4));
    deck.append(&mut duplicate_card(create_sic_bo(), 4));
    // deck.append(&mut duplicate_card(create_slot_machine(), 4));
    // deck.append(&mut duplicate_card(create_dealers_enforcer(), 4));
    // deck.append(&mut duplicate_card(create_blackjack_dealer(), 4));
    // deck.append(&mut duplicate_card(create_poker_pro(), 4));
    // deck.append(&mut duplicate_card(create_card_counter(), 4));
    // deck.append(&mut duplicate_card(create_mind_games(), 4));

    // deck.append(&mut duplicate_card(create_casino_grounds(), 4));
    // deck.append(&mut duplicate_card(create_gold(), 14));
    // deck.append(&mut duplicate_card(create_blue(), 6));

    deck
}
