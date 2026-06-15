import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { Player } from '@/types'
import { getFlagEmoji } from '@/lib/flags'
import styles from './PlayerCard.module.css'

export default function PlayerCard({ player }: { player: Player }) {
  const { t } = useTranslation()

  return (
    <Link to={`/player/${player.id}`} className={styles.card}>
      <div className={styles.header}>
        <h3>{player.name}</h3>
        <span className={styles.country} aria-label={player.country}>
          {getFlagEmoji(player.country)}
        </span>
      </div>
      <div className={styles.stats}>
        <div className={styles.stat}>
          <span className={styles.label}>{t('players.totalDistance')}</span>
          <span className={styles.value}>
            {player.totalDistance.toFixed(0)} {t('players.km')}
          </span>
        </div>
        <div className={styles.stat}>
          <span className={styles.label}>{t('players.citiesCompleted')}</span>
          <span className={styles.value}>{player.citiesCompleted}</span>
        </div>
      </div>
      <div className={styles.achievements}>
        <h4>{t('players.achievements')}</h4>
        {player.achievements.length > 0 ? (
          <ul>
            {player.achievements.map((achievement) => (
              <li key={`${achievement.type}-${achievement.cityId}`}>
                {achievement.type === 'first_city'
                  ? t('players.firstCity')
                  : achievement.type === 'all_streets'
                  ? t('players.allStreets')
                  : t('players.fastest')}
              </li>
            ))}
          </ul>
        ) : (
          <p>{t('common.noData')}</p>
        )}
      </div>
    </Link>
  )
}
