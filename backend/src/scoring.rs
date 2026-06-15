//! Pure scoring and ranking functions shared by the data-build pipeline.

use serde::{Deserialize, Serialize};

/// Calculate points based on completed cities.
pub fn calculate_points_from_cities(cities_completed: i64, base_points_per_city: i64) -> i64 {
    cities_completed * base_points_per_city
}

/// Calculate points based on distance covered.
pub fn calculate_points_from_distance(distance_km: f64, points_per_km: f64) -> i64 {
    (distance_km * points_per_km).floor() as i64
}

/// Calculate total leaderboard points.
pub fn calculate_total_points(cities_completed: i64, total_distance: f64) -> i64 {
    calculate_points_from_cities(cities_completed, 100)
        + calculate_points_from_distance(total_distance, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AchievementType {
    FirstCity,
    AllStreets,
    Fastest,
}

/// Calculate achievement bonus points.
pub fn calculate_achievement_bonus_points(achievements: &[AchievementType]) -> i64 {
    achievements
        .iter()
        .map(|achievement| match achievement {
            AchievementType::FirstCity => 50,
            AchievementType::AllStreets => 75,
            AchievementType::Fastest => 100,
        })
        .sum()
}

/// Check if a player completed all streets in a city.
pub fn is_all_streets_completed(total_distance: f64, expected_total_distance: f64) -> bool {
    const COMPLETION_THRESHOLD: f64 = 0.95;
    total_distance >= expected_total_distance * COMPLETION_THRESHOLD
}

/// Trait for entries that can be ranked by total points (descending).
pub trait HasPoints {
    fn total_points(&self) -> i64;
}

/// Rank entries by total points, returning each entry paired with its 1-based rank.
pub fn rank_entries<T: HasPoints>(mut entries: Vec<T>) -> Vec<(T, usize)> {
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.total_points()));
    entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| (entry, index + 1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_points_from_cities() {
        assert_eq!(calculate_points_from_cities(3, 100), 300);
        assert_eq!(calculate_points_from_cities(0, 100), 0);
    }

    #[test]
    fn test_calculate_points_from_distance() {
        assert_eq!(calculate_points_from_distance(12.7, 1.0), 12);
        assert_eq!(calculate_points_from_distance(0.0, 1.0), 0);
    }

    #[test]
    fn test_calculate_total_points() {
        assert_eq!(calculate_total_points(2, 27.99), 227);
        assert_eq!(calculate_total_points(0, 0.0), 0);
    }

    #[test]
    fn test_calculate_achievement_bonus_points() {
        let achievements = vec![
            AchievementType::FirstCity,
            AchievementType::AllStreets,
            AchievementType::Fastest,
        ];
        assert_eq!(calculate_achievement_bonus_points(&achievements), 225);
        assert_eq!(calculate_achievement_bonus_points(&[]), 0);
    }

    #[test]
    fn test_is_all_streets_completed() {
        assert!(is_all_streets_completed(10.0, 10.0));
        assert!(is_all_streets_completed(9.6, 10.0));
        assert!(!is_all_streets_completed(9.0, 10.0));
    }

    struct Entry {
        total_points: i64,
    }

    impl HasPoints for Entry {
        fn total_points(&self) -> i64 {
            self.total_points
        }
    }

    #[test]
    fn test_rank_entries() {
        let entries = vec![
            Entry { total_points: 100 },
            Entry { total_points: 300 },
            Entry { total_points: 200 },
        ];
        let ranked = rank_entries(entries);
        let ranks: Vec<(i64, usize)> = ranked
            .iter()
            .map(|(entry, rank)| (entry.total_points, *rank))
            .collect();
        assert_eq!(ranks, vec![(300, 1), (200, 2), (100, 3)]);
    }
}
