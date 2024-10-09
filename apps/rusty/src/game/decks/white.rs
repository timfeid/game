use crate::game::{
    action::{
        generate_mana::GenerateManaAction, ActionTriggerType, ApplyDynamicEffectToCard,
        ApplyEffectToCardBasedOnTotalCardType, AsyncClosureAction, AsyncClosureWithCardAction,
        CardActionTrigger, CardRequiredTarget, CardTargetTeam, DeclareAttackerAction,
        DeclareBlockerAction, PlayerActionTarget, TriggerTarget,
    },
    card::{
        card::{create_creature_card, create_multiple_cards},
        Card, CardPhase, CardType, CreatureType,
    },
    effects::{DynamicStatModifierEffect, Effect, EffectID, EffectTarget, ExpireContract},
    mana::ManaType,
    player::Player,
    stat::{Stat, StatType, Stats},
    turn::TurnPhase,
    Game,
};
use std::{f32::consts::E, future::Future, mem::zeroed, pin::Pin, sync::Arc};

use tokio::sync::Mutex;
use ulid::Ulid;

use super::duplicate_card;

fn create_plains() -> Card {
    Card::new(
        "Plains",
        "",
        vec![CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Adds {W} white mana to your pool.".to_string(),
                vec![],
                None,
                true,
            ),
            CardRequiredTarget::None,
            Arc::new(GenerateManaAction {
                mana_to_add: vec![ManaType::White],
                target: PlayerActionTarget::Owner,
            }),
        )],
        CardPhase::Ready,
        CardType::BasicLand(ManaType::White),
        vec![],
        vec![],
    )
}

pub fn create_angels_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    // deck.push();
    deck.append(&mut duplicate_card(create_righteous_valkyrie(), 4));
    deck.append(&mut duplicate_card(create_angelic_accord(), 4));
    deck.append(&mut duplicate_card(create_plains(), 12));

    deck
}

fn create_angelic_accord() -> Card {
    Card::new(
        "Angelic Accord",
        "At the beginning of each end step, if you gained 4 or more life this turn, create a 4/4 white Angel creature token with flying.",
        vec![
            CardActionTrigger::new(
                ActionTriggerType::PhaseStarted(vec![TurnPhase::End], TriggerTarget::Any),
                CardRequiredTarget::None,
                Arc::new(AsyncClosureAction::new(Arc::new(
                    |game: Arc<Mutex<Game>>, card: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = ()> + Send>> {
                        Box::pin(async move {
                            let (difference, owner) = {

                            let card = card.lock().await;
                            let owner_arc = card.owner.clone().unwrap();
                            let owner = owner_arc.lock().await;
                            let health = owner.stat_manager.get_stat_value(StatType::Health);
                                (health - owner.health_at_start_of_round, owner_arc.clone())
                            };
                            println!("difference: {}", difference);
                            if difference > 3 {
                                Game::play_token(&game, &owner, create_creature_card!("Token", CreatureType::Angel, "", 4,4, [], [])).await.ok();
                            }
                        })
                    }
                )))
            ),
            CardActionTrigger::new(
                ActionTriggerType::Attached,
                CardRequiredTarget::CardOfType(CardType::Creature, CardTargetTeam::Any),
                Arc::new(ApplyEffectToCardBasedOnTotalCardType {
                    card_type: CardType::BasicLand(ManaType::Green),

                    effects_generator: Arc::new(|target, source_card, amount_calculator| {
                        vec![Arc::new(Mutex::new(DynamicStatModifierEffect::new(
                            target,
                            StatType::Toughness,
                            amount_calculator.clone(),
                            ExpireContract::Never,
                            source_card.clone(),
                            false,
                        )))]
                    }),
                }),
            ),
        ],
        CardPhase::Ready,
        CardType::Enchantment,
        vec![],
        // vec![ManaType::White, ManaType::Colorless, ManaType::Colorless, ManaType::Colorless],
        vec![],
    )
}

