import {
  City,
  CityEditionSummary,
  LeaderboardEntry,
  PathData,
  Player,
  StatsData,
} from '@/types'

const DATA_BASE = `${import.meta.env.BASE_URL}data`

async function fetchJson<T>(path: string): Promise<T> {
  const response = await fetch(`${DATA_BASE}${path}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch ${path}`)
  }
  return response.json() as Promise<T>
}

export function getCities(): Promise<City[]> {
  return fetchJson<City[]>('/cities.json')
}

export async function getCity(id: string): Promise<City | undefined> {
  const cities = await getCities()
  return cities.find((city) => city.id === id)
}

export function getPlayers(): Promise<Player[]> {
  return fetchJson<Player[]>('/players.json')
}

export async function getPlayer(id: string): Promise<Player | undefined> {
  const players = await getPlayers()
  return players.find((player) => player.id === id)
}

export function getLeaderboard(): Promise<LeaderboardEntry[]> {
  return fetchJson<LeaderboardEntry[]>('/leaderboard.json')
}

export function getCityEditions(dir: string): Promise<CityEditionSummary[]> {
  return fetchJson<CityEditionSummary[]>(`/${dir}/index.json`)
}

export function getCityPath(dir: string, editionId: string): Promise<PathData> {
  return fetchJson<PathData>(`/${dir}/paths/${editionId}.json`)
}

export function getCityStats(dir: string, editionId: string): Promise<StatsData> {
  return fetchJson<StatsData>(`/${dir}/stats/${editionId}.json`)
}
