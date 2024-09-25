# Current output

```
advancing card phases for Player 1
Player 1's turn: ------
┌────────────────────────────┐ H   P
│          Player 1          │ H   P
├────────────────────────────┤ H   P
│ Alive: true                │ H   P
├────────────────────────────┤ H   P
│ Health: 100                │ H   P
│ Action Points: 5           │ H   P
│ Mana:                      │ H   P
│                            │ H   P
└────────────────────────────┘ H   P

Player 1 draws a card.
Resolving combat damage.
advancing card phases for Player 2
Player 2's turn: ------
┌────────────────────────────┐ H   P
│          Player 2          │ H   P
├────────────────────────────┤ H   P
│ Alive: true                │ H   P
├────────────────────────────┤ H   P
│ Health: 100                │ H   P
│ Action Points: 5           │ H   P
│ Mana:                      │ H   P
│                            │ H   P
└────────────────────────────┘ H   P

Player 2 draws a card.
Resolving combat damage.
advancing card phases for Player 1
Player 1's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐ P
│          Player 1          │ H  │           Swamp            │ P
├────────────────────────────┤ H  ├────────────────────────────┤ P
│ Alive: true                │ H  │                        0/0 │ P
├────────────────────────────┤ H  ├────────────────────────────┤ P
│ Health: 100                │ H  │ TAP: Adds 1 Swamp mana to  │ P
│ Action Points: 5           │ H  │ your pool                  │ P
│ Mana:                      │ H  │                            │ P
│                            │ H  └────────────────────────────┘ P
└────────────────────────────┘ H                                 P

Player 1 draws a card.
Player Player 1 paid  mana for Swamp
Resolving combat damage.
advancing card phases for Player 2
Player 2's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐ P
│          Player 2          │ H  │           Island           │ P
├────────────────────────────┤ H  ├────────────────────────────┤ P
│ Alive: true                │ H  │                        0/0 │ P
├────────────────────────────┤ H  ├────────────────────────────┤ P
│ Action Points: 5           │ H  │ TAP: Adds 1 blue mana to   │ P
│ Health: 100                │ H  │ your pool                  │ P
│ Mana:                      │ H  │                            │ P
│                            │ H  └────────────────────────────┘ P
└────────────────────────────┘ H                                 P

Player 2 draws a card.
Resolving combat damage.
advancing card phases for Player 1
Player 1's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐ P  ┌────────────────────────────┐
│          Player 1          │ H  │           Poison           │ P  │           Swamp            │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤
│ Alive: true                │ H  │ {B}                    2/0 │ P  │                        0/0 │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤
│ Action Points: 5           │ H  │ Applies 2 damage for the   │ P  │ TAP: Adds 1 Swamp mana to  │
│ Health: 100                │ H  │ next 3 turns after 1 turn. │ P  │ your pool                  │
│ Mana:                      │ H  │                            │ P  │                            │
│                            │ H  └────────────────────────────┘ P  └────────────────────────────┘
└────────────────────────────┘ H                                 P

Player 1 draws a card.
Resolving combat damage.
Swamp was tapped
┌────────────────────────────┐ H  ┌────────────────────────────┐  ┌────────────────────────────┐ P  ┌────────────────────────────┐
│          Player 1          │ H  │           Poison           │  │            Buff            │ P  │           Swamp            │
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤ P  ├────────────────────────────┤
│ Alive: true                │ H  │ {B}                    2/0 │  │ {B}                    0/0 │ P  │                        0/0 │
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤ P  ├────────────────────────────┤
│ Health: 100                │ H  │ Applies 2 damage for the   │  │ Adds 1 damage stat to      │ P  │ TAP: Adds 1 Swamp mana to  │
│ Action Points: 5           │ H  │ next 3 turns after 1 turn. │  │ a card                     │ P  │ your pool                  │
│ Mana: {B}                  │ H  │                            │  │                            │ P  │                            │
│                            │ H  └────────────────────────────┘  └────────────────────────────┘ P  └────────────────────────────┘
└────────────────────────────┘ H                                                                 P

Player Player 1 paid {B} mana for Poison
advancing card phases for Player 2
Player 2's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐  ┌────────────────────────────┐ P
│          Player 2          │ H  │           Island           │  │          Destroy           │ P
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤ P
│ Alive: true                │ H  │                        0/0 │  │ {U}                    1/0 │ P
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤ P
│ Action Points: 5           │ H  │ TAP: Adds 1 blue mana to   │  │ Destroy target card        │ P
│ Health: 100                │ H  │ your pool                  │  │                            │ P
│ Mana:                      │ H  │                            │  │                            │ P
│                            │ H  └────────────────────────────┘  └────────────────────────────┘ P
└────────────────────────────┘ H                                                                 P

Player 2 draws a card.
Resolving combat damage.
advancing card phases for Player 1
Card 'Poison' is now Ready.
Player 1's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐ P  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 1          │ H  │            Buff            │ P  │           Swamp            │  │           Poison           │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H  │ {B}                    0/0 │ P  │                        0/0 │  │ {B}                    2/0 │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤
│ Action Points: 5           │ H  │ Adds 1 damage stat to      │ P  │ TAP: Adds 1 Swamp mana to  │  │ Applies 2 damage for the   │
│ Health: 100                │ H  │ a card                     │ P  │ your pool                  │  │ next 3 turns after 1 turn. │
│ Mana:                      │ H  │                            │ P  │                            │  │                            │
│                            │ H  └────────────────────────────┘ P  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H                                 P

Do damage 2 to Player { name: "Player 2", is_alive: true, cards_in_hand: <3 cards> }
Player 1 draws a card.
Swamp was tapped
Player Player 1 paid {B} mana for Buff
Resolving combat damage.
advancing card phases for Player 2
Player 2's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐  ┌────────────────────────────┐  ┌────────────────────────────┐ P
│          Player 2          │ H  │           Island           │  │          Destroy           │  │           Plains           │ P
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤ P
│ Alive: true                │ H  │                        0/0 │  │ {U}                    1/0 │  │                        0/0 │ P
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤ P
│ Action Points: 5           │ H  │ TAP: Adds 1 blue mana to   │  │ Destroy target card        │  │ TAP: Adds 1 white mana to  │ P
│ Health: 98                 │ H  │ your pool                  │  │                            │  │ your pool                  │ P
│ Mana:                      │ H  │                            │  │                            │  │                            │ P
│                            │ H  └────────────────────────────┘  └────────────────────────────┘  └────────────────────────────┘ P
└────────────────────────────┘ H                                                                                                 P

Player 2 draws a card.
Resolving combat damage.
advancing card phases for Player 1
Player 1's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐ P  ┌────────────────────────────┐  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 1          │ H  │           A wall           │ P  │           Swamp            │  │           Poison           │  │            Buff            │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H  │ {B}                    1/7 │ P  │                        0/0 │  │ {B}                    3/0 │  │ {B}                    0/0 │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Action Points: 5           │ H  │ A brick wall               │ P  │ TAP: Adds 1 Swamp mana to  │  │ Applies 2 damage for the   │  │ Adds 1 damage stat to      │
│ Health: 100                │ H  │                            │ P  │ your pool                  │  │ next 3 turns after 1 turn. │  │ a card                     │
│ Mana:                      │ H  │                            │ P  │                            │  │                            │  │                            │
│                            │ H  └────────────────────────────┘ P  └────────────────────────────┘  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H                                 P

Do damage 3 to Player { name: "Player 2", is_alive: true, cards_in_hand: <4 cards> }
Player 1 draws a card.
Resolving combat damage.
Player Player 2 paid  mana for Island
Island was tapped
Player Player 2 paid {U} mana for Destroy
Detaching card at index 2 from player Player 1
Detaching card at index 1 from player Player 2
advancing card phases for Player 2
Player 2's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐  ┌────────────────────────────┐ P  ┌────────────────────────────┐
│          Player 2          │ H  │           Plains           │  │          Stabber           │ P  │           Island           │
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤ P  ├────────────────────────────┤
│ Alive: true                │ H  │                        0/0 │  │ {W} {U}                6/1 │ P  │                        0/0 │
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤ P  ├────────────────────────────┤
│ Action Points: 5           │ H  │ TAP: Adds 1 white mana to  │  │ A stabbing creature        │ P  │ TAP: Adds 1 blue mana to   │
│ Health: 95                 │ H  │ your pool                  │  │                            │ P  │ your pool                  │
│ Mana:                      │ H  │                            │  │                            │ P  │                            │
│                            │ H  └────────────────────────────┘  └────────────────────────────┘ P  └────────────────────────────┘
└────────────────────────────┘ H                                                                 P

Player 2 draws a card.
Resolving combat damage.
advancing card phases for Player 1
Player 1's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐ P  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 1          │ H  │           A wall           │ P  │           Swamp            │  │           Poison           │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H  │ {B}                    1/7 │ P  │                        0/0 │  │ {B}                    2/0 │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤
│ Health: 100                │ H  │ A brick wall               │ P  │ TAP: Adds 1 Swamp mana to  │  │ Applies 2 damage for the   │
│ Action Points: 5           │ H  │                            │ P  │ your pool                  │  │ next 3 turns after 1 turn. │
│ Mana:                      │ H  │                            │ P  │                            │  │                            │
│                            │ H  └────────────────────────────┘ P  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H                                 P

Do damage 2 to Player { name: "Player 2", is_alive: true, cards_in_hand: <2 cards> }
Player 1 draws a card.
Resolving combat damage.
advancing card phases for Player 2
Player 2's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐  ┌────────────────────────────┐ P  ┌────────────────────────────┐
│          Player 2          │ H  │           Plains           │  │          Stabber           │ P  │           Island           │
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤ P  ├────────────────────────────┤
│ Alive: true                │ H  │                        0/0 │  │ {W} {U}                6/1 │ P  │                        0/0 │
├────────────────────────────┤ H  ├────────────────────────────┤  ├────────────────────────────┤ P  ├────────────────────────────┤
│ Action Points: 5           │ H  │ TAP: Adds 1 white mana to  │  │ A stabbing creature        │ P  │ TAP: Adds 1 blue mana to   │
│ Health: 93                 │ H  │ your pool                  │  │                            │ P  │ your pool                  │
│ Mana:                      │ H  │                            │  │                            │ P  │                            │
│                            │ H  └────────────────────────────┘  └────────────────────────────┘ P  └────────────────────────────┘
└────────────────────────────┘ H                                                                 P

Player 2 draws a card.
Player Player 2 paid  mana for Plains
Island was tapped
Plains was tapped
Player Player 2 paid {W} {U} mana for Stabber
Resolving combat damage.
advancing card phases for Player 1
Player 1's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐ P  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 1          │ H  │           A wall           │ P  │           Swamp            │  │           Poison           │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H  │ {B}                    1/7 │ P  │                        0/0 │  │ {B}                    2/0 │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤
│ Action Points: 5           │ H  │ A brick wall               │ P  │ TAP: Adds 1 Swamp mana to  │  │ Applies 2 damage for the   │
│ Health: 100                │ H  │                            │ P  │ your pool                  │  │ next 3 turns after 1 turn. │
│ Mana:                      │ H  │                            │ P  │                            │  │                            │
│                            │ H  └────────────────────────────┘ P  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H                                 P

Do damage 2 to Player { name: "Player 2", is_alive: true, cards_in_hand: <0 cards> }
Player 1 draws a card.
Resolving combat damage.
advancing card phases for Player 2
Card 'Stabber' is now Ready.
Player 2's turn: ------
┌────────────────────────────┐ H   P  ┌────────────────────────────┐  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 2          │ H   P  │           Island           │  │           Plains           │  │          Stabber           │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H   P  │                        0/0 │  │                        0/0 │  │ {W} {U}                6/1 │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Health: 91                 │ H   P  │ TAP: Adds 1 blue mana to   │  │ TAP: Adds 1 white mana to  │  │ A stabbing creature        │
│ Action Points: 5           │ H   P  │ your pool                  │  │ your pool                  │  │                            │
│ Mana:                      │ H   P  │                            │  │                            │  │                            │
│                            │ H   P  └────────────────────────────┘  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H   P

Player 2 draws a card.
Declaring attackers....
[CardActionWrapper { action: DeclareAttackerAction }]
Resolving combat damage.
Attacker Stabber deals 6 damage to player Player 1
advancing card phases for Player 1
Player 1's turn: ------
┌────────────────────────────┐ H  ┌────────────────────────────┐ P  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 1          │ H  │           A wall           │ P  │           Swamp            │  │           Poison           │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H  │ {B}                    1/7 │ P  │                        0/0 │  │ {B}                    2/0 │
├────────────────────────────┤ H  ├────────────────────────────┤ P  ├────────────────────────────┤  ├────────────────────────────┤
│ Action Points: 5           │ H  │ A brick wall               │ P  │ TAP: Adds 1 Swamp mana to  │  │ Applies 2 damage for the   │
│ Health: 94                 │ H  │                            │ P  │ your pool                  │  │ next 3 turns after 1 turn. │
│ Mana:                      │ H  │                            │ P  │                            │  │                            │
│                            │ H  └────────────────────────────┘ P  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H                                 P

Do damage 2 to Player { name: "Player 2", is_alive: true, cards_in_hand: <0 cards> }
Player 1 draws a card.
Swamp was tapped
Player Player 1 paid {B} mana for A wall
Resolving combat damage.
advancing card phases for Player 2
Player 2's turn: ------
┌────────────────────────────┐ H   P  ┌────────────────────────────┐  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 2          │ H   P  │           Island           │  │           Plains           │  │          Stabber           │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H   P  │                        0/0 │  │                        0/0 │  │ {W} {U}                6/1 │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Action Points: 5           │ H   P  │ TAP: Adds 1 blue mana to   │  │ TAP: Adds 1 white mana to  │  │ A stabbing creature        │
│ Health: 89                 │ H   P  │ your pool                  │  │ your pool                  │  │                            │
│ Mana:                      │ H   P  │                            │  │                            │  │                            │
│                            │ H   P  └────────────────────────────┘  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H   P

Player 2 draws a card.
[CardActionWrapper { action: DeclareAttackerAction }]
[CardActionWrapper { action: DeclareBlockerAction }]
Resolving combat damage.
Attacker Stabber deals 6 damage to player Player 1
Attacker A wall deals 1 damage to card Stabber
advancing card phases for Player 1
Card 'A wall' is now Ready.
Player 1's turn: ------
┌────────────────────────────┐ H   P  ┌────────────────────────────┐  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 1          │ H   P  │           Swamp            │  │           Poison           │  │           A wall           │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H   P  │                        0/0 │  │ {B}                    2/0 │  │ {B}                    1/7 │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Health: 88                 │ H   P  │ TAP: Adds 1 Swamp mana to  │  │ Applies 2 damage for the   │  │ A brick wall               │
│ Action Points: 5           │ H   P  │ your pool                  │  │ next 3 turns after 1 turn. │  │                            │
│ Mana:                      │ H   P  │                            │  │                            │  │                            │
│                            │ H   P  └────────────────────────────┘  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H   P

Do damage 2 to Player { name: "Player 2", is_alive: true, cards_in_hand: <0 cards> }
Player 1 draws a card.
Resolving combat damage.
advancing card phases for Player 2
Player 2's turn: ------
┌────────────────────────────┐ H   P  ┌────────────────────────────┐  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 2          │ H   P  │           Island           │  │           Plains           │  │          Stabber           │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H   P  │                        0/0 │  │                        0/0 │  │ {W} {U}                6/0 │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Health: 87                 │ H   P  │ TAP: Adds 1 blue mana to   │  │ TAP: Adds 1 white mana to  │  │ A stabbing creature        │
│ Action Points: 5           │ H   P  │ your pool                  │  │ your pool                  │  │                            │
│ Mana:                      │ H   P  │                            │  │                            │  │                            │
│                            │ H   P  └────────────────────────────┘  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H   P

Player 2 draws a card.
Resolving combat damage.
advancing card phases for Player 1
Player 1's turn: ------
┌────────────────────────────┐ H   P  ┌────────────────────────────┐  ┌────────────────────────────┐  ┌────────────────────────────┐
│          Player 1          │ H   P  │           Swamp            │  │           Poison           │  │           A wall           │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Alive: true                │ H   P  │                        0/0 │  │ {B}                    2/0 │  │ {B}                    1/7 │
├────────────────────────────┤ H   P  ├────────────────────────────┤  ├────────────────────────────┤  ├────────────────────────────┤
│ Health: 88                 │ H   P  │ TAP: Adds 1 Swamp mana to  │  │ Applies 2 damage for the   │  │ A brick wall               │
│ Action Points: 5           │ H   P  │ your pool                  │  │ next 3 turns after 1 turn. │  │                            │
│ Mana:                      │ H   P  │                            │  │                            │  │                            │
│                            │ H   P  └────────────────────────────┘  └────────────────────────────┘  └────────────────────────────┘
└────────────────────────────┘ H   P



```
