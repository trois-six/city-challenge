import { describe, it, expect, beforeEach, vi } from 'vitest'
import {
  getCities,
  getCity,
  getPlayers,
  getPlayer,
  getLeaderboard,
  getCityEditions,
  getCityPath,
  getCityStats,
} from './data'
import {
  City,
  CityEditionSummary,
  LeaderboardEntry,
  PathData,
  Player,
  PlayerResult,
  StatsData,
} from '@/types'

// Fixtures mirror the camelCase JSON written by `backend/src/bin/build_data.rs`
// (CityManifestEntry, EditionSummary, PathFile, StatsFile, PlayerManifestEntry,
// LeaderboardEntry, PlayerResultData). This is the real contract between the
// data pipeline and the frontend static-data layer.

const cityDir = 'FR/75/75001/Paris'
const editionId = 'FR-75-75001-Paris-2026-06-15'

const cityFixture: City = {
  id: 'FR-75-75001-Paris',
  country: 'FRA',
  region: 'Île-de-France',
  department: '75',
  postalCode: '75001',
  name: 'Paris',
  date: '2026-06-15',
  streetCount: 42,
  totalMeters: 13661,
  dir: cityDir,
}

const editionFixture: CityEditionSummary = {
  id: editionId,
  current: true,
  country: 'FRA',
  region: 'Île-de-France',
  department: '75',
  postalCode: '75001',
  name: 'Paris',
  date: '2026-06-15',
  streetCount: 42,
  totalMeters: 13661,
}

const playerResultFixture: PlayerResult = {
  playerId: 'player-1',
  playerName: 'Alice',
  country: 'FRA',
  cityId: 'FR-75-75001-Paris',
  distance: 27.36,
  completed: true,
  rank: 1,
  time: 9876,
}

const playerFixture: Player = {
  id: 'player-1',
  name: 'Alice',
  country: 'FRA',
  totalDistance: 156.8,
  citiesCompleted: 5,
  achievements: [
    { type: 'first_city', cityId: 'FR-75-75001-Paris', date: '2026-06-15' },
  ],
  results: [playerResultFixture],
}

const leaderboardEntryFixture: LeaderboardEntry = {
  rank: 1,
  playerId: 'player-1',
  playerName: 'Alice',
  country: 'FRA',
  citiesCompleted: 5,
  totalDistance: 156.8,
}

const pathFixture: PathData = {
  id: editionId,
  cityId: 'FR-75-75001-Paris',
  coordinates: [
    [48.8566, 2.3522],
    [48.86, 2.35],
  ],
  streetGeometries: [
    [[48.8566, 2.3522], [48.86, 2.35]],
  ],
  routeSteps: [
    { instruction: 'start', streetName: 'Rue de Rivoli', distanceM: 245, coordinate: [48.8566, 2.3522], geometry: [[48.8566, 2.3522], [48.86, 2.35]] },
    { instruction: 'arrive', streetName: '', distanceM: 0, coordinate: [48.86, 2.35], geometry: [] },
  ],
  totalDistance: 27.356,
  streetCount: 42,
}

const statsFixture: StatsData = {
  cityId: 'FR-75-75001-Paris',
  editionId,
  date: '2026-06-15',
  streetCount: 42,
  totalMeters: 13661,
  totalDistance: 13.661,
  totalAttempts: 1,
  totalCompleted: 1,
  leaderboard: [playerResultFixture],
}

const routes: Record<string, unknown> = {
  '/data/cities.json': [cityFixture],
  '/data/players.json': [playerFixture],
  '/data/leaderboard.json': [leaderboardEntryFixture],
  [`/data/${cityDir}/index.json`]: [editionFixture],
  [`/data/${cityDir}/paths/${editionId}.json`]: pathFixture,
  [`/data/${cityDir}/stats/${editionId}.json`]: statsFixture,
}

beforeEach(() => {
  vi.stubGlobal(
    'fetch',
    vi.fn((url: string) => {
      const path = url.replace(/^https?:\/\/[^/]+/, '')
      if (path in routes) {
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve(routes[path]),
        } as Response)
      }
      return Promise.resolve({ ok: false, status: 404 } as Response)
    }),
  )
})

describe('data layer (frontend/backend JSON contract)', () => {
  it('getCities returns cities matching the City contract', async () => {
    const cities = await getCities()
    expect(cities).toEqual([cityFixture])
  })

  it('getCity finds a city by id', async () => {
    const city = await getCity(cityFixture.id)
    expect(city).toEqual(cityFixture)
  })

  it('getCity returns undefined for an unknown id', async () => {
    const city = await getCity('does-not-exist')
    expect(city).toBeUndefined()
  })

  it('getPlayers returns players matching the Player contract', async () => {
    const players = await getPlayers()
    expect(players).toEqual([playerFixture])
  })

  it('getPlayer finds a player by id', async () => {
    const player = await getPlayer(playerFixture.id)
    expect(player).toEqual(playerFixture)
  })

  it('getLeaderboard returns entries matching the LeaderboardEntry contract', async () => {
    const leaderboard = await getLeaderboard()
    expect(leaderboard).toEqual([leaderboardEntryFixture])
  })

  it('getCityEditions returns the per-city edition index', async () => {
    const editions = await getCityEditions(cityDir)
    expect(editions).toEqual([editionFixture])
  })

  it('getCityPath returns the optimized route for an edition', async () => {
    const path = await getCityPath(cityDir, editionId)
    expect(path).toEqual(pathFixture)
  })

  it('getCityStats returns the stats/leaderboard for an edition', async () => {
    const stats = await getCityStats(cityDir, editionId)
    expect(stats).toEqual(statsFixture)
  })

  it('throws when a static data file is missing', async () => {
    await expect(getCityPath(cityDir, 'unknown-edition')).rejects.toThrow(
      'Failed to fetch /FR/75/75001/Paris/paths/unknown-edition.json',
    )
  })
})
