use crate::game::{
    action::{
        generate_mana::GenerateManaAction, ActionTriggerType, AsyncClosureAction,
        AsyncClosureWithCardAction, CardActionTarget, CardActionTrigger, CardRequiredTarget,
        CardTargetTeam, CounterSpellAction, DeclareAttackerAction, DeclareBlockerAction,
        DrawCardCardAction, PlayerActionTarget, ReturnToHandAction, TriggerTarget,
    },
    card::{
        card::{create_creature_card, create_multiple_cards},
        Card, CardPhase, CardType, CreatureType,
    },
    decks::duplicate_card,
    effects::{
        DynamicStatModifierEffect, Effect, EffectID, EffectTarget, ExpireContract, LifeLinkAction,
    },
    mana::ManaType,
    player::Player,
    stat::{Stat, StatType, Stats},
    turn::TurnPhase,
    Game,
};
use std::{f32::consts::E, future::Future, mem::zeroed, pin::Pin, sync::Arc};

use tokio::sync::Mutex;
use ulid::Ulid;

fn create_frost_breath() -> Card {
    Card::new(
        "Frost Breath",
        "Tap up to two target creatures. They don't untap during their controller's next untap step.",
        vec![CardActionTrigger::new(
            ActionTriggerType::CardPlayedFromHand,
            CardRequiredTarget::MultipleCardsOfType(CardType::Creature, 2),
            Arc::new(AsyncClosureWithCardAction::new(Arc::new(
                |game: Arc<Mutex<Game>>,
                 source: Arc<Mutex<Card>>,
                 card_played: Arc<Mutex<Card>>|
                 -> Pin<Box<dyn Future<Output = ()> + Send>> {
                    Box::pin(async move {
                        let mut card = card_played.lock().await;
                        card.tapped = true;
                    })
                },
            ))),
        )],
        CardPhase::Ready,
        CardType::Instant,
        vec![],
        vec![ManaType::Blue, ManaType::Colorless],
    )
}

fn create_island() -> Card {
    Card::new(
        "Island",
        "",
        vec![CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "Add 1 {B} to your pool".to_string(),
                vec![],
                None,
                true,
            ),
            CardRequiredTarget::None,
            Arc::new(GenerateManaAction {
                mana_to_add: vec![ManaType::Blue],
                target: PlayerActionTarget::Owner,
            }),
        )],
        CardPhase::Ready,
        CardType::BasicLand(ManaType::Blue),
        vec![],
        vec![],
    )
}

pub fn create_counterspell() -> Card {
    Card::new(
        "Counter Spell",
        "Counter target spell.",
        vec![CardActionTrigger::new(
            ActionTriggerType::CardPlayedFromHand,
            CardRequiredTarget::Spell,
            Arc::new(CounterSpellAction {}),
        )],
        CardPhase::Ready,
        CardType::Instant,
        vec![],
        vec![ManaType::Blue, ManaType::Blue],
    )
}

pub fn create_divination() -> Card {
    Card::new(
        "Divination",
        "Draw two cards.",
        vec![CardActionTrigger::new(
            ActionTriggerType::CardPlayedFromHand,
            CardRequiredTarget::None,
            Arc::new(DrawCardCardAction {
                target: CardActionTarget::SelfOwner,
                count: 2,
            }),
        )],
        CardPhase::Ready,
        CardType::Sorcery,
        vec![],
        vec![ManaType::Blue, ManaType::Colorless],
    )
}

pub fn create_unsummon() -> Card {
    Card::new(
        "Unsummon",
        "Return target creature to its owner's hand.",
        vec![CardActionTrigger::new(
            ActionTriggerType::CardPlayedFromHand,
            CardRequiredTarget::CardOfType(CardType::Creature, CardTargetTeam::Any),
            Arc::new(ReturnToHandAction {}),
        )],
        CardPhase::Ready,
        CardType::Instant,
        vec![],
        vec![ManaType::Blue],
    )
}

pub fn create_blue_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    deck.append(&mut duplicate_card(create_counterspell(), 4));
    deck.append(&mut duplicate_card(create_frost_breath(), 4));
    deck.append(&mut duplicate_card(create_unsummon(), 4));
    deck.append(&mut duplicate_card(create_divination(), 4));
    deck.append(&mut duplicate_card(create_island(), 8));

    deck
}

mod test {
    use std::sync::Arc;

    use tokio::sync::{Mutex, RwLock};

    use crate::game::{
        card::Card,
        decks::{
            blue::{create_counterspell, create_island},
            Deck,
        },
        effects::EffectTarget,
        mana,
        player::Player,
        Game,
    };

    #[tokio::test]
    async fn test_green_1() {
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
                    create_island(),
                    create_island(),
                    create_island(),
                    create_counterspell(),
                ],
            ))
            .await;

        player.lock().await.draw_card();
        player.lock().await.draw_card();
        player.lock().await.draw_card();
        let hydra = player.lock().await.draw_card();
        game.start_turn(0).await;

        {
            let clone = Arc::clone(&player);
            let mut player = clone.lock().await;
            let mut cards: Vec<Arc<Mutex<Card>>> = player.cards_in_hand.drain(0..3).collect();

            // Append the drained cards to `cards_in_play`
            player.cards_in_play.append(&mut cards);
        }

        let ga = Arc::new(Mutex::new(game));
        Game::process_action_queue(ga.clone(), hydra.clone().unwrap()).await;

        ga.lock()
            .await
            .activate_card_action_old(&player, 1, None)
            .await
            .expect("oh no?");

        ga.lock().await.print().await;

        // ga.lock().await.advance_turn().await;
    }
}
