/**
 * Calculate points based on completed cities
 */
export function calculatePointsFromCities(
  citiesCompleted: number,
  basePointsPerCity: number = 100
): number {
  return citiesCompleted * basePointsPerCity
}

/**
 * Calculate points based on distance covered
 */
export function calculatePointsFromDistance(
  distanceKm: number,
  pointsPerKm: number = 1
): number {
  return Math.floor(distanceKm * pointsPerKm)
}

/**
 * Calculate total leaderboard points
 */
export function calculateTotalPoints(
  citiesCompleted: number,
  totalDistance: number,
  basePointsPerCity: number = 100,
  pointsPerKm: number = 1
): number {
  const cityPoints = calculatePointsFromCities(citiesCompleted, basePointsPerCity)
  const distancePoints = calculatePointsFromDistance(totalDistance, pointsPerKm)
  return cityPoints + distancePoints
}

/**
 * Calculate achievement bonus points
 */
export function calculateAchievementBonusPoints(
  achievements: Array<{
    type: 'first_city' | 'all_streets' | 'fastest'
  }>
): number {
  const bonusPoints: Record<string, number> = {
    first_city: 50,
    all_streets: 75,
    fastest: 100,
  }

  return achievements.reduce(
    (total, achievement) => total + (bonusPoints[achievement.type] || 0),
    0
  )
}

/**
 * Rank entries by total points
 */
export function rankEntries(entries: Array<{ totalPoints: number }>): Array<{
  entry: typeof entries[0]
  rank: number
}> {
  return entries
    .sort((a, b) => b.totalPoints - a.totalPoints)
    .map((entry, index) => ({
      entry,
      rank: index + 1,
    }))
}

/**
 * Check if player completed all streets in a city
 */
export function isAllStreetsCompleted(
  totalDistance: number,
  expectedTotalDistance: number,
  completionThreshold: number = 0.95
): boolean {
  return totalDistance >= expectedTotalDistance * completionThreshold
}

/**
 * Calculate completion percentage
 */
export function calculateCompletionPercentage(
  totalDistance: number,
  expectedTotalDistance: number
): number {
  if (expectedTotalDistance === 0) return 0
  return Math.min(
    100,
    (totalDistance / expectedTotalDistance) * 100
  )
}

/**
 * Format time duration in seconds to readable string
 */
export function formatDuration(seconds: number): string {
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const secs = Math.floor(seconds % 60)

  const parts: string[] = []
  if (hours > 0) parts.push(`${hours}h`)
  if (minutes > 0) parts.push(`${minutes}m`)
  if (secs > 0 || parts.length === 0) parts.push(`${secs}s`)

  return parts.join(' ')
}

/**
 * Sort leaderboard entries
 */
export function sortLeaderboard<
  T extends { totalPoints?: number; totalDistance?: number }
>(entries: T[], sortBy: 'points' | 'distance' = 'points'): T[] {
  const sorted = [...entries]
  if (sortBy === 'points') {
    sorted.sort((a, b) => (b.totalPoints || 0) - (a.totalPoints || 0))
  } else {
    sorted.sort((a, b) => (b.totalDistance || 0) - (a.totalDistance || 0))
  }
  return sorted
}
