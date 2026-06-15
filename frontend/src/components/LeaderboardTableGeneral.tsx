import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { LeaderboardEntry } from '@/types'
import styles from './LeaderboardTableGeneral.module.css'

interface SortConfig {
  key: keyof LeaderboardEntry
  direction: 'asc' | 'desc'
}

export default function LeaderboardTableGeneral({
  data,
}: {
  data: LeaderboardEntry[]
}) {
  const { t } = useTranslation()
  const [sortConfig, setSortConfig] = useState<SortConfig>({
    key: 'rank',
    direction: 'asc',
  })

  const handleSort = (key: keyof LeaderboardEntry) => {
    setSortConfig((prev) => ({
      key,
      direction:
        prev.key === key && prev.direction === 'asc' ? 'desc' : 'asc',
    }))
  }

  const sortedData = [...data].sort((a, b) => {
    const aValue = a[sortConfig.key]
    const bValue = b[sortConfig.key]

    if (aValue === undefined || bValue === undefined) return 0

    const comparison =
      typeof aValue === 'string'
        ? aValue.localeCompare(String(bValue))
        : Number(aValue) - Number(bValue)

    return sortConfig.direction === 'asc' ? comparison : -comparison
  })

  const getSortIndicator = (key: keyof LeaderboardEntry) => {
    if (sortConfig.key !== key) return ' ▼▲'
    return sortConfig.direction === 'asc' ? ' ▼' : ' ▲'
  }

  return (
    <div className={styles.tableContainer}>
      <table>
        <thead>
          <tr>
            <th onClick={() => handleSort('rank')}>
              {t('leaderboard.rank')}
              {getSortIndicator('rank')}
            </th>
            <th onClick={() => handleSort('playerName')}>
              {t('leaderboard.player')}
              {getSortIndicator('playerName')}
            </th>
            <th onClick={() => handleSort('citiesCompleted')}>
              {t('leaderboard.cities')}
              {getSortIndicator('citiesCompleted')}
            </th>
            <th onClick={() => handleSort('totalDistance')}>
              {t('leaderboard.distance')}
              {getSortIndicator('totalDistance')}
            </th>
          </tr>
        </thead>
        <tbody>
          {sortedData.map((entry) => (
            <tr key={entry.playerId}>
              <td className={styles.rank}>{entry.rank}</td>
              <td className={styles.player}>
                <Link to={`/player/${entry.playerId}`}>{entry.playerName}</Link>
              </td>
              <td className={styles.cities}>{entry.citiesCompleted}</td>
              <td className={styles.distance}>
                {entry.totalDistance.toFixed(1)} km
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
