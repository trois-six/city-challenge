import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Player } from '@/types'
import { getPlayers } from '@/lib/data'
import PlayerCard from '@/components/PlayerCard'
import styles from './PlayersPage.module.css'

export default function PlayersPage() {
  const { t } = useTranslation()
  const [players, setPlayers] = useState<Player[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const fetchPlayers = async () => {
      try {
        setLoading(true)
        setError(null)
        setPlayers(await getPlayers())
      } catch (err) {
        setError(
          err instanceof Error ? err.message : t('common.error')
        )
      } finally {
        setLoading(false)
      }
    }

    fetchPlayers()
  }, [t])

  if (loading) {
    return (
      <div className={styles.container}>
        <h1>{t('players.title')}</h1>
        <div className={styles.grid}>
          {[1, 2, 3, 4, 5, 6].map((i) => (
            <div key={i} className={`${styles.card} skeleton`} style={{ height: '250px' }} />
          ))}
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className={styles.container}>
        <h1>{t('players.title')}</h1>
        <div className="error-box">{error}</div>
        <button onClick={() => window.location.reload()}>{t('common.retry')}</button>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <h1>{t('players.title')}</h1>
      {players.length === 0 ? (
        <p>{t('common.noData')}</p>
      ) : (
        <div className={styles.grid}>
          {players.map((player) => (
            <PlayerCard key={player.id} player={player} />
          ))}
        </div>
      )}
    </div>
  )
}
