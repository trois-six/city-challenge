import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { City, Player } from '@/types'
import { getCities, getPlayer } from '@/lib/data'
import { getFlagEmoji } from '@/lib/flags'
import styles from './PlayerDetailPage.module.css'

export default function PlayerDetailPage() {
  const { id } = useParams<{ id: string }>()
  const { t } = useTranslation()

  const [player, setPlayer] = useState<Player | null>(null)
  const [cities, setCities] = useState<City[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!id) return

    const fetchData = async () => {
      try {
        setLoading(true)
        setError(null)

        const [playerData, citiesData] = await Promise.all([getPlayer(id), getCities()])
        if (!playerData) throw new Error(t('players.notFound'))
        setPlayer(playerData)
        setCities(citiesData)
      } catch (err) {
        setError(err instanceof Error ? err.message : t('common.error'))
      } finally {
        setLoading(false)
      }
    }

    fetchData()
  }, [id, t])

  if (loading) {
    return (
      <div className={styles.container}>
        <div className="skeleton" style={{ height: '150px', marginBottom: '2rem' }} />
        <div className="skeleton" style={{ height: '300px' }} />
      </div>
    )
  }

  if (error || !player) {
    return (
      <div className={styles.container}>
        <div className="error-box">{error || t('common.error')}</div>
        <button onClick={() => window.history.back()}>{t('common.retry')}</button>
      </div>
    )
  }

  const achievementLabel = (type: string) => {
    switch (type) {
      case 'first_city':
        return t('players.firstCity')
      case 'all_streets':
        return t('players.allStreets')
      default:
        return t('players.fastest')
    }
  }

  const cityName = (cityId: string) => cities.find((city) => city.id === cityId)?.name ?? cityId

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <span className={styles.flag} aria-label={player.country}>
          {getFlagEmoji(player.country)}
        </span>
        <h1>{player.name}</h1>
      </div>

      <section className={styles.section}>
        <h2>{t('players.stats')}</h2>
        <div className={styles.stats}>
          <div className={styles.stat}>
            <span className={styles.label}>{t('players.totalDistance')}</span>
            <span className={styles.value}>
              {player.totalDistance.toFixed(1)} {t('players.km')}
            </span>
          </div>
          <div className={styles.stat}>
            <span className={styles.label}>{t('players.citiesCompleted')}</span>
            <span className={styles.value}>{player.citiesCompleted}</span>
          </div>
        </div>
      </section>

      <section className={styles.section}>
        <h2>{t('players.achievements')}</h2>
        {player.achievements.length > 0 ? (
          <ul className={styles.achievements}>
            {player.achievements.map((achievement) => (
              <li key={`${achievement.type}-${achievement.cityId}`}>
                {achievementLabel(achievement.type)} — {cityName(achievement.cityId)}
              </li>
            ))}
          </ul>
        ) : (
          <p>{t('common.noData')}</p>
        )}
      </section>

      <section className={styles.section}>
        <h2>{t('players.resultsTitle')}</h2>
        {player.results.length > 0 ? (
          <div className={styles.tableContainer}>
            <table>
              <thead>
                <tr>
                  <th>{t('players.city')}</th>
                  <th>{t('cities.rank')}</th>
                  <th>{t('cities.distance')}</th>
                  <th>{t('cities.completed_label')}</th>
                </tr>
              </thead>
              <tbody>
                {player.results.map((result) => (
                  <tr key={result.cityId}>
                    <td>
                      <Link to={`/city/${result.cityId}`}>{cityName(result.cityId)}</Link>
                    </td>
                    <td>{result.rank}</td>
                    <td>{result.distance.toFixed(1)} km</td>
                    <td>{result.completed ? '✓' : '—'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p>{t('common.noData')}</p>
        )}
      </section>
    </div>
  )
}
