import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { City } from '@/types'
import { getFlagEmoji } from '@/lib/flags'
import styles from './CityCard.module.css'

export default function CityCard({ city }: { city: City }) {
  const { t } = useTranslation()

  return (
    <Link to={`/city/${city.id}`} className={styles.card}>
      <div className={styles.header}>
        <h3>{city.name}</h3>
        <span className={styles.country} aria-label={city.country}>
          {getFlagEmoji(city.country)}
        </span>
      </div>
      <div className={styles.info}>
        <div className={styles.stat}>
          <span className={styles.label}>{t('cities.streets')}</span>
          <span className={styles.value}>{city.streetCount}</span>
        </div>
        <div className={styles.stat}>
          <span className={styles.label}>{t('cities.kilometers')}</span>
          <span className={styles.value}>
            {(city.totalMeters / 1000).toFixed(1)}
          </span>
        </div>
      </div>
      <div className={styles.footer}>
        <small>{city.postalCode}</small>
      </div>
    </Link>
  )
}
