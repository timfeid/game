use crate::game::{
    action::{
        generate_mana::GenerateManaAction, ActionTriggerType, AsyncClosureAction,
        AsyncClosureWithCardAction, CardActionTrigger, CardRequiredTarget, DeclareAttackerAction,
        DeclareBlockerAction, PlayerActionTarget, TriggerTarget,
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

fn create_fire() -> Card {
    Card::new(
        "Fire",
        "",
        vec![CardActionTrigger::new(
            ActionTriggerType::AbilityWithinPhases(
                "TAP: Adds 1 red mana to your pool.".to_string(),
                vec![],
                None,
                true,
            ),
            CardRequiredTarget::None,
            Arc::new(GenerateManaAction {
                mana_to_add: vec![ManaType::Red],
                target: PlayerActionTarget::Owner,
            }),
        )],
        CardPhase::Ready,
        CardType::BasicLand(ManaType::Red),
        vec![],
        vec![],
    )
}

pub fn create_red_deck() -> Vec<Card> {
    let mut deck: Vec<Card> = vec![];
    deck.append(&mut duplicate_card(create_fire(), 8));

    deck
}

mod test {
    use std::sync::Arc;

    use tokio::sync::{Mutex, RwLock};

    use crate::game::{
        card::Card, decks::red::create_fire, effects::EffectTarget, mana, player::Player, Game,
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
                vec![create_fire(), create_fire(), create_fire(), create_fire()],
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
