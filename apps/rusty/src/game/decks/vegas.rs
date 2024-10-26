use crate::game::{
    action::{
        generate_mana::GenerateManaAction, ActionBuilder, ActionTriggerType, CardRequiredTarget,
        PhaseTarget, PlayerActionTarget,
    },
    card::{Card, CardBuilder, CardType},
    mana::ManaType,
    stat::{StatType, Stats},
    turn::TurnPhase,
    Game,
};

use super::duplicate_card;

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
                "Adds {Bk} to your mana pool.".to_string(),
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
                "Adds {R} to your mana pool.".to_string(),
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

    deck.append(&mut duplicate_card(create_casino_grounds(), 1));

    deck
}
