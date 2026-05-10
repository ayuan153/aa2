//! AoE (Area of Effect) hit detection for combat abilities.

use aa2_data::AoeShape;
use crate::unit::Unit;
use crate::vec2::Vec2;

/// Returns indices of units hit by an AoE shape.
/// Excludes the caster. Filters by team based on `hit_enemies` flag.
#[allow(clippy::too_many_arguments)]
pub fn find_aoe_targets(
    shape: &AoeShape,
    origin: Vec2,
    direction: Vec2,
    units: &[Unit],
    caster_id: u32,
    caster_team: u8,
    hit_enemies: bool,
) -> Vec<usize> {
    units
        .iter()
        .enumerate()
        .filter(|(_, u)| {
            u.id != caster_id
                && u.is_alive()
                && if hit_enemies { u.team != caster_team } else { u.team == caster_team }
                && is_in_shape(shape, origin, direction, u.position)
        })
        .map(|(i, _)| i)
        .collect()
}

/// Check if a point is within the given AoE shape.
fn is_in_shape(shape: &AoeShape, origin: Vec2, direction: Vec2, point: Vec2) -> bool {
    match shape {
        AoeShape::Circle { radius } => origin.distance(point) <= *radius,
        AoeShape::Cone { angle, range } => {
            let dist = origin.distance(point);
            if dist > *range {
                return false;
            }
            let to_point = point - origin;
            let angle_between = direction.angle() - to_point.angle();
            // Normalize to [-PI, PI]
            let mut diff = angle_between;
            while diff > std::f32::consts::PI {
                diff -= 2.0 * std::f32::consts::PI;
            }
            while diff < -std::f32::consts::PI {
                diff += 2.0 * std::f32::consts::PI;
            }
            diff.abs() <= angle / 2.0
        }
        AoeShape::Line { width, length } => {
            let offset = point - origin;
            let forward = direction.normalize();
            let right = Vec2::new(-forward.y, forward.x);
            let along = offset.dot(forward);
            let across = offset.dot(right);
            along >= 0.0 && along <= *length && across.abs() <= width / 2.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa2_data::{Attribute, HeroDef};
    use crate::unit::Unit;

    fn make_hero() -> HeroDef {
        HeroDef {
            name: "Test".to_string(),
            primary_attribute: Attribute::Strength,
            base_str: 20.0,
            base_agi: 20.0,
            base_int: 20.0,
            str_gain: 2.0,
            agi_gain: 2.0,
            int_gain: 2.0,
            base_attack_time: 1.7,
            attack_range: 150.0,
            attack_point: 0.5,
            move_speed: 300.0,
            turn_rate: 0.6,
            collision_radius: 24.0,
            tier: 1,
            is_melee: true,
            base_damage_min: 30.0,
            base_damage_max: 30.0,
            projectile_speed: None,
        }
    }

    fn make_unit(id: u32, team: u8, pos: Vec2) -> Unit {
        Unit::from_hero_def(&make_hero(), id, team, pos)
    }

    #[test]
    fn test_circle_aoe() {
        let units = vec![
            make_unit(0, 0, Vec2::new(0.0, 0.0)),   // caster
            make_unit(1, 1, Vec2::new(50.0, 0.0)),   // in range
            make_unit(2, 1, Vec2::new(100.0, 0.0)),  // at edge
            make_unit(3, 1, Vec2::new(150.0, 0.0)),  // out of range
        ];
        let shape = AoeShape::Circle { radius: 100.0 };
        let hits = find_aoe_targets(&shape, Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), &units, 0, 0, true);
        assert_eq!(hits, vec![1, 2]);
    }

    #[test]
    fn test_cone_aoe() {
        let units = vec![
            make_unit(0, 0, Vec2::new(0.0, 0.0)),    // caster
            make_unit(1, 1, Vec2::new(50.0, 0.0)),    // directly ahead
            make_unit(2, 1, Vec2::new(50.0, 10.0)),   // slightly off-axis (within 90° cone)
            make_unit(3, 1, Vec2::new(0.0, 50.0)),    // 90° off (outside 90° cone)
            make_unit(4, 1, Vec2::new(-50.0, 0.0)),   // behind
        ];
        // 90° cone (PI/2 radians full angle), range 100
        let shape = AoeShape::Cone { angle: std::f32::consts::FRAC_PI_2, range: 100.0 };
        let hits = find_aoe_targets(&shape, Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), &units, 0, 0, true);
        assert!(hits.contains(&1));
        assert!(hits.contains(&2));
        assert!(!hits.contains(&3)); // exactly at half-angle boundary or beyond
        assert!(!hits.contains(&4));
    }

    #[test]
    fn test_line_aoe() {
        let units = vec![
            make_unit(0, 0, Vec2::new(0.0, 0.0)),    // caster
            make_unit(1, 1, Vec2::new(50.0, 0.0)),    // on the line
            make_unit(2, 1, Vec2::new(50.0, 20.0)),   // beside line (width 30 -> half=15, 20 > 15)
            make_unit(3, 1, Vec2::new(50.0, 10.0)),   // within width
            make_unit(4, 1, Vec2::new(150.0, 0.0)),   // beyond length
            make_unit(5, 1, Vec2::new(-10.0, 0.0)),   // behind origin
        ];
        let shape = AoeShape::Line { width: 30.0, length: 100.0 };
        let hits = find_aoe_targets(&shape, Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), &units, 0, 0, true);
        assert!(hits.contains(&1));
        assert!(!hits.contains(&2));
        assert!(hits.contains(&3));
        assert!(!hits.contains(&4));
        assert!(!hits.contains(&5));
    }

    #[test]
    fn test_aoe_team_filter() {
        let units = vec![
            make_unit(0, 0, Vec2::new(0.0, 0.0)),   // caster (team 0)
            make_unit(1, 0, Vec2::new(50.0, 0.0)),   // ally
            make_unit(2, 1, Vec2::new(50.0, 0.0)),   // enemy
        ];
        let shape = AoeShape::Circle { radius: 100.0 };

        // hit_enemies = true -> only enemies
        let hits = find_aoe_targets(&shape, Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), &units, 0, 0, true);
        assert_eq!(hits, vec![2]);

        // hit_enemies = false -> only allies (excluding caster)
        let hits = find_aoe_targets(&shape, Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), &units, 0, 0, false);
        assert_eq!(hits, vec![1]);
    }
}
