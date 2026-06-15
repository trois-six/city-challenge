import { useEffect, useState, type FormEvent } from 'react'
import { useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { City, PathData, StatsData } from '@/types'
import { getCity, getCityEditions, getCityPath, getCityStats } from '@/lib/data'
import LeaderboardTable from '@/components/LeaderboardTable'
import CityMap from '@/components/CityMap'
import { getFlagEmoji } from '@/lib/flags'
import styles from './CityDetailPage.module.css'

export default function CityDetailPage() {
  const { id } = useParams<{ id: string }>()
  const { t } = useTranslation()

  const [city, setCity] = useState<City | null>(null)
  const [stats, setStats] = useState<StatsData | null>(null)
  const [pathData, setPathData] = useState<PathData | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [comment, setComment] = useState('')

  useEffect(() => {
    if (!id) return

    const fetchData = async () => {
      try {
        setLoading(true)
        setError(null)

        const cityData = await getCity(id)
        if (!cityData) throw new Error(t('cities.error'))
        setCity(cityData)

        const editions = await getCityEditions(cityData.dir)
        const currentEdition = editions.find((edition) => edition.current) ?? editions[0]
        if (currentEdition) {
          const [path, statsData] = await Promise.all([
            getCityPath(cityData.dir, currentEdition.id),
            getCityStats(cityData.dir, currentEdition.id),
          ])
          setPathData(path)
          setStats(statsData)
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : t('common.error'))
      } finally {
        setLoading(false)
      }
    }

    fetchData()
  }, [id, t])

  const handleCommentSubmit = async (e: FormEvent) => {
    e.preventDefault()
    if (!comment.trim() || !id) return

    try {
      const response = await fetch(`/api/cities/${id}/comments`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ text: comment }),
      })
      if (response.ok) {
        setComment('')
      }
    } catch (err) {
      console.error('Failed to post comment:', err)
    }
  }

  if (loading) {
    return (
      <div className={styles.container}>
        <div className="skeleton" style={{ height: '300px', marginBottom: '2rem' }} />
        <div className="skeleton" style={{ height: '200px' }} />
      </div>
    )
  }

  if (error || !city) {
    return (
      <div className={styles.container}>
        <div className="error-box">{error || t('common.error')}</div>
        <button onClick={() => window.history.back()}>{t('common.retry')}</button>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <div className={styles.header}>
        <h1>{city.name}</h1>
        <p>
          <span aria-label={city.country}>{getFlagEmoji(city.country)}</span> • {city.postalCode}
        </p>
      </div>

      {pathData && <CityMap pathData={pathData} />}

      <section className={styles.section}>
        <h2>{t('cities.leaderboardTitle', { cityName: city.name })}</h2>
        {stats && stats.leaderboard.length > 0 ? (
          <LeaderboardTable data={stats.leaderboard} />
        ) : (
          <p>{t('common.noData')}</p>
        )}
      </section>

      <section className={styles.section}>
        <h2>{t('cities.comments')}</h2>
        <form onSubmit={handleCommentSubmit} className={styles.commentForm}>
          <textarea
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            placeholder={t('cities.commentPlaceholder')}
            rows={4}
          />
          <button type="submit" disabled={!comment.trim()}>
            {t('cities.competitionButton')}
          </button>
        </form>
      </section>
    </div>
  )
}
