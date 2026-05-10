//! Unit AI: ability casting decision logic.

use aa2_data::TargetType;
use crate::buff::active_status;
use crate::unit::Unit;
use crate::vec2::Vec2;

/// Try to find an ability to cast. Returns `(ability_index, target_id, target_pos)` if found.
///
/// Iterates abilities in order; first ready ability with a valid target wins.
/// An ability is ready if off cooldown, unit has enough mana, and unit is not silenced.
pub fn try_find_cast(
    unit: &Unit,
    units: &[Unit],
) -> Option<(usize, Option<u32>, Option<Vec2>)> {
    let status = active_status(&unit.buffs);
    if status.silenced || status.stunned || status.hexed {
        return None;
    }

    for (i, ability) in unit.abilities.iter().enumerate() {
        if matches!(ability.def.targeting, TargetType::Passive) {
            continue;
        }
        if ability.cooldown_remaining > 0.0 {
            continue;
        }
        if unit.mana < ability.def.mana_cost {
            continue;
        }

        let cast_range = ability.def.cast_range;
        match &ability.def.targeting {
            TargetType::SingleEnemy | TargetType::PointAoE => {
                if let Some((id, pos)) = closest_living_enemy(unit, units, cast_range) {
                    let target_pos = Some(pos);
                    let target_id = Some(id);
                    return Some((i, target_id, target_pos));
                }
            }
            TargetType::SingleAlly => {
                if let Some((id, pos)) = closest_living_ally(unit, units, cast_range) {
                    return Some((i, Some(id), Some(pos)));
                }
            }
            TargetType::NoTarget => {
                return Some((i, None, None));
            }
            TargetType::Passive => unreachable!(),
        }
    }
    None
}

/// Find the closest living enemy within range.
fn closest_living_enemy(unit: &Unit, units: &[Unit], range: f32) -> Option<(u32, Vec2)> {
    units
        .iter()
        .filter(|u| u.team != unit.team && u.is_alive())
        .filter_map(|u| {
            let d = unit.position.distance(u.position);
            (d <= range).then_some((d, u.id, u.position))
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .map(|(_, id, pos)| (id, pos))
}

/// Find the closest living ally (not self) within range.
fn closest_living_ally(unit: &Unit, units: &[Unit], range: f32) -> Option<(u32, Vec2)> {
    units
        .iter()
        .filter(|u| u.team == unit.team && u.is_alive() && u.id != unit.id)
        .filter_map(|u| {
            let d = unit.position.distance(u.position);
            (d <= range).then_some((d, u.id, u.position))
        })
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
        .map(|(_, id, pos)| (id, pos))
}