fn create_righteous_valkyrie() -> Card {
    create_creature_card!(
        "Righteous Valkyrie",
        CreatureType::Angel,
        "Whenever another angel or cleric enters the battlefield under your control, you gain life equal to that creature’s toughness. If you have 27 or more life, creatures you control get +2/+2.",
        2,
        4,
        // [ManaType::White, ManaType::Colorless, ManaType::Colorless],
        [],
        [StatType::Flying],
        CardActionTrigger::new(
            ActionTriggerType::Continuous,
            CardRequiredTarget::None,
            Arc::new(ApplyDynamicEffectToCard::new(Arc::new(
                    move |card_arc: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = i8> + Send>> {
                        Box::pin(async move {
                            let owner = {
                                card_arc.lock().await.owner.clone()
                            };

                            if let Some(owner_arc) = owner {
                                let owner = owner_arc.lock().await;
                                if owner.get_stat_value(StatType::Health) > 26 {
                                    2
                                } else {
                                    0
                                }
                            } else {
                                0
                            }
                        })
                    },
                ),
                 Arc::new(
                    move |target, source_card, amount, id| -> Pin<Box<dyn Future<Output = Vec<Arc<Mutex<dyn Effect + Send + Sync>>>> + Send>> {
                        Box::pin(async move {
                            let mut effects: Vec<Arc<Mutex<dyn Effect + Send + Sync>>> = vec![];

                            let (owner,name,id) = {
                                let card = source_card.clone();
                                let card = card.lock().await;
                                (card.owner.clone(), card.name.clone(), card.id.clone())
                            };

                            if let Some(owner_arc) = owner {
                                for card in &owner_arc.lock().await.cards_in_play {
                                    if card.lock().await.card_type == CardType::Creature {
                                        // println!("{} is applying effect to card: {}", name, card.lock().await.name);

                                        let mut effect = DynamicStatModifierEffect::new(
                                            EffectTarget::Card(card.clone()),
                                            StatType::Power,
                                            amount.clone(),
                                            ExpireContract::Never,
                                            Some(source_card.clone()),
                                            false,
                                        );
                                        let id = format!("{}-{}-{}-damage",card.lock().await.id, id, name);
                                        effect.id = EffectID(id.clone());

                                        effects.push(Arc::new(Mutex::new(effect)));
                                        let mut effect = DynamicStatModifierEffect::new(
                                            EffectTarget::Card(card.clone()),
                                            StatType::Toughness,
                                            amount.clone(),
                                            ExpireContract::Never,
                                            Some(source_card.clone()),
                                            false,
                                        );

                                        let id = format!("{}-{}-{}-defense",card.lock().await.id, id, name);
                                        effect.id = EffectID(id.clone());
                                        effects.push(Arc::new(Mutex::new(effect)));
                                    } else {
                                        println!("{} is skipping {}, not right type", name, card.lock().await.name);
                                    }
                                }
                            }

                            effects
                        })
                    },

                )),
            )
        )
        ,
        CardActionTrigger::new(
            ActionTriggerType::OtherCardPlayed(TriggerTarget::Owner),
            CardRequiredTarget::None,
            Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                |game: Arc<Mutex<Game>>, source: Arc<Mutex<Card>>, card_played: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = ()> + Send>> {
                    Box::pin(async move {
                        let card = card_played.lock().await;
                        let owner_arc = card.owner.clone().unwrap();

                        if card.creature_type == Some(CreatureType::Angel) {
                            let mut owner = owner_arc.lock().await;
                            println!("{} is an angel!", card.name);
                            let toughness = card.get_stat_value(StatType::Toughness);
                            let id = format!("{}-{}", card.name, Ulid::new().to_string());

                            println!("Adding health {}", toughness);

                            owner.stat_manager.add_stat(id, Stat::new(StatType::Health, toughness));
                        }
                    })
                }
            )))
        )

    )
}

mod test {
    use std::sync::Arc;

    use tokio::sync::{Mutex, RwLock};

    use crate::game::{
        decks::{
            duplicate_card,
            white::{create_angelic_accord, create_righteous_valkyrie},
            Deck,
        },
        effects::EffectTarget,
        mana,
        player::Player,
        Game,
    };

