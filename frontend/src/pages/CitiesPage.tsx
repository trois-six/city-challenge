import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { City } from '@/types'
import { getCities } from '@/lib/data'
import CityCard from '@/components/CityCard'
import styles from './CitiesPage.module.css'

export default function CitiesPage() {
  const { t } = useTranslation()
  const [cities, setCities] = useState<City[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const fetchCities = async () => {
      try {
        setLoading(true)
        setError(null)
        setCities(await getCities())
      } catch (err) {
        setError(
          err instanceof Error ? err.message : t('common.error')
        )
      } finally {
        setLoading(false)
      }
    }

    fetchCities()
  }, [t])

  if (loading) {
    return (
      <div className={styles.container}>
        <h1>{t('cities.title')}</h1>
        <div className={styles.grid}>
          {[1, 2, 3, 4, 5, 6].map((i) => (
            <div key={i} className={`${styles.card} skeleton`} style={{ height: '200px' }} />
          ))}
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className={styles.container}>
        <h1>{t('cities.title')}</h1>
        <div className="error-box">{error}</div>
        <button onClick={() => window.location.reload()}>{t('common.retry')}</button>
      </div>
    )
  }

  return (
    <div className={styles.container}>
      <h1>{t('cities.title')}</h1>
      {cities.length === 0 ? (
        <p>{t('common.noData')}</p>
      ) : (
        <div className={styles.grid}>
          {cities.map((city) => (
            <CityCard key={city.id} city={city} />
          ))}
        </div>
      )}
    </div>
  )
}
