//! Drives one game from plate appearance to plate appearance.
//!
//! The state machine applies the result. The store keeps that state. The sim keeps nothing.

use crate::game::{Game, GameError};
use crate::governance::Governance;
use crate::manager::ComputerManager;
use crate::sim::Sim;
use crate::store::MemoryStore;

pub fn play_game(
    game: &mut Game,
    sim: &mut Sim,
    manager: &mut ComputerManager,
    store: &mut MemoryStore,
    governance: &Governance,
) -> Result<(), GameError> {
    let mut guard = 0u32;
    while !game.is_final() {
        guard += 1;
        if guard > 2_000 {
            return Err(GameError::TooLong);
        }
        let needs_pitcher = {
            let side = game.fielding_side();
            match game.club(side).pitcher_slot.clone() {
                None => true,
                Some(id) => game.pitcher_is_spent(side, &id),
            }
        };
        if needs_pitcher {
            manager.assign_next(game)?;
        }
        let calls = manager.pre_pa_calls();
        let input = game.plate_appearance_input(calls)?;
        let result = governance.review(sim.resolve(&input));
        game.apply_plate_appearance(result)?;
        store.save(game)?;
    }
    Ok(())
}