    #[tokio::test]
    async fn test_angel_1() {
        // DeclareAttackerAction
        // TurnPhase
        // DeclareBlockerAction
        // Stat
        let mut game = Game::new();
        let player = game
            .add_player(Player::new(
                "test",
                0,
                duplicate_card(create_righteous_valkyrie(), 4),
            ))
            .await;

        {
            player.lock().await.draw_card();
            player.lock().await.draw_card();
            player.lock().await.draw_card();
            player.lock().await.draw_card();
        }
        game.start_turn(0).await;

        let ga = Arc::new(Mutex::new(game));

        println!("\n\n\n\nplaying creature");
        let a = ga
            .lock()
            .await
            .play_card(&player, 0, None)
            .await
            .expect("oh");
        Game::process_action_queue(ga.clone(), a.clone()).await;
        ga.lock().await.print().await;

        println!("\n\n\n\nplaying creature");
        let b = ga
            .lock()
            .await
            .play_card(&player, 0, None)
            .await
            .expect("oh");
        Game::process_action_queue(ga.clone(), b.clone()).await;
        ga.lock().await.print().await;

        println!("\n\n\n\nplaying creature");
        let c = ga
            .lock()
            .await
            .play_card(&player, 0, None)
            .await
            .expect("oh");
        Game::process_action_queue(ga.clone(), c.clone()).await;
        ga.lock().await.print().await;

        println!("\n\n\n\nplaying creature");
        let d = ga
            .lock()
            .await
            .play_card(&player, 0, None)
            .await
            .expect("oh");
        Game::process_action_queue(ga.clone(), b.clone()).await;
        ga.lock().await.print().await;

        println!("\n\n\n\ndestroying creature");
        let d = ga.lock().await.destroy_card(&d).await;
        Game::process_action_queue(ga.clone(), b.clone()).await;
        ga.lock().await.advance_turn().await;
        ga.lock().await.print().await;
    }
    #[tokio::test]

    async fn test_angel_2() {
        // DeclareAttackerAction
        // TurnPhase
        // DeclareBlockerAction
        // Stat
        let mut game = Game::new();
        let player = game
            .add_player(Player::new(
                "test",
                0,
                vec![
                    create_angelic_accord(),
                    create_righteous_valkyrie(),
                    create_righteous_valkyrie(),
                ],
            ))
            .await;

        {
            player.lock().await.draw_card();
            player.lock().await.draw_card();
            player.lock().await.draw_card();
        }
        game.start_turn(0).await;

        let ga = Arc::new(Mutex::new(game));

        println!("\n\n\n\nplaying creature");
        let a = ga
            .lock()
            .await
            .play_card(&player, 0, None)
            .await
            .expect("oh");
        Game::process_action_queue(ga.clone(), a.clone()).await;
        ga.lock().await.print().await;

        println!("\n\n\n\nplaying creature");
        let b = ga
            .lock()
            .await
            .play_card(&player, 0, None)
            .await
            .expect("oh");
        Game::process_action_queue(ga.clone(), b.clone()).await;
        ga.lock().await.print().await;

        println!("\n\n\n\nplaying enchantment");
        let b = ga
            .lock()
            .await
            .play_card(&player, 0, None)
            .await
            .expect("oh");
        Game::process_action_queue(ga.clone(), b.clone()).await;
        ga.lock().await.print().await;

        // println!("\n\n\n\nplaying creature");
        // let c = ga
        //     .lock()
        //     .await
        //     .play_card(&player, 0, None)
        //     .await
        //     .expect("oh");
        // Game::process_action_queue(ga.clone(), c.clone()).await;
        // ga.lock().await.print().await;

        // println!("\n\n\n\nplaying creature");
        // let d = ga
        //     .lock()
        //     .await
        //     .play_card(&player, 0, None)
        //     .await
        //     .expect("oh");
        // Game::process_action_queue(ga.clone(), b.clone()).await;
        // ga.lock().await.print().await;

        for _ in 0..11 {
            ga.lock().await.advance_turn().await;
        }
        ga.lock().await.print().await;
    }
}
