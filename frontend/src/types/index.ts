export interface City {
  id: string
  country: string
  region: string
  department: string
  postalCode: string
  name: string
  date: string
  streetCount: number
  totalMeters: number
  dir: string
}

export interface CityEditionSummary {
  id: string
  current: boolean
  country: string
  region: string
  department: string
  postalCode: string
  name: string
  date: string
  streetCount: number
  totalMeters: number
}

export interface PlayerResult {
  playerId: string
  playerName: string
  country: string
  cityId: string
  distance: number
  completed: boolean
  rank: number
  time: number
}

export interface Player {
  id: string
  name: string
  country: string
  totalDistance: number
  citiesCompleted: number
  achievements: Achievement[]
  results: PlayerResult[]
}

export interface Achievement {
  type: 'first_city' | 'all_streets' | 'fastest'
  cityId: string
  date: string
}

export interface LeaderboardEntry {
  rank: number
  playerId: string
  playerName: string
  country: string
  citiesCompleted: number
  totalDistance: number
}

export interface PathData {
  id: string
  cityId: string
  coordinates: [number, number][]
  totalDistance: number
  streetCount: number
}

export interface StatsData {
  cityId: string
  editionId: string
  date: string
  streetCount: number
  totalMeters: number
  totalDistance: number
  totalAttempts: number
  totalCompleted: number
  leaderboard: PlayerResult[]
}
