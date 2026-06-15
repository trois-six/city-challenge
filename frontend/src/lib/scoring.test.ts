import { describe, it, expect } from 'vitest'
import {
  calculatePointsFromCities,
  calculatePointsFromDistance,
  calculateTotalPoints,
  calculateAchievementBonusPoints,
  rankEntries,
  isAllStreetsCompleted,
  calculateCompletionPercentage,
  formatDuration,
  sortLeaderboard,
} from './scoring'

describe('Scoring System', () => {
  describe('calculatePointsFromCities', () => {
    it('should calculate points for single city', () => {
      expect(calculatePointsFromCities(1, 100)).toBe(100)
    })

    it('should calculate points for multiple cities', () => {
      expect(calculatePointsFromCities(5, 100)).toBe(500)
    })

    it('should use custom base points', () => {
      expect(calculatePointsFromCities(3, 150)).toBe(450)
    })

    it('should return 0 for no cities', () => {
      expect(calculatePointsFromCities(0, 100)).toBe(0)
    })
  })

  describe('calculatePointsFromDistance', () => {
    it('should calculate points for distance', () => {
      expect(calculatePointsFromDistance(10, 1)).toBe(10)
    })

    it('should use custom points per km', () => {
      expect(calculatePointsFromDistance(10, 2)).toBe(20)
    })

    it('should floor the result', () => {
      expect(calculatePointsFromDistance(10.7, 1)).toBe(10)
    })

    it('should return 0 for zero distance', () => {
      expect(calculatePointsFromDistance(0, 1)).toBe(0)
    })
  })

  describe('calculateTotalPoints', () => {
    it('should combine city and distance points', () => {
      expect(calculateTotalPoints(5, 50, 100, 1)).toBe(550)
    })

    it('should use default multipliers', () => {
      expect(calculateTotalPoints(2, 20)).toBe(220)
    })

    it('should handle zero values', () => {
      expect(calculateTotalPoints(0, 0, 100, 1)).toBe(0)
    })

    it('should handle high values', () => {
      expect(calculateTotalPoints(100, 1000, 100, 1)).toBe(11000)
    })
  })

  describe('calculateAchievementBonusPoints', () => {
    it('should award points for first city', () => {
      expect(
        calculateAchievementBonusPoints([{ type: 'first_city' }])
      ).toBe(50)
    })

    it('should award points for all streets', () => {
      expect(
        calculateAchievementBonusPoints([{ type: 'all_streets' }])
      ).toBe(75)
    })

    it('should award points for fastest', () => {
      expect(calculateAchievementBonusPoints([{ type: 'fastest' }])).toBe(100)
    })

    it('should award cumulative points for multiple achievements', () => {
      expect(
        calculateAchievementBonusPoints([
          { type: 'first_city' },
          { type: 'all_streets' },
          { type: 'fastest' },
        ])
      ).toBe(225)
    })

    it('should return 0 for no achievements', () => {
      expect(calculateAchievementBonusPoints([])).toBe(0)
    })
  })

  describe('rankEntries', () => {
    it('should rank entries by points', () => {
      const entries = [
        { totalPoints: 100 },
        { totalPoints: 200 },
        { totalPoints: 150 },
      ]
      const ranked = rankEntries(entries)

      expect(ranked[0].rank).toBe(1)
      expect(ranked[0].entry.totalPoints).toBe(200)
      expect(ranked[1].rank).toBe(2)
      expect(ranked[1].entry.totalPoints).toBe(150)
      expect(ranked[2].rank).toBe(3)
      expect(ranked[2].entry.totalPoints).toBe(100)
    })

    it('should handle ties', () => {
      const entries = [
        { totalPoints: 100 },
        { totalPoints: 100 },
        { totalPoints: 50 },
      ]
      const ranked = rankEntries(entries)

      expect(ranked[0].rank).toBe(1)
      expect(ranked[1].rank).toBe(2)
      expect(ranked[2].rank).toBe(3)
    })

    it('should handle single entry', () => {
      const entries = [{ totalPoints: 100 }]
      const ranked = rankEntries(entries)

      expect(ranked).toHaveLength(1)
      expect(ranked[0].rank).toBe(1)
    })

    it('should handle empty list', () => {
      const entries: { totalPoints: number }[] = []
      const ranked = rankEntries(entries)

      expect(ranked).toHaveLength(0)
    })
  })

  describe('isAllStreetsCompleted', () => {
    it('should return true when all streets completed', () => {
      expect(isAllStreetsCompleted(100, 100, 0.95)).toBe(true)
    })

    it('should return true when more than threshold', () => {
      expect(isAllStreetsCompleted(96, 100, 0.95)).toBe(true)
    })

    it('should return false when below threshold', () => {
      expect(isAllStreetsCompleted(94, 100, 0.95)).toBe(false)
    })

    it('should use custom threshold', () => {
      expect(isAllStreetsCompleted(80, 100, 0.8)).toBe(true)
      expect(isAllStreetsCompleted(79, 100, 0.8)).toBe(false)
    })

    it('should handle zero expected distance', () => {
      expect(isAllStreetsCompleted(0, 0, 0.95)).toBe(true)
    })
  })

  describe('calculateCompletionPercentage', () => {
    it('should calculate completion percentage', () => {
      expect(calculateCompletionPercentage(50, 100)).toBe(50)
    })

    it('should cap at 100 percent', () => {
      expect(calculateCompletionPercentage(150, 100)).toBe(100)
    })

    it('should return 0 for zero expected distance', () => {
      expect(calculateCompletionPercentage(10, 0)).toBe(0)
    })

    it('should handle partial completion', () => {
      expect(calculateCompletionPercentage(33.33, 100)).toBeCloseTo(33.33)
    })
  })

  describe('formatDuration', () => {
    it('should format seconds only', () => {
      expect(formatDuration(45)).toBe('45s')
    })

    it('should format minutes and seconds', () => {
      expect(formatDuration(125)).toBe('2m 5s')
    })

    it('should format hours, minutes and seconds', () => {
      expect(formatDuration(3665)).toBe('1h 1m 5s')
    })

    it('should format only hours', () => {
      expect(formatDuration(3600)).toBe('1h')
    })

    it('should handle zero', () => {
      expect(formatDuration(0)).toBe('0s')
    })

    it('should handle large values', () => {
      expect(formatDuration(86400)).toBe('24h')
    })
  })

  describe('sortLeaderboard', () => {
    it('should sort by points by default', () => {
      const entries = [
        { totalPoints: 100 },
        { totalPoints: 300 },
        { totalPoints: 200 },
      ]
      const sorted = sortLeaderboard(entries)

      expect(sorted[0].totalPoints).toBe(300)
      expect(sorted[1].totalPoints).toBe(200)
      expect(sorted[2].totalPoints).toBe(100)
    })

    it('should sort by distance when specified', () => {
      const entries = [
        { totalDistance: 10 },
        { totalDistance: 30 },
        { totalDistance: 20 },
      ]
      const sorted = sortLeaderboard(entries, 'distance')

      expect(sorted[0].totalDistance).toBe(30)
      expect(sorted[1].totalDistance).toBe(20)
      expect(sorted[2].totalDistance).toBe(10)
    })

    it('should not modify original array', () => {
      const original = [
        { totalPoints: 100 },
        { totalPoints: 200 },
      ]
      const sorted = sortLeaderboard(original)

      expect(original[0].totalPoints).toBe(100)
      expect(sorted[0].totalPoints).toBe(200)
    })
  })
})
