import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { PlayerResult } from '@/types'
import styles from './LeaderboardTable.module.css'

interface SortConfig {
  key: keyof PlayerResult
  direction: 'asc' | 'desc'
}

export default function LeaderboardTable({ data }: { data: PlayerResult[] }) {
  const { t } = useTranslation()
  const [sortConfig, setSortConfig] = useState<SortConfig>({
    key: 'rank',
    direction: 'asc',
  })

  const handleSort = (key: keyof PlayerResult) => {
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

  const getSortIndicator = (key: keyof PlayerResult) => {
    if (sortConfig.key !== key) return ' ▼▲'
    return sortConfig.direction === 'asc' ? ' ▼' : ' ▲'
  }

  return (
    <div className={styles.tableContainer}>
      <table>
        <thead>
          <tr>
            <th onClick={() => handleSort('rank')}>
              {t('cities.rank')}
              {getSortIndicator('rank')}
            </th>
            <th onClick={() => handleSort('playerName')}>
              {t('cities.player')}
              {getSortIndicator('playerName')}
            </th>
            <th onClick={() => handleSort('distance')}>
              {t('cities.distance')}
              {getSortIndicator('distance')}
            </th>
            <th onClick={() => handleSort('completed')}>
              {t('cities.completed_label')}
              {getSortIndicator('completed')}
            </th>
          </tr>
        </thead>
        <tbody>
          {sortedData.map((result) => (
            <tr key={`${result.playerId}-${result.cityId}`}>
              <td className={styles.rank}>{result.rank}</td>
              <td className={styles.player}>
                <Link to={`/player/${result.playerId}`}>{result.playerName}</Link>
              </td>
              <td className={styles.distance}>
                {result.distance.toFixed(1)} km
              </td>
              <td className={styles.completed}>
                {result.completed ? '✓' : '—'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
