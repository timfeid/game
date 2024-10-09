use crate::game::{
    action::{
        generate_mana::GenerateManaAction, Action, ActionTriggerType, ApplyEffectToPlayerCardType,
        AsyncClosureAction, AsyncClosureWithCardAction, CardAction, CardActionTarget,
        CardActionTrigger, CardActionWrapper, CardRequiredTarget, CardTargetTeam,
        CastMandatoryAdditionalAbility, CastOptionalAdditionalAbility, DeclareAttackerAction,
        DeclareBlockerAction, DrawCardAction, DrawCardCardAction, PlayerActionTarget,
        TriggerTarget,
    },
    card::{
        card::{create_creature_card, create_multiple_cards},
        Card, CardPhase, CardType, CreatureType,
    },
    decks::duplicate_card,
    effects::{Effect, EffectID, EffectTarget, ExpireContract, LifeLinkAction, StatModifierEffect},
    mana::ManaType,
    player::Player,
    stat::{Stat, StatType, Stats},
    turn::TurnPhase,
    ActionType, Game,
};
use std::{f32::consts::E, future::Future, mem::zeroed, pin::Pin, sync::Arc};

use tokio::sync::Mutex;
use ulid::Ulid;

fn create_forest() -> Card {
    Card::new(
        "Forest",
        "",
        vec![CardActionTrigger::new(
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
        )],
        CardPhase::Ready,
        CardType::BasicLand(ManaType::Green),
        vec![],
        vec![],
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
                 -> Pin<Box<dyn Future<Output = ()> + Send>> {
                    Box::pin(async move {
                        let owner = card.lock().await.owner.clone().unwrap();
                        let cards_in_play = &owner.lock().await.cards_in_play.clone();
                        for card in cards_in_play {
                            if card.lock().await.creature_type == Some(CreatureType::Elf) {
                                owner.lock().await.mana_pool.add_mana(ManaType::Green);
                            }
                        }
                    })
                }
            )))
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
                target: CardRequiredTarget::CreatureOfType(CreatureType::Elf, CardTargetTeam::Owner),
                description: "Return an Elf you control to its owner's hand: Untap target creature. Activate only once each turn.".to_string(),
                ability: Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {

                    Arc::new(AsyncClosureAction::new(Arc::new(
                        |game: Arc<Mutex<Game>>, card: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = ()> + Send>> {
                        Box::pin(async move {
                        let owner_arc = {
                            let card = card.lock().await;
                            card.owner.clone()
                        };

                        if let Some(owner_arc) = owner_arc {
                            {

                            let mut owner = owner_arc.lock().await;
                            // Remove the card from the battlefield and add it to the owner's hand
                            if let Some(index) = owner.cards_in_play.iter().position(|c| Arc::ptr_eq(c, &card)) {
                                let card_arc = owner.cards_in_play.remove(index);
                                owner.cards_in_hand.push(card_arc.clone());
                            }
                        }


                                    game.lock().await.execute_actions(&mut vec![Arc::new(CardActionWrapper {
                                        card: card,
                                        action: Arc::new(CastMandatoryAdditionalAbility {
                                            action_type: ActionType::None,
                                            mana: vec![],
                                            target: CardRequiredTarget::CardOfType(CardType::Creature, CardTargetTeam::Any),
                                            description:
                                                "Untap target creature"
                                                    .to_string(),
                                            ability: Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                                                Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                                                    |game: Arc<Mutex<Game>>,
                                                    source: Arc<Mutex<Card>>,
                                                    card_played: Arc<Mutex<Card>>|
                                                    -> Pin<Box<dyn Future<Output = ()> + Send>> {
                                                        Box::pin(async move {
                                                            let mut card = card_played.lock().await;
                                                            card.tapped = false;
                                                        })
                                                    },
                                                )))
                                                    as Arc<dyn CardAction + Send + Sync>
                                            })
                                        }) ,
                                        target: None
                                    })]).await;
                        }

                    })
                },
            )))})}),
            Arc::new(
                |game: Arc<Mutex<Game>>, card: Arc<Mutex<Card>>| -> Pin<Box<dyn Future<Output = bool> + Send>> {
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
            Arc::new(CastOptionalAdditionalAbility {
                action_type: ActionType::None,
                mana: vec![ManaType::Green],
                target: CardRequiredTarget::None,
                description:
                    "Whenever you cast an Elf spell, you may pay {G}. If you do, draw a card."
                        .to_string(),
                ability: Arc::new(|card| -> Arc<dyn CardAction + Send + Sync> {
                    Arc::new(DrawCardCardAction::one(CardActionTarget::SelfOwner))
                        as Arc<dyn CardAction + Send + Sync>
                })
            })
        ),
        CardActionTrigger::new(
            ActionTriggerType::Continuous,
            CardRequiredTarget::None,
            Arc::new(ApplyEffectToPlayerCardType {
                card_type: CardType::Creature,
                effect_generator: Arc::new(
                    |target, card| -> Vec<Arc<Mutex<dyn Effect + Send + Sync>>> {
                        vec![
                            Arc::new(Mutex::new(StatModifierEffect::new(
                                target.clone(),
                                StatType::Toughness,
                                1,
                                ExpireContract::Never,
                                card.clone(),
                            ))),
                            Arc::new(Mutex::new(StatModifierEffect::new(
                                target,
                                StatType::Power,
                                1,
                                ExpireContract::Never,
                                card,
                            ))),
                        ]
                    }
                )
            })
        )
    )
}

pub fn create_green_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    deck.append(&mut duplicate_card(create_forest(), 4));
    // deck.append(&mut duplicate_card(create_priest_of_titania(), 4));
    // deck.append(&mut duplicate_card(create_leaf_crowned_visionary(), 4));

    deck.append(&mut duplicate_card(create_wirewood(), 4));

    deck
}

mod test {
    use std::sync::Arc;

    use tokio::sync::{Mutex, RwLock};

    use crate::{
        game::{
            card::{Card, CardPhase},
            decks::{
                green::{create_forest, create_leaf_crowned_visionary, create_wirewood},
                Deck,
            },
            effects::EffectTarget,
            mana,
            player::Player,
            turn::TurnPhase,
            CardWithDetails, Game,
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
        Game::process_action_queue(ga.clone(), leaf.clone().unwrap()).await;
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

        ga.lock()
            .await
            .play_card(&player, 0, None)
            .await
            .expect("oh no");
        Game::process_action_queue(ga.clone(), leaf.clone().unwrap()).await;

        let ability_id = ga
            .lock()
            .await
            .abilities
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
}
