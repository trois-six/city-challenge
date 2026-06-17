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

interface Achievement {
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

export type TurnInstruction =
  | 'start'
  | 'straight'
  | 'slight_left'
  | 'slight_right'
  | 'turn_left'
  | 'turn_right'
  | 'sharp_left'
  | 'sharp_right'
  | 'uturn'
  | 'arrive'

export interface RouteStep {
  instruction: TurnInstruction
  streetName: string
  distanceM: number
  coordinate: [number, number]
  geometry: [number, number][]
}

export interface PathData {
  id: string
  cityId: string
  /** Full optimised route, may repeat streets. Used for start/end pins and distance. */
  coordinates: [number, number][]
  /** Each unique street geometry once, for map rendering. */
  streetGeometries: [number, number][][]
  /** Turn-by-turn navigation steps. */
  routeSteps: RouteStep[]
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
