import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { LeaderboardEntry } from '@/types'
import { getLeaderboard } from '@/lib/data'
import LeaderboardTableGeneral from '@/components/LeaderboardTableGeneral'
import styles from './LeaderboardPage.module.css'

export default function LeaderboardPage() {
  const { t } = useTranslation()
  const [entries, setEntries] = useState<LeaderboardEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const fetchLeaderboard = async () => {
      try {
        setLoading(true)
        setError(null)
        setEntries(await getLeaderboard())
      } catch (err) {
        setError(
          err instanceof Error ? err.message : t('common.error')
        )
      } finally {
        setLoading(false)
      }
    }

    fetchLeaderboard()
  }, [t])

  if (loading) {
    return (
      <div className={styles.container}>
        <h1>{t('leaderboard.title')}</h1>
        <div className="skeleton" style={{ height: '400px' }} />
      </div>
    )
  }

  if (error) {
    return (
      <div className={styles.container}>
        <h1>{t('leaderboard.title')}</h1>
        <div className="error-box">{error}</div>
        <button onClick={() => window.location.reload()}>{t('common.retry')}</button>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <h1>{t('leaderboard.title')}</h1>
      {entries.length === 0 ? (
        <p>{t('common.noData')}</p>
      ) : (
        <LeaderboardTableGeneral data={entries} />
      )}
    </div>
  )
}
